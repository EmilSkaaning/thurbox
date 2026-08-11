//! The kernel's half of declared motion: epochs, leases, and the frame tables
//! the renderer paints from.
//!
//! This is the only stateful piece of the motion system, and the only one that
//! reads a clock. Everything it decides is derived from the trees the panes
//! already hold, so a plugin contributes frames and nothing else — it cannot
//! schedule, request, or cause a paint.
//!
//! The demand-driven loop survives this because [`MotionState::sync`] reports a
//! change only when a resolved frame table actually differs. The tick runs at
//! ~100 Hz; an 8 fps cycle resolves to the same frame for ~12 consecutive
//! ticks, so eleven of every twelve syncs mark nothing dirty, and a hidden pane
//! is never evaluated at all.

use std::collections::HashMap;
use std::time::Instant;

use crate::plugin::PluginPane;
use crate::session::motion::{allocate_rates, FrameTable, Grant, LeaseRequest, Motion};
use crate::session::view_tree::ViewNode;

use super::clock;

/// Which pane an animated node belongs to. Pane ids are unique only within a
/// plugin, so both halves are part of the identity.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct PaneKey {
    /// Owning plugin.
    pub(crate) plugin: String,
    /// Pane id within that plugin.
    pub(crate) pane: String,
}

/// Identity of one animated node: its pane plus the key the conversion derived
/// from the node's `id` (or its structural path).
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
struct MotionKey {
    pane: PaneKey,
    node: String,
}

/// What the kernel remembers about one animated node.
#[derive(Debug, Clone)]
struct Epoch {
    /// Hash of the declaration, so an identical re-push is recognisable.
    signature: u64,
    /// When this key was first seen on this signature. Every frame is computed
    /// from `now - started`.
    started: Instant,
    /// The frame it was last resolved to, which is the frame a frozen lease
    /// holds.
    frame: usize,
}

/// One animated node found in a pane's current tree.
struct Site<'a> {
    key: MotionKey,
    motion: &'a Motion,
    keyed_by_id: bool,
}

/// Counters this sync produced, folded into the app's [`PerfCounters`] by the
/// caller. Returned rather than written directly so this module stays testable
/// without an `App`.
///
/// [`PerfCounters`]: super::metrics_state::PerfCounters
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct MotionCounters {
    /// Leases newly granted (a retained lease is not re-counted).
    pub(crate) granted: u64,
    /// Syncs that resolved a different frame table — one repaint each, and the
    /// whole cost motion adds to the render loop.
    pub(crate) frames: u64,
    /// Declared motions the kernel declined to animate: reduced motion, or a
    /// pane that is not on screen.
    pub(crate) denied: u64,
    /// Leases frozen by the aggregate budget.
    pub(crate) frozen: u64,
}

/// Epochs and frame tables for every animated node currently on screen.
#[derive(Debug, Default)]
pub(crate) struct MotionState {
    epochs: HashMap<MotionKey, Epoch>,
    tables: HashMap<PaneKey, FrameTable>,
    /// Panes currently holding an animation lease. Distinct from `tables`: a
    /// finished non-repeating cycle keeps its last frame on screen (a table)
    /// while costing no further repaints (no lease).
    leases: std::collections::HashSet<PaneKey>,
    /// Structural keys already reported, so the warning is one per node per
    /// session rather than one per tick.
    warned_structural: std::collections::HashSet<MotionKey>,
    counters: MotionCounters,
}

impl MotionState {
    /// Re-evaluate every pane's motion against the clock.
    ///
    /// Returns whether any pane's frame table changed — the *only* condition
    /// under which the caller may mark the UI dirty. Marking on anything else
    /// (a lease existing, a tick elapsing) would put motion back on the render
    /// loop's critical path.
    pub(crate) fn sync(
        &mut self,
        panes: &[PluginPane],
        focused: Option<&PaneKey>,
        reduce_motion: bool,
        features: &crate::session::settings::FeatureFlags,
    ) -> bool {
        let now = clock::now();

        // Every motion any pane declared, visible or not — the denominator the
        // denied counter is measured against.
        let declared: u64 = panes
            .iter()
            .filter_map(|p| p.tree())
            .map(count_motions)
            .sum();

        // Only panes on screen are evaluated: a hidden pane's — or a pane whose
        // declared `[features]` switch is off — motion is neither drawn nor paid
        // for, which is what makes an off-screen animation free.
        let mut sites: Vec<(PaneKey, Vec<Site>)> = Vec::new();
        if !reduce_motion {
            for pane in panes.iter().filter(|p| p.is_shown(features)) {
                let pane_key = PaneKey {
                    plugin: pane.plugin.clone(),
                    pane: pane.id.clone(),
                };
                let Some(tree) = pane.tree() else {
                    continue;
                };
                let mut found = Vec::new();
                collect(tree, &pane_key, &mut found);
                if !found.is_empty() {
                    sites.push((pane_key, found));
                }
            }
        }

        // Anything not evaluated this pass loses its epoch. That single rule
        // covers every drop in FEATURES-Animation.md §4 — a tree without
        // motion, a hidden pane, a closed pane, a reloaded plugin, reduced
        // motion — and is why motion state cannot accumulate.
        let live: std::collections::HashSet<&MotionKey> = sites
            .iter()
            .flat_map(|(_, s)| s.iter().map(|s| &s.key))
            .collect();
        self.epochs.retain(|k, _| live.contains(k));
        self.warned_structural.retain(|k| live.contains(k));

        for (pane_key, found) in &sites {
            for site in found {
                let sig = site.motion.signature();
                self.epochs
                    .entry(site.key.clone())
                    // The rule that makes re-pushing safe: an unchanged
                    // declaration keeps its epoch, so a plugin that re-renders
                    // on unrelated state changes does not pin its animation to
                    // frame 0.
                    .and_modify(|e| {
                        if e.signature != sig {
                            *e = Epoch {
                                signature: sig,
                                started: now,
                                frame: 0,
                            };
                        }
                    })
                    .or_insert(Epoch {
                        signature: sig,
                        started: now,
                        frame: 0,
                    });

                if !site.keyed_by_id && self.warned_structural.insert(site.key.clone()) {
                    tracing::warn!(
                        "plugin `{}` pane `{}` animates a node with no id; it is keyed by \
                         position ({}), so the animation restarts whenever the tree shape \
                         changes",
                        pane_key.plugin,
                        pane_key.pane,
                        site.key.node
                    );
                }
            }
        }

        // One lease per pane, at the fastest rate any of its nodes declared —
        // a pane with six animated nodes repaints once, not six times.
        let requests: Vec<LeaseRequest> = sites
            .iter()
            .map(|(pane_key, found)| LeaseRequest {
                declared_fps: found.iter().map(|s| s.motion.fps).max().unwrap_or(1),
                focused: focused == Some(pane_key),
            })
            .collect();
        let grants = allocate_rates(&requests);

        let mut tables: HashMap<PaneKey, FrameTable> = HashMap::new();
        let mut leases: std::collections::HashSet<PaneKey> = std::collections::HashSet::new();
        let mut served_sites = 0u64;
        let mut frozen = 0u64;
        for ((pane_key, found), grant) in sites.iter().zip(grants) {
            let served = match grant {
                Grant::Served(fps) => fps,
                Grant::Frozen => {
                    // Hold the last resolved frames rather than serving a rate
                    // that reads as stutter. The lease is retained: the pane is
                    // still animating, it is simply not being served this pass.
                    frozen += 1;
                    if let Some(prev) = self.tables.get(pane_key) {
                        tables.insert(pane_key.clone(), prev.clone());
                    }
                    leases.insert(pane_key.clone());
                    continue;
                }
            };

            let mut table = FrameTable::default();
            let mut any_live = false;
            for site in found {
                let Some(epoch) = self.epochs.get_mut(&site.key) else {
                    continue;
                };
                let elapsed = now.saturating_duration_since(epoch.started);
                let frame = site.motion.frame_at(elapsed, served);
                epoch.frame = frame;
                table.set(site.key.node.clone(), frame);
                any_live |= site.motion.is_live_at(elapsed, served);
                served_sites += 1;
            }

            // A pane whose every motion has finished keeps its last frame but
            // stops holding a lease.
            if any_live {
                leases.insert(pane_key.clone());
            }
            tables.insert(pane_key.clone(), table);
        }

        self.counters.granted += leases.difference(&self.leases).count() as u64;
        self.counters.frozen += frozen;
        self.counters.denied += declared.saturating_sub(served_sites);
        self.leases = leases;

        let changed = tables != self.tables;
        self.tables = tables;
        if changed {
            self.counters.frames += 1;
        }
        changed
    }

    /// The frame table for one pane, for the renderer.
    pub(crate) fn table_for(&self, plugin: &str, pane: &str) -> Option<&FrameTable> {
        self.tables.get(&PaneKey {
            plugin: plugin.to_string(),
            pane: pane.to_string(),
        })
    }

    /// Counters accumulated so far.
    pub(crate) fn counters(&self) -> MotionCounters {
        self.counters
    }

    /// How many animated nodes the kernel is currently tracking. The leak
    /// guard's observable form.
    #[cfg(test)]
    pub(crate) fn tracked(&self) -> usize {
        self.epochs.len()
    }

    /// Whether any pane currently holds a lease.
    #[cfg(test)]
    pub(crate) fn has_lease(&self) -> bool {
        !self.leases.is_empty()
    }
}

/// Walk a tree collecting its animated nodes.
fn collect<'a>(node: &'a ViewNode, pane: &PaneKey, out: &mut Vec<Site<'a>>) {
    if let ViewNode::Motion {
        key,
        keyed_by_id,
        motion,
    } = node
    {
        out.push(Site {
            key: MotionKey {
                pane: pane.clone(),
                node: key.clone(),
            },
            motion,
            keyed_by_id: *keyed_by_id,
        });
    }
    for child in node.children() {
        collect(child, pane, out);
    }
}

/// How many animated nodes a tree contains.
fn count_motions(node: &ViewNode) -> u64 {
    let here = u64::from(matches!(node, ViewNode::Motion { .. }));
    here + node.children().iter().map(count_motions).sum::<u64>()
}

#[cfg(test)]
impl MotionState {
    /// [`Self::sync`] with every `[features]` switch on.
    ///
    /// The cases below are about frames, leases and rates; the feature gate has
    /// its own tests in `plugin::pane` and in the acceptance harness, and
    /// spelling `FeatureFlags::default()` at twenty call sites would bury what
    /// each case is actually asserting.
    fn sync_ungated(
        &mut self,
        panes: &[PluginPane],
        focused: Option<&PaneKey>,
        reduce_motion: bool,
    ) -> bool {
        self.sync(
            panes,
            focused,
            reduce_motion,
            &crate::session::settings::FeatureFlags::default(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::plugin_manifest::PaneSlot;
    use crate::session::view_tree::ViewNode;
    use std::time::Duration;

    fn motion_node(key: &str, fps: u8, repeat: bool, frames: usize) -> ViewNode {
        ViewNode::Motion {
            key: key.to_string(),
            keyed_by_id: true,
            motion: Motion::cycle(
                (0..frames).map(|i| ViewNode::text(i.to_string())).collect(),
                fps,
                repeat,
            ),
        }
    }

    fn pane_with(tree: ViewNode, visible: bool) -> PluginPane {
        let mut p = PluginPane::loading("demo", "board", "Board", PaneSlot::Right, visible);
        p.apply(Ok(tree));
        p
    }

    #[test]
    fn an_identical_re_push_does_not_restart_the_animation() {
        let mut m = MotionState::default();
        let panes = vec![pane_with(motion_node("spinner", 8, true, 4), true)];
        m.sync_ungated(&panes, None, false);
        assert_eq!(m.table_for("demo", "board").unwrap().frame("spinner"), 0);

        clock::advance(Duration::from_millis(250));
        // The same tree, pushed again — exactly what a plugin re-rendering on
        // unrelated state does.
        let repushed = vec![pane_with(motion_node("spinner", 8, true, 4), true)];
        m.sync_ungated(&repushed, None, false);
        assert_eq!(
            m.table_for("demo", "board").unwrap().frame("spinner"),
            2,
            "phase must survive a re-push"
        );
    }

    #[test]
    fn changing_the_declaration_restarts_the_animation() {
        let mut m = MotionState::default();
        m.sync_ungated(
            &[pane_with(motion_node("s", 8, true, 4), true)],
            None,
            false,
        );
        clock::advance(Duration::from_millis(250));
        m.sync_ungated(
            &[pane_with(motion_node("s", 8, true, 4), true)],
            None,
            false,
        );
        assert_eq!(m.table_for("demo", "board").unwrap().frame("s"), 2);

        // A different rate is a different animation.
        m.sync_ungated(
            &[pane_with(motion_node("s", 4, true, 4), true)],
            None,
            false,
        );
        assert_eq!(m.table_for("demo", "board").unwrap().frame("s"), 0);
    }

    #[test]
    fn changing_the_node_identity_restarts_the_animation() {
        let mut m = MotionState::default();
        m.sync_ungated(
            &[pane_with(motion_node("a", 8, true, 4), true)],
            None,
            false,
        );
        clock::advance(Duration::from_millis(250));
        m.sync_ungated(
            &[pane_with(motion_node("b", 8, true, 4), true)],
            None,
            false,
        );

        let table = m.table_for("demo", "board").unwrap();
        assert_eq!(table.frame("b"), 0, "a new identity is a new animation");
        assert_eq!(m.tracked(), 1, "the old identity is gone");
    }

    #[test]
    fn hiding_a_pane_drops_its_lease_and_its_state() {
        let mut m = MotionState::default();
        m.sync_ungated(
            &[pane_with(motion_node("s", 8, true, 4), true)],
            None,
            false,
        );
        assert!(m.has_lease());
        assert_eq!(m.tracked(), 1);

        m.sync_ungated(
            &[pane_with(motion_node("s", 8, true, 4), false)],
            None,
            false,
        );
        assert!(!m.has_lease(), "a hidden pane holds no lease");
        assert_eq!(m.tracked(), 0, "and keeps no state");
    }

    #[test]
    fn a_tree_without_motion_drops_the_lease() {
        let mut m = MotionState::default();
        m.sync_ungated(
            &[pane_with(motion_node("s", 8, true, 4), true)],
            None,
            false,
        );
        assert!(m.has_lease());

        m.sync_ungated(&[pane_with(ViewNode::text("still"), true)], None, false);
        assert!(!m.has_lease());
        assert_eq!(m.tracked(), 0);
    }

    #[test]
    fn a_finished_non_repeating_cycle_drops_the_lease_but_keeps_its_frame() {
        let mut m = MotionState::default();
        let panes = vec![pane_with(motion_node("once", 8, false, 3), true)];
        m.sync_ungated(&panes, None, false);
        let granted = m.counters().granted;
        assert_eq!(granted, 1);

        clock::advance(Duration::from_secs(5));
        m.sync_ungated(&panes, None, false);
        assert_eq!(
            m.table_for("demo", "board").unwrap().frame("once"),
            2,
            "the last frame stays on screen"
        );
        // No new lease is granted for a finished animation, however many more
        // syncs run.
        m.sync_ungated(&panes, None, false);
        assert_eq!(m.counters().granted, granted);
    }

    #[test]
    fn repeated_pushes_do_not_grow_the_state() {
        let mut m = MotionState::default();
        for _ in 0..50 {
            m.sync_ungated(
                &[pane_with(motion_node("s", 8, true, 4), true)],
                None,
                false,
            );
        }
        assert_eq!(m.tracked(), 1, "one entry per animated node, not per push");
    }

    #[test]
    fn reduced_motion_renders_frame_zero_and_grants_nothing() {
        let mut m = MotionState::default();
        let panes = vec![pane_with(motion_node("s", 8, true, 4), true)];
        m.sync_ungated(&panes, None, true);
        clock::advance(Duration::from_secs(1));
        let changed = m.sync_ungated(&panes, None, true);

        assert!(!changed, "a suppressed motion never repaints");
        assert!(!m.has_lease());
        assert_eq!(
            m.table_for("demo", "board").map(|t| t.frame("s")),
            None,
            "an absent table reads as frame 0"
        );
        assert!(m.counters().denied > 0);
        assert_eq!(m.counters().granted, 0);
    }

    #[test]
    fn a_frame_table_changes_only_when_the_frame_does() {
        let mut m = MotionState::default();
        let panes = vec![pane_with(motion_node("s", 8, true, 4), true)];
        assert!(
            m.sync_ungated(&panes, None, false),
            "the first table is a change"
        );

        // 8 fps is one frame per 125 ms; a 10 ms tick must not repaint.
        clock::advance(Duration::from_millis(10));
        assert!(
            !m.sync_ungated(&panes, None, false),
            "same frame, no repaint"
        );

        clock::advance(Duration::from_millis(120));
        assert!(m.sync_ungated(&panes, None, false), "the frame moved");
    }

    #[test]
    fn one_lease_covers_every_animated_node_in_a_pane() {
        let mut m = MotionState::default();
        let tree = ViewNode::Column(vec![
            motion_node("a", 8, true, 2),
            motion_node("b", 8, true, 2),
            motion_node("c", 8, true, 2),
        ]);
        m.sync_ungated(&[pane_with(tree, true)], None, false);
        assert_eq!(m.tracked(), 3, "three nodes");
        assert_eq!(m.counters().granted, 1, "one lease");
    }

    #[test]
    fn a_hidden_pane_costs_no_frames() {
        let mut m = MotionState::default();
        let panes = vec![pane_with(motion_node("s", 8, true, 4), false)];
        for _ in 0..20 {
            clock::advance(Duration::from_millis(200));
            assert!(!m.sync_ungated(&panes, None, false));
        }
        assert_eq!(m.counters().frames, 0);
        assert!(m.counters().denied > 0, "and says why");
    }
}
