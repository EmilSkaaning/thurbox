//! When a plugin pane is asked to render, and which panes (ADR-49).
//!
//! The render worker used to render every visible pane and then sleep for a fixed
//! second. Nothing told it that kernel state had moved, so a plugin's copy of a
//! cursor trailed the kernel's by up to that second, and an idle TUI entered a
//! Luau VM once per pane per second for a tree that had not changed.
//!
//! This module is the replacement decision, and it is **pure**: it holds no
//! clock, no channel and no host, taking `now` as a parameter and the pane list as
//! an argument. That is not fastidiousness — the loop it drives lives in
//! `src/main.rs`, which is a binary, so a policy written inline there is a policy
//! no test can reach. Everything that decides lives here; what stays in `main` is
//! a channel receive, a filesystem stat, and a `Vec` sent down a pipe.
//!
//! ## The two rules
//!
//! **A pane renders when a source it reads moves.** Sources are named by
//! [`crate::session::plugin_manifest::PaneSource`], one per state-reading
//! capability, so a pane granted `tasks` is not re-rendered because host CPU
//! resampled. A pane is also rendered when its plugin was offered input (answering
//! may have changed the plugin's own state), when it has just become visible after
//! being skipped, and when its plugin was reloaded.
//!
//! **A render pass happens at most once per [`MIN_INTERVAL`].** The bound
//! *coalesces*, it does not delay: a change arriving at rest renders immediately,
//! and changes arriving inside the interval merge into one pass at its end. A
//! bound is needed because a source can move far faster than a user can perceive —
//! agent activity text changes on consecutive ticks — and an unbounded trigger
//! would put a VM call on every one.

use std::time::{Duration, Instant};

use crate::session::plugin_manifest::{PaneSource, SourceSet};

/// The floor between two render passes: ten passes a second.
///
/// Three reasons for this number rather than a smaller one. It is the session-list
/// spike's own bar (`view.push` rate ≤ 10 Hz sustained), so the trigger cannot
/// break the bar it was written against. It is tighter than the kernel's own
/// 250 ms forced-redraw floor, so a plugin pane can never be more than one forced
/// frame behind the interface around it. And it was the ceiling named when the gap
/// was filed.
///
/// The worst case it introduces is a change arriving just after a pass, which
/// waits out the remainder — recorded rather than rounded off in
/// `docs/PHASE4-PANE-READINESS.md`.
pub const MIN_INTERVAL: Duration = Duration::from_millis(100);

/// A pane, by the pair that names it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaneRef {
    /// Owning plugin.
    pub plugin: String,
    /// Pane id within that plugin.
    pub pane: String,
}

impl PaneRef {
    pub fn new(plugin: impl Into<String>, pane: impl Into<String>) -> Self {
        PaneRef {
            plugin: plugin.into(),
            pane: pane.into(),
        }
    }
}

/// One pane the kernel is showing, and what its plugin may read.
///
/// The sources come from the plugin's **granted** capabilities rather than from
/// its manifest's request, for the reason `PluginHost::pane_bindings` reads the
/// grant: a pane re-rendered for a source its plugin would then be refused is a
/// VM call that can produce nothing new.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaneReads {
    pub pane: PaneRef,
    pub sources: SourceSet,
}

/// What the worker should do next.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Due {
    /// Something a pane may read moved and the interval has elapsed: render now.
    Now,
    /// Something moved, but a pass happened inside the interval. This is what is
    /// left of it.
    Throttled(Duration),
    /// Nothing moved. There is nothing to render however long the worker waits.
    Idle,
}

/// What the worker has been told since its last render pass.
#[derive(Debug, Default)]
pub struct RenderTrigger {
    /// Sources whose state has moved.
    pending: SourceSet,
    /// Panes to render whatever moved — the ones whose plugin was offered input.
    forced: Vec<PaneRef>,
    /// Render every visible pane: a plugin reloaded, or a pane became visible
    /// after being skipped, so a tree the worker never produced is missing rather
    /// than stale.
    everything: bool,
    /// When the last pass that actually entered a VM happened. `None` before the
    /// first, which is why the first pass is never throttled.
    last_pass: Option<Instant>,
}

impl RenderTrigger {
    pub fn new() -> Self {
        RenderTrigger::default()
    }

    /// Record that the state behind `sources` moved.
    pub fn state_moved(&mut self, sources: SourceSet) {
        self.pending.extend(sources);
    }

    /// Record that the plugin's own durable state may have moved.
    ///
    /// Separate from [`Self::state_moved`] only in who calls it: the publisher
    /// cannot, because that state is not in the snapshot and no event announces a
    /// write to it (a plugin's headless half writes it from another process). So
    /// the worker raises it on a cadence, and this is the name of that fact.
    pub fn plugin_state_may_have_moved(&mut self) {
        self.pending.insert(PaneSource::PluginState);
    }

    /// Record that every visible pane must be rendered.
    pub fn everything_moved(&mut self) {
        self.everything = true;
    }

    /// Record that a pane's plugin was offered input, so its own state may have
    /// moved in answering — whether or not it reported the event consumed, since a
    /// plugin may change state and still decline the key.
    pub fn pane_took_input(&mut self, plugin: &str, pane: &str) {
        let it = PaneRef::new(plugin, pane);
        if !self.forced.contains(&it) {
            self.forced.push(it);
        }
    }

    /// Whether anything at all is waiting to be rendered.
    pub fn has_work(&self) -> bool {
        self.everything || !self.pending.is_empty() || !self.forced.is_empty()
    }

    /// What to do at `now`.
    ///
    /// Cheap by design — no allocation and no pane list — so the worker can ask it
    /// on every wakeup and only build the pane list when the answer is [`Due::Now`].
    pub fn due(&self, now: Instant) -> Due {
        if !self.has_work() {
            return Due::Idle;
        }
        let Some(last) = self.last_pass else {
            return Due::Now;
        };
        let elapsed = now.saturating_duration_since(last);
        match MIN_INTERVAL.checked_sub(elapsed) {
            // `checked_sub` yields `None` once the interval has passed, and
            // `Some(0)` exactly at its boundary — both mean "render".
            None => Due::Now,
            Some(left) if left.is_zero() => Due::Now,
            Some(left) => Due::Throttled(left),
        }
    }

    /// Which of `panes` this pass should render, in the order given.
    ///
    /// May be **empty** while [`Self::due`] says `Now`: a source moved that no
    /// visible pane reads, which is the case the whole design exists to make free.
    /// The caller still settles the trigger, so the pending set clears.
    pub fn wanted(&self, panes: &[PaneReads]) -> Vec<PaneRef> {
        panes
            .iter()
            .filter(|p| {
                self.everything
                    || p.sources.intersects(self.pending)
                    || self.forced.contains(&p.pane)
            })
            .map(|p| p.pane.clone())
            .collect()
    }

    /// Clear what has been handled.
    ///
    /// `rendered_any` advances the rate clock; a pass that entered no VM leaves it
    /// alone, so a change no visible pane reads cannot delay the next real render
    /// by up to [`MIN_INTERVAL`].
    pub fn settle(&mut self, now: Instant, rendered_any: bool) {
        self.pending = SourceSet::empty();
        self.forced.clear();
        self.everything = false;
        if rendered_any {
            self.last_pass = Some(now);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reads(plugin: &str, pane: &str, sources: SourceSet) -> PaneReads {
        PaneReads {
            pane: PaneRef::new(plugin, pane),
            sources,
        }
    }

    /// One pane per source, so a selection test can name what it expected.
    fn three_panes() -> Vec<PaneReads> {
        vec![
            reads("tasks", "tasks", SourceSet::of(PaneSource::Tasks)),
            reads("info", "info", SourceSet::of(PaneSource::Metrics)),
            reads(
                "list",
                "sessions",
                SourceSet::of(PaneSource::Sessions).with(PaneSource::Metrics),
            ),
        ]
    }

    #[test]
    fn nothing_moved_is_never_due() {
        let trigger = RenderTrigger::new();
        assert!(!trigger.has_work());
        assert_eq!(trigger.due(Instant::now()), Due::Idle);
        // And the answer does not change with time, which is the whole point: an
        // idle host enters no VM however long it runs.
        let later = Instant::now() + Duration::from_secs(600);
        assert_eq!(trigger.due(later), Due::Idle);
        assert!(trigger.wanted(&three_panes()).is_empty());
    }

    #[test]
    fn the_first_pass_is_not_throttled() {
        let mut trigger = RenderTrigger::new();
        trigger.state_moved(SourceSet::of(PaneSource::Tasks));
        assert_eq!(trigger.due(Instant::now()), Due::Now);
    }

    #[test]
    fn a_pane_is_rendered_only_for_a_source_it_reads() {
        let mut trigger = RenderTrigger::new();
        trigger.state_moved(SourceSet::of(PaneSource::Tasks));
        assert_eq!(
            trigger.wanted(&three_panes()),
            vec![PaneRef::new("tasks", "tasks")]
        );

        // Metrics reaches two of the three, because one pane's plugin holds both
        // grants — a pane reads the union of its plugin's sources.
        let mut trigger = RenderTrigger::new();
        trigger.state_moved(SourceSet::of(PaneSource::Metrics));
        assert_eq!(
            trigger.wanted(&three_panes()),
            vec![
                PaneRef::new("info", "info"),
                PaneRef::new("list", "sessions")
            ]
        );
    }

    #[test]
    fn a_source_nobody_reads_renders_nothing() {
        let mut trigger = RenderTrigger::new();
        trigger.state_moved(SourceSet::of(PaneSource::Review));
        assert_eq!(
            trigger.due(Instant::now()),
            Due::Now,
            "the worker still looks, because it is the pane list that answers"
        );
        assert!(
            trigger.wanted(&three_panes()).is_empty(),
            "no visible pane reads the review, so no VM is entered"
        );
    }

    /// The rate clock only moves for a pass that rendered something, so a change
    /// nobody reads cannot delay the next real render.
    #[test]
    fn a_pass_that_rendered_nothing_leaves_the_rate_clock_alone() {
        let start = Instant::now();
        let mut trigger = RenderTrigger::new();
        trigger.state_moved(SourceSet::of(PaneSource::Review));
        assert!(trigger.wanted(&three_panes()).is_empty());
        trigger.settle(start, false);

        trigger.state_moved(SourceSet::of(PaneSource::Tasks));
        assert_eq!(
            trigger.due(start + Duration::from_millis(1)),
            Due::Now,
            "the previous pass entered no VM, so nothing is owed to the ceiling"
        );
    }

    #[test]
    fn a_second_change_inside_the_interval_is_throttled_by_the_remainder() {
        let start = Instant::now();
        let mut trigger = RenderTrigger::new();
        trigger.state_moved(SourceSet::of(PaneSource::Tasks));
        assert_eq!(trigger.due(start), Due::Now);
        trigger.settle(start, true);

        assert_eq!(
            trigger.due(start + Duration::from_millis(10)),
            Due::Idle,
            "a settled trigger has nothing pending, whatever the clock says"
        );

        trigger.state_moved(SourceSet::of(PaneSource::Tasks));
        assert_eq!(
            trigger.due(start + Duration::from_millis(10)),
            Due::Throttled(Duration::from_millis(90))
        );
        assert_eq!(
            trigger.due(start + MIN_INTERVAL),
            Due::Now,
            "at the boundary the interval has elapsed"
        );
        assert_eq!(trigger.due(start + Duration::from_secs(5)), Due::Now);
    }

    /// Many changes inside one interval coalesce into a single pass, which is what
    /// bounds the render rate under a source that moves every tick.
    #[test]
    fn changes_inside_one_interval_coalesce() {
        let start = Instant::now();
        let mut trigger = RenderTrigger::new();
        trigger.state_moved(SourceSet::of(PaneSource::Tasks));
        trigger.settle(start, true);

        for ms in 1..100 {
            trigger.state_moved(SourceSet::of(PaneSource::Tasks));
            assert!(
                matches!(
                    trigger.due(start + Duration::from_millis(ms)),
                    Due::Throttled(_)
                ),
                "a change at +{ms}ms must wait for the interval"
            );
        }
        // One pass covers all of them, and it renders the pane once.
        assert_eq!(
            trigger.wanted(&three_panes()),
            vec![PaneRef::new("tasks", "tasks")]
        );
    }

    #[test]
    fn a_pane_offered_input_is_rendered_whatever_moved() {
        let mut trigger = RenderTrigger::new();
        trigger.pane_took_input("info", "info");
        assert!(trigger.has_work());
        assert_eq!(
            trigger.wanted(&three_panes()),
            vec![PaneRef::new("info", "info")],
            "the pane that answered, and no other"
        );
        // Recording it twice is one pane, not two renders.
        trigger.pane_took_input("info", "info");
        assert_eq!(trigger.wanted(&three_panes()).len(), 1);
    }

    #[test]
    fn everything_moved_renders_every_visible_pane() {
        let mut trigger = RenderTrigger::new();
        trigger.everything_moved();
        assert_eq!(trigger.wanted(&three_panes()).len(), 3);
        // Including a pane that reads nothing at all — a pane that has never
        // rendered must still get its first tree.
        let silent = vec![reads("hello", "hello", SourceSet::empty())];
        assert_eq!(trigger.wanted(&silent).len(), 1);
    }

    #[test]
    fn a_pane_that_reads_nothing_is_never_woken_by_state() {
        let mut trigger = RenderTrigger::new();
        trigger.state_moved(SourceSet::all());
        let silent = vec![reads("hello", "hello", SourceSet::empty())];
        assert!(
            trigger.wanted(&silent).is_empty(),
            "a pane granted no state reader has nothing to re-render for"
        );
    }

    /// The one surviving timer: a pane that may read its plugin's own state.
    #[test]
    fn the_plugin_state_cadence_reaches_only_the_panes_that_read_it() {
        let mut trigger = RenderTrigger::new();
        trigger.plugin_state_may_have_moved();
        let panes = vec![
            reads("stateful", "s", SourceSet::of(PaneSource::PluginState)),
            reads("plain", "p", SourceSet::of(PaneSource::Tasks)),
        ];
        assert_eq!(trigger.wanted(&panes), vec![PaneRef::new("stateful", "s")]);
    }

    #[test]
    fn settling_clears_every_kind_of_work() {
        let now = Instant::now();
        let mut trigger = RenderTrigger::new();
        trigger.state_moved(SourceSet::all());
        trigger.pane_took_input("info", "info");
        trigger.everything_moved();
        assert!(trigger.has_work());

        trigger.settle(now, true);
        assert!(!trigger.has_work());
        assert_eq!(trigger.due(now + Duration::from_secs(1)), Due::Idle);
        assert!(trigger.wanted(&three_panes()).is_empty());
    }

    /// A pane the kernel is not showing is simply absent from the list the trigger
    /// is given, so hiding one costs no render by construction rather than by a
    /// check here.
    #[test]
    fn a_hidden_pane_is_absent_from_the_list_rather_than_filtered_here() {
        let mut trigger = RenderTrigger::new();
        trigger.everything_moved();
        assert!(trigger.wanted(&[]).is_empty());
    }
}
