//! Which plugin panes the kernel is keeping off screen, as a published set.
//!
//! Pure data (no local crate imports beyond `super`), matching the `session/`
//! architecture rule. Visibility is kernel state living in `app`; the thing that
//! needs to know it is the plugin-render worker, which holds the host on another
//! thread and may not reach `storage`. So `app` [`publish_hidden`]es the set and
//! `plugin` asks [`is_hidden`] before entering a VM. The mechanism mirrors
//! [`super::pane_context`], which publishes the kernel state a pane may *read*;
//! this publishes whether it is worth reading at all.
//!
//! ## The hidden set, not the visible set
//!
//! The reader's question is "may I skip this pane?", and for a pane nobody has
//! told it about the safe answer is no — a pane that went unpublished must still
//! be drawn. Publishing the *hidden* ones makes that the structure rather than a
//! rule to remember: an empty publication means "hide nothing", which is exactly
//! how a process with no publisher (a short-lived `thurbox-cli` command) must
//! behave.

/// One pane the kernel is currently keeping off screen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HiddenPane {
    /// The plugin that declared it.
    pub plugin: String,
    /// The pane's id within that plugin.
    pub pane: String,
}

impl HiddenPane {
    /// A hidden pane, from its owning plugin and its pane id.
    pub fn new(plugin: impl Into<String>, pane: impl Into<String>) -> Self {
        Self {
            plugin: plugin.into(),
            pane: pane.into(),
        }
    }
}

/// The process-wide hidden set.
///
/// A `RwLock` for the same reason [`super::pane_context`] uses one: the writer is
/// the UI thread and the reader is a plugin worker, and neither may block on the
/// other for longer than it takes to replace or scan a short list.
static HIDDEN: std::sync::RwLock<Vec<HiddenPane>> = std::sync::RwLock::new(Vec::new());

/// Whether any running plugin declares a pane at all.
///
/// Set by the plugin host from what it is running, read by the publisher: with no
/// pane declared there is nothing to hide, so the tick pays one relaxed load and
/// returns. A build without the plugin host never sets it.
static PANES_PRESENT: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Publish the set of panes to keep off screen, replacing whatever was there.
pub fn publish_hidden(panes: Vec<HiddenPane>) {
    if let Ok(mut slot) = HIDDEN.write() {
        *slot = panes;
    }
}

/// Whether this pane is one the kernel is keeping off screen.
///
/// A pane absent from the published set is **not** hidden, so a host running
/// before the first publication — or in a process that never publishes — renders
/// every pane exactly as it did before this slot existed.
pub fn is_hidden(plugin: &str, pane: &str) -> bool {
    HIDDEN
        .read()
        .map(|slot| slot.iter().any(|h| h.plugin == plugin && h.pane == pane))
        .unwrap_or(false)
}

/// What is published, for a caller that wants the whole set.
pub fn hidden() -> Vec<HiddenPane> {
    HIDDEN.read().map(|slot| slot.clone()).unwrap_or_default()
}

/// Record whether any running plugin declares a pane.
pub fn set_panes_present(present: bool) {
    PANES_PRESENT.store(present, std::sync::atomic::Ordering::Relaxed);
}

/// Whether publishing a hidden set is worth doing at all.
pub fn panes_present() -> bool {
    PANES_PRESENT.load(std::sync::atomic::Ordering::Relaxed)
}

/// Serializes every test that touches a process-wide plugin-boundary slot.
///
/// Deliberately the *same* lock [`super::pane_context`] hands out rather than a
/// second one: one tick publishes both slots, so a test driving a tick can
/// disturb either, and two locks would not serialize against each other — which
/// is the bug the lock exists to prevent.
#[cfg(test)]
pub(crate) fn test_lock() -> std::sync::MutexGuard<'static, ()> {
    super::pane_context::test_lock()
}

/// Return both slots to their pristine state.
#[cfg(test)]
pub(crate) fn clear_for_test() {
    publish_hidden(Vec::new());
    set_panes_present(false);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pane_visibility_unknown_pane_is_not_hidden() {
        let _guard = test_lock();
        clear_for_test();
        assert!(
            !is_hidden("info-panel", "info"),
            "nothing published means nothing hidden, so every pane draws"
        );
    }

    #[test]
    fn pane_visibility_publication_replaces_the_set() {
        let _guard = test_lock();
        clear_for_test();

        publish_hidden(vec![HiddenPane::new("hello", "board")]);
        assert!(is_hidden("hello", "board"));
        assert!(!is_hidden("hello", "other"), "another pane is unaffected");
        assert!(!is_hidden("other", "board"), "another plugin is unaffected");

        // A publication is a replacement, not a merge: the pane that dropped out
        // of the set is visible again.
        publish_hidden(vec![HiddenPane::new("info-panel", "info")]);
        assert!(!is_hidden("hello", "board"));
        assert!(is_hidden("info-panel", "info"));

        clear_for_test();
    }

    #[test]
    fn pane_visibility_demand_flag_round_trips() {
        let _guard = test_lock();
        clear_for_test();
        assert!(!panes_present(), "no plugin host has spoken");
        set_panes_present(true);
        assert!(panes_present());
        clear_for_test();
        assert!(!panes_present());
    }
}
