//! Motion a plugin **declares** and the kernel drives.
//!
//! Pure data (no local crate imports beyond `super`), matching the `session/`
//! architecture rule. The split matters more here than elsewhere: `ui` may not
//! reference `crate::plugin`, so the answer to "which frame is showing" has to
//! reach the renderer as *data*. [`FrameTable`] is that data — built by `app`
//! from its clock before the paint, read by `ui` during it — which is why the
//! renderer cannot call a plugin to animate even by accident.
//!
//! The plugin owns the frames; the kernel owns the clock. A plugin pushes once
//! and never participates in a frame, so an animation costs a repaint of one
//! pane and nothing else — no plugin call, no tree rebuild, no diff.

use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::time::Duration;

use super::view_tree::ViewNode;

/// Frame rate used when a declaration omits one.
pub const DEFAULT_FPS: u8 = 8;

/// Slowest rate a motion may be served at. Zero would mean "never advance",
/// which is a static node written the hard way.
pub const MIN_FPS: u8 = 1;

/// Fastest rate any single pane may be served at.
pub const MAX_FPS: u8 = 30;

/// Fastest total rate across every lease. Ten animated panes must not cost ten
/// times one animated pane; the budget is what makes a lease grantable without
/// knowing how many other plugins are installed.
pub const AGGREGATE_FPS: u16 = 30;

/// Below this served rate an animation reads as stutter rather than motion, so
/// the kernel freezes it instead (see [`allocate_rates`]).
pub const FREEZE_FLOOR_FPS: u8 = 4;

/// Fewest frames a motion may declare. One frame is a static node.
pub const MIN_FRAMES: usize = 2;

/// Most frames a motion may declare. Frames ship inside the pushed tree, so
/// this bounds what one animation can cost before it is ever drawn.
pub const MAX_FRAMES: usize = 64;

/// What a motion does over time.
///
/// One variant today. `marquee`, `pulse`, `blink` and `tween` are specified in
/// `docs/v2/FEATURES-Animation.md` §2 and deliberately absent: the three that
/// are not expressible as frames need a resolved rect or layout props the view
/// tree does not carry yet. Growing this enum is additive, and an unrecognised
/// kind is a named conversion error rather than a silently static node.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum MotionKind {
    /// Round-robin pre-supplied subtrees — the general case: spinners, typing
    /// indicators, hand-drawn frame animation, and (as two frames) a pulse.
    Cycle {
        /// Rendered in order. Bounded by [`MIN_FRAMES`]/[`MAX_FRAMES`].
        frames: Vec<ViewNode>,
        /// `false` holds the last frame and drops the lease.
        repeat: bool,
    },
}

/// A motion declaration, already clamped to the host's bounds.
///
/// There is deliberately no "pause while unfocused" knob. `FEATURES-Animation.md`
/// §4 lists one, but a plugin pane stays *visible* when focus moves elsewhere,
/// so honouring it would freeze an animation the user can still see — which
/// reads as a hung pane, not as a saving. thurbox's own working spinner animates
/// regardless of which pane holds focus for exactly that reason. The cost case
/// the knob was for is a pane nobody is looking at, and that is already covered:
/// a hidden pane drops its lease and its state.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Motion {
    /// Declared rate, clamped into `[MIN_FPS, MAX_FPS]` at conversion.
    pub fps: u8,
    /// What it does.
    pub kind: MotionKind,
}

impl Motion {
    /// Build a cycle, clamping the rate. Frame-count bounds are enforced at
    /// conversion, where a violation can be reported to the plugin author.
    pub fn cycle(frames: Vec<ViewNode>, fps: u8, repeat: bool) -> Motion {
        Motion {
            fps: fps.clamp(MIN_FPS, MAX_FPS),
            kind: MotionKind::Cycle { frames, repeat },
        }
    }

    /// The subtrees this motion can draw. Exposed as the node's children so
    /// that a motion's frames count against the same depth and node bounds
    /// every other tree pays.
    pub fn frames(&self) -> &[ViewNode] {
        match &self.kind {
            MotionKind::Cycle { frames, .. } => frames,
        }
    }

    /// Whether the animation runs forever.
    pub fn repeats(&self) -> bool {
        match &self.kind {
            MotionKind::Cycle { repeat, .. } => *repeat,
        }
    }

    /// A hash of everything that changes what the animation looks like.
    ///
    /// Identity is `(pane, node key, signature)`: an identical re-push keeps
    /// its epoch and the animation continues, while any change to the
    /// declaration starts a new one. Frames are part of it because two motions
    /// that differ only in their frames are two different animations.
    pub fn signature(&self) -> u64 {
        let mut h = std::collections::hash_map::DefaultHasher::new();
        self.fps.hash(&mut h);
        match &self.kind {
            MotionKind::Cycle { frames, repeat } => {
                "cycle".hash(&mut h);
                repeat.hash(&mut h);
                frames.hash(&mut h);
            }
        }
        h.finish()
    }

    /// Which frame is showing `elapsed` after the epoch, when served at
    /// `served_fps`.
    ///
    /// Degradation is expressed by stretching the frame interval rather than by
    /// running a second clock, so an un-degraded animation (`served == fps`) is
    /// the identity case of the same arithmetic.
    pub fn frame_at(&self, elapsed: Duration, served_fps: u8) -> usize {
        let frames = self.frames().len();
        if frames == 0 {
            return 0;
        }
        let fps = served_fps.clamp(MIN_FPS, MAX_FPS) as u128;
        // Integer maths throughout: a float here would make the frame index
        // depend on rounding mode, and the acceptance harness asserts on exact
        // frames after an exact clock advance.
        let ticks = (elapsed.as_millis() * fps) / 1_000;
        if self.repeats() {
            (ticks % frames as u128) as usize
        } else {
            (ticks as usize).min(frames - 1)
        }
    }

    /// Whether this motion still needs a lease at `elapsed` past its epoch.
    ///
    /// A non-repeating cycle that has reached its last frame is finished: it
    /// keeps that frame on screen and stops costing repaints.
    pub fn is_live_at(&self, elapsed: Duration, served_fps: u8) -> bool {
        if self.repeats() {
            return self.frames().len() > 1;
        }
        self.frame_at(elapsed, served_fps) + 1 < self.frames().len()
    }
}

/// Which frame each animated node in one pane is showing.
///
/// Keys are the node keys carried by [`ViewNode::Motion`], derived once at
/// conversion so the collector and the renderer cannot disagree about them.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FrameTable {
    entries: HashMap<String, usize>,
}

impl FrameTable {
    /// Record the frame an animated node is showing.
    pub fn set(&mut self, key: impl Into<String>, frame: usize) {
        self.entries.insert(key.into(), frame);
    }

    /// The frame to draw for `key`.
    ///
    /// Frame 0 is the answer for anything not in the table — a suppressed
    /// motion, a pane the kernel declined to animate, or a paint that raced a
    /// push. This is why frame 0 must be a correct rendering on its own: for a
    /// user with reduced motion it is the only frame.
    pub fn frame(&self, key: &str) -> usize {
        self.entries.get(key).copied().unwrap_or(0)
    }

    /// How many animated nodes it covers.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether it covers no animated nodes.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// One pane asking to be animated.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LeaseRequest {
    /// The fastest rate any motion in that pane declared.
    pub declared_fps: u8,
    /// Whether the pane holds focus.
    pub focused: bool,
}

/// What a pane was granted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Grant {
    /// Served at this many frames per second.
    Served(u8),
    /// Over budget: hold the current frame rather than stutter.
    Frozen,
}

/// Distribute the aggregate rate budget over the requesting panes.
///
/// The focused pane keeps its declared rate; the rest are visited in ascending
/// declared rate and get max-min fair shares of what is left, which is what
/// "round-robin, cheap animations served first" means once written down. A
/// lease that must be degraded *and* would land below [`FREEZE_FLOOR_FPS`] is
/// frozen instead of served.
///
/// Freezing rather than slowing everyone is deliberate: halving every rate
/// makes every animation look broken, whereas freezing the greediest keeps the
/// rest correct.
///
/// Returns one grant per request, in the order the requests were given.
pub fn allocate_rates(requests: &[LeaseRequest]) -> Vec<Grant> {
    let mut grants = vec![Grant::Frozen; requests.len()];
    let mut budget = AGGREGATE_FPS;

    // The focused pane is served first and unconditionally: it is the one the
    // user is looking at.
    let focused = requests.iter().position(|r| r.focused);
    if let Some(i) = focused {
        let want = requests[i].declared_fps.clamp(MIN_FPS, MAX_FPS);
        let served = want.min(budget.min(u16::from(MAX_FPS)) as u8).max(MIN_FPS);
        grants[i] = Grant::Served(served);
        budget = budget.saturating_sub(u16::from(served));
    }

    let mut rest: Vec<usize> = (0..requests.len())
        .filter(|i| Some(*i) != focused)
        .collect();
    rest.sort_by_key(|&i| (requests[i].declared_fps, i));

    let mut remaining = rest.len() as u16;
    for i in rest {
        if remaining == 0 {
            break;
        }
        let share = budget / remaining;
        remaining -= 1;
        let want = u16::from(requests[i].declared_fps.clamp(MIN_FPS, MAX_FPS));
        let served = want.min(share);
        // Freeze only a lease that is both *degraded* and below the readable
        // floor. A motion that asked for 2 fps and got 2 fps is not stuttering
        // — it is running exactly as declared — so the floor applies to what
        // was lost, not to the absolute rate.
        if served < want && served < u16::from(FREEZE_FLOOR_FPS) {
            // Its share returns to the pool, so freezing one greedy lease can
            // be what keeps the next one servable.
            continue;
        }
        grants[i] = Grant::Served(served as u8);
        budget -= served;
    }

    grants
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frames(n: usize) -> Vec<ViewNode> {
        (0..n).map(|i| ViewNode::text(i.to_string())).collect()
    }

    fn cycle(n: usize, fps: u8, repeat: bool) -> Motion {
        Motion::cycle(frames(n), fps, repeat)
    }

    #[test]
    fn a_declared_rate_is_clamped_both_ways() {
        assert_eq!(cycle(2, 200, true).fps, MAX_FPS);
        assert_eq!(cycle(2, 0, true).fps, MIN_FPS);
        assert_eq!(cycle(2, 8, true).fps, 8);
    }

    #[test]
    fn frames_advance_on_the_declared_interval() {
        let m = cycle(4, 8, true);
        // 8 fps = one frame every 125 ms.
        assert_eq!(m.frame_at(Duration::ZERO, 8), 0);
        assert_eq!(m.frame_at(Duration::from_millis(124), 8), 0);
        assert_eq!(m.frame_at(Duration::from_millis(125), 8), 1);
        assert_eq!(m.frame_at(Duration::from_millis(375), 8), 3);
        assert_eq!(m.frame_at(Duration::from_millis(500), 8), 0, "wraps");
    }

    #[test]
    fn a_degraded_rate_stretches_the_interval() {
        let m = cycle(4, 8, true);
        assert_eq!(m.frame_at(Duration::from_millis(125), 4), 0);
        assert_eq!(m.frame_at(Duration::from_millis(250), 4), 1);
    }

    #[test]
    fn a_non_repeating_cycle_holds_its_last_frame_and_goes_quiet() {
        let m = cycle(3, 8, false);
        assert_eq!(m.frame_at(Duration::from_secs(10), 8), 2);
        assert!(m.is_live_at(Duration::from_millis(125), 8));
        assert!(
            !m.is_live_at(Duration::from_secs(10), 8),
            "a finished cycle must stop costing repaints"
        );
    }

    #[test]
    fn a_repeating_cycle_stays_live() {
        assert!(cycle(2, 8, true).is_live_at(Duration::from_secs(3600), 8));
    }

    #[test]
    fn the_signature_covers_everything_that_changes_the_animation() {
        let base = cycle(3, 8, true);
        assert_eq!(base.signature(), cycle(3, 8, true).signature());
        assert_ne!(base.signature(), cycle(3, 9, true).signature(), "rate");
        assert_ne!(base.signature(), cycle(3, 8, false).signature(), "repeat");
        assert_ne!(base.signature(), cycle(4, 8, true).signature(), "frames");
    }

    #[test]
    fn an_unknown_key_reads_as_frame_zero() {
        let mut t = FrameTable::default();
        t.set("known", 3);
        assert_eq!(t.frame("known"), 3);
        assert_eq!(t.frame("absent"), 0);
        assert_eq!(t.len(), 1);
        assert!(!t.is_empty());
    }

    #[test]
    fn every_lease_within_budget_is_served_in_full() {
        let reqs = [
            LeaseRequest {
                declared_fps: 8,
                focused: true,
            },
            LeaseRequest {
                declared_fps: 4,
                focused: false,
            },
        ];
        assert_eq!(
            allocate_rates(&reqs),
            vec![Grant::Served(8), Grant::Served(4)]
        );
    }

    #[test]
    fn the_focused_pane_keeps_its_rate_when_over_budget() {
        let reqs = [
            LeaseRequest {
                declared_fps: 30,
                focused: false,
            },
            LeaseRequest {
                declared_fps: 30,
                focused: true,
            },
        ];
        let grants = allocate_rates(&reqs);
        assert_eq!(grants[1], Grant::Served(30), "focused served in full");
        assert_eq!(grants[0], Grant::Frozen, "nothing left for the other");
    }

    #[test]
    fn a_starved_lease_freezes_rather_than_stutters() {
        // Ten panes at 10 fps want 100 fps against a 30 fps budget. Fair shares
        // are 3 fps, below the freeze floor, so the early ones freeze and their
        // share funds the later ones instead of everyone stuttering.
        let reqs: Vec<LeaseRequest> = (0..10)
            .map(|_| LeaseRequest {
                declared_fps: 10,
                focused: false,
            })
            .collect();
        let grants = allocate_rates(&reqs);

        let served: Vec<u8> = grants
            .iter()
            .filter_map(|g| match g {
                Grant::Served(f) => Some(*f),
                Grant::Frozen => None,
            })
            .collect();
        assert!(
            served.iter().all(|f| *f >= FREEZE_FLOOR_FPS),
            "nothing is served below the floor: {served:?}"
        );
        assert!(
            served.iter().map(|f| u16::from(*f)).sum::<u16>() <= AGGREGATE_FPS,
            "the aggregate cap holds: {served:?}"
        );
        assert!(!served.is_empty(), "someone is still animating");
    }

    #[test]
    fn cheap_animations_are_served_before_greedy_ones() {
        let reqs = [
            LeaseRequest {
                declared_fps: 30,
                focused: false,
            },
            LeaseRequest {
                declared_fps: 2,
                focused: false,
            },
        ];
        let grants = allocate_rates(&reqs);
        assert_eq!(
            grants[1],
            Grant::Served(2),
            "a 2 fps motion getting 2 fps is not stuttering, whatever the floor"
        );
        assert!(matches!(grants[0], Grant::Served(_)), "and so is the rest");
    }

    #[test]
    fn no_requests_allocate_nothing() {
        assert!(allocate_rates(&[]).is_empty());
    }
}
