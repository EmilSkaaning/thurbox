//! What a plugin pane is showing right now.
//!
//! A pane never goes blank. While a re-render is in flight it keeps the last
//! tree it successfully produced; when a render fails it keeps that tree and
//! adds an error indicator. Clearing on failure would turn a transient plugin
//! bug into a flickering pane — worse to look at and worse to debug than stale
//! content plus an explicit marker.
//!
//! Only a pane that has *never* rendered shows a loading state, and only one
//! whose very first render failed shows a bare error.

use crate::session::plugin_manifest::PaneSlot;
use crate::session::view_tree::ViewNode;

/// A plugin pane and everything needed to draw it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginPane {
    /// The plugin that owns it.
    pub plugin: String,
    /// The pane's id within that plugin.
    pub id: String,
    /// Title for the pane border.
    pub title: String,
    /// Where it sits.
    pub slot: PaneSlot,
    /// Whether the pane is currently on screen.
    ///
    /// Kernel state: the manifest seeds it and the user owns it thereafter, so
    /// a plugin cannot put its own pane back on screen.
    pub visible: bool,
    /// Whether the owning plugin declared the `input` capability.
    ///
    /// Carried on the pane so the UI can decide focusability without holding
    /// the host — the host publishes it alongside the pane set.
    pub accepts_input: bool,
    /// What it is currently showing.
    pub presentation: PanePresentation,
}

impl PluginPane {
    /// A pane that has not rendered yet, visible per its seed.
    pub fn loading(plugin: &str, id: &str, title: &str, slot: PaneSlot, visible: bool) -> Self {
        Self {
            plugin: plugin.to_string(),
            id: id.to_string(),
            title: title.to_string(),
            slot,
            visible,
            accepts_input: false,
            presentation: PanePresentation::Loading,
        }
    }

    /// Whether this pane can take focus: it must be on screen, and its plugin
    /// must have asked for input.
    pub fn is_focusable(&self) -> bool {
        self.visible && self.accepts_input
    }

    /// Apply a render result.
    ///
    /// Returns whether what the user sees changed — the caller marks the UI
    /// dirty only when it did. A plugin that re-renders to the same tree must
    /// not cost a repaint, or a single installed plugin would undo the
    /// demand-driven loop for everyone.
    pub fn apply(&mut self, result: Result<ViewNode, String>) -> bool {
        let next = match (result, &self.presentation) {
            (Ok(tree), _) => PanePresentation::Ready(tree),
            // A failure after a good render keeps the good render.
            (Err(error), PanePresentation::Ready(tree))
            | (Err(error), PanePresentation::Stale { tree, .. }) => PanePresentation::Stale {
                tree: tree.clone(),
                error,
            },
            // Nothing good has ever been shown, so there is nothing to keep.
            (Err(error), PanePresentation::Loading) | (Err(error), PanePresentation::Failed(_)) => {
                PanePresentation::Failed(error)
            }
        };

        let changed = next != self.presentation;
        self.presentation = next;
        changed
    }

    /// The tree to draw, if there is one.
    pub fn tree(&self) -> Option<&ViewNode> {
        match &self.presentation {
            PanePresentation::Ready(tree) | PanePresentation::Stale { tree, .. } => Some(tree),
            PanePresentation::Loading | PanePresentation::Failed(_) => None,
        }
    }

    /// The error to surface, if the last render failed.
    pub fn error(&self) -> Option<&str> {
        match &self.presentation {
            PanePresentation::Stale { error, .. } | PanePresentation::Failed(error) => Some(error),
            PanePresentation::Loading | PanePresentation::Ready(_) => None,
        }
    }
}

/// What a pane currently has to show.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PanePresentation {
    /// Never rendered; nothing to draw yet.
    Loading,
    /// Showing a tree from a successful render.
    Ready(ViewNode),
    /// Showing the last good tree, with the failure that followed it.
    Stale {
        /// The last tree that rendered successfully.
        tree: ViewNode,
        /// Why the most recent attempt failed.
        error: String,
    },
    /// Never rendered successfully, and the last attempt failed.
    Failed(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pane() -> PluginPane {
        PluginPane::loading("demo", "board", "Board", PaneSlot::Right, true)
    }

    fn tree(text: &str) -> ViewNode {
        ViewNode::list(vec![ViewNode::text(text)])
    }

    #[test]
    fn a_new_pane_is_loading() {
        let p = pane();
        assert_eq!(p.presentation, PanePresentation::Loading);
        assert!(p.tree().is_none());
        assert!(p.error().is_none());
    }

    #[test]
    fn a_successful_render_becomes_ready() {
        let mut p = pane();
        assert!(p.apply(Ok(tree("one"))), "first content is a change");
        assert_eq!(p.tree(), Some(&tree("one")));
        assert!(p.error().is_none());
    }

    #[test]
    fn an_unchanged_tree_is_not_a_change() {
        let mut p = pane();
        p.apply(Ok(tree("same")));
        assert!(
            !p.apply(Ok(tree("same"))),
            "re-rendering to the same tree must not cost a repaint"
        );
    }

    #[test]
    fn a_different_tree_is_a_change() {
        let mut p = pane();
        p.apply(Ok(tree("one")));
        assert!(p.apply(Ok(tree("two"))));
        assert_eq!(p.tree(), Some(&tree("two")));
    }

    #[test]
    fn a_failure_after_success_keeps_the_last_good_tree() {
        let mut p = pane();
        p.apply(Ok(tree("good")));
        assert!(p.apply(Err("boom".to_string())));

        assert_eq!(p.tree(), Some(&tree("good")), "content must survive");
        assert_eq!(p.error(), Some("boom"));
    }

    #[test]
    fn a_repeated_failure_is_not_a_change() {
        let mut p = pane();
        p.apply(Ok(tree("good")));
        p.apply(Err("boom".to_string()));
        assert!(
            !p.apply(Err("boom".to_string())),
            "the same failure must not repaint every tick"
        );
    }

    #[test]
    fn recovering_from_a_failure_clears_the_error() {
        let mut p = pane();
        p.apply(Ok(tree("good")));
        p.apply(Err("boom".to_string()));
        assert!(p.apply(Ok(tree("good"))));

        assert_eq!(p.presentation, PanePresentation::Ready(tree("good")));
        assert!(p.error().is_none());
    }

    #[test]
    fn a_first_render_failure_shows_failed_not_loading_forever() {
        let mut p = pane();
        assert!(p.apply(Err("nope".to_string())));

        assert_eq!(p.presentation, PanePresentation::Failed("nope".to_string()));
        assert!(p.tree().is_none());
        assert_eq!(p.error(), Some("nope"));
    }

    #[test]
    fn a_pane_recovers_from_a_failed_first_render() {
        let mut p = pane();
        p.apply(Err("nope".to_string()));
        assert!(p.apply(Ok(tree("finally"))));
        assert_eq!(p.tree(), Some(&tree("finally")));
        assert!(p.error().is_none());
    }

    #[test]
    fn a_new_error_over_a_stale_pane_is_a_change() {
        let mut p = pane();
        p.apply(Ok(tree("good")));
        p.apply(Err("first".to_string()));
        assert!(p.apply(Err("second".to_string())));
        assert_eq!(p.error(), Some("second"));
        assert_eq!(p.tree(), Some(&tree("good")));
    }
}
