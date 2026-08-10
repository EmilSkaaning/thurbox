//! The kernel state a plugin pane is allowed to read, as a published snapshot.
//!
//! Pure data (no local crate imports beyond `super`), matching the `session/`
//! architecture rule. `app` builds a [`PaneContext`] and [`publish`]es it;
//! `plugin` reads the published value when a plugin calls one of its
//! capability-gated readers. Neither ever holds a reference to the other, which
//! is what keeps plugin code off the thread that draws and keeps the plugin
//! host out of `app`'s type graph. The mechanism mirrors
//! [`super::spawn_contribution`], which publishes a registry the spawn path
//! reads.
//!
//! ## What a snapshot carries, and what it deliberately does not
//!
//! A plugin VM loads no `os` and no path library, so it has **no clock and no
//! filesystem**. The snapshot therefore resolves exactly what a plugin cannot
//! compute:
//!
//! - a duration to an event, never an absolute instant ([`UsageWindowSnapshot::resets_in_secs`]);
//! - a directory's display name, never a path ([`SessionSnapshot::repo_name`]);
//! - a referenced record's name, never only its id ([`SessionSnapshot::parent_name`]);
//! - and, for a status, the glyph and style token the kernel draws it with
//!   ([`StatusSnapshot`]) — because `super::view_tree::StyleToken::for_status`
//!   exists so two panes cannot disagree about that mapping, and a plugin
//!   re-deriving it would be a second, unchecked copy.
//!
//! Everything else is a **number**. Byte counts, token counts, durations, costs
//! and percentages are published raw and the plugin composes every string it
//! displays. Publishing `"8.0/16.0 GB"` would make a pane plugin an arrangement
//! of strings the kernel formatted, which would prove nothing about what a
//! third-party pane can do.

/// One session status, in the three forms a pane needs to draw it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusSnapshot {
    /// Stable wire name (`working`, `blocked`, …), for a plugin branching on it.
    pub name: &'static str,
    /// The label thurbox shows (`Working`, `Blocked`, …).
    pub label: String,
    /// The glyph thurbox draws.
    pub icon: &'static str,
    /// The wire name of the style token the kernel resolves this status to.
    pub token: &'static str,
}

impl StatusSnapshot {
    /// Describe `status` the way the kernel draws it.
    pub fn of(status: super::SessionStatus) -> StatusSnapshot {
        StatusSnapshot {
            name: match status {
                super::SessionStatus::Working => "working",
                super::SessionStatus::Blocked => "blocked",
                super::SessionStatus::Done => "done",
                super::SessionStatus::Idle => "idle",
                super::SessionStatus::Error => "error",
                super::SessionStatus::Unreachable => "unreachable",
            },
            label: status.to_string(),
            icon: status.icon(),
            token: super::view_tree::StyleToken::for_status(status).as_str(),
        }
    }
}

/// A session's git change summary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitSnapshot {
    /// Tracked files with staged/unstaged changes.
    pub files_changed: u64,
    /// Lines added.
    pub insertions: u64,
    /// Lines removed.
    pub deletions: u64,
    /// Whether anything is uncommitted.
    pub dirty: bool,
    /// Commits ahead of the base.
    pub ahead: u64,
    /// Commits behind the base.
    pub behind: u64,
}

/// What the session's agent reports about itself.
///
/// Every field is optional because an agent reports what it chooses to; a pane
/// omits the row when its value is absent.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct AgentMetricsSnapshot {
    /// Model name the agent is running.
    pub model_display_name: Option<String>,
    /// The agent CLI's version.
    pub cli_version: Option<String>,
    /// Spend so far, in USD.
    pub total_cost_usd: Option<f64>,
    /// Wall-clock time the agent has spent, in milliseconds.
    pub total_duration_ms: Option<u64>,
    /// Of which, time in API calls.
    pub total_api_duration_ms: Option<u64>,
    /// Lines the agent added.
    pub total_lines_added: Option<u64>,
    /// Lines the agent removed.
    pub total_lines_removed: Option<u64>,
    /// Input tokens consumed.
    pub total_input_tokens: Option<u64>,
    /// Output tokens produced.
    pub total_output_tokens: Option<u64>,
    /// Size of the agent's context window, in tokens.
    pub context_window_size: Option<u64>,
    /// How much of that window is used, as a whole percentage.
    pub used_percentage: Option<u8>,
    /// Tokens read from the prompt cache.
    pub cache_read_input_tokens: Option<u64>,
    /// Tokens written to the prompt cache.
    pub cache_creation_input_tokens: Option<u64>,
}

/// One account rate-limit window.
#[derive(Debug, Clone, PartialEq)]
pub struct UsageWindowSnapshot {
    /// Short label, e.g. `5h` or `Week`.
    pub label: String,
    /// Percent of the window consumed.
    pub used_percent: f32,
    /// Seconds until it resets, resolved at publication because a plugin has no
    /// clock. `None` when the reset time is unknown.
    pub resets_in_secs: Option<u64>,
}

/// Account-level usage for the session's agent, on the host it runs on.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct UsageSnapshot {
    /// One entry per rate-limit window.
    pub windows: Vec<UsageWindowSnapshot>,
    /// Plan or tier label, when the vendor reports one.
    pub plan: Option<String>,
    /// Human note shown when there are no windows (not logged in, API error).
    pub note: Option<String>,
}

/// The session the user is currently on.
#[derive(Debug, Clone, PartialEq)]
pub struct SessionSnapshot {
    /// The session's id, as text.
    pub id: String,
    /// Its name.
    pub name: String,
    /// Its status, in drawable form.
    pub status: StatusSnapshot,
    /// The agent driving it.
    pub agent: String,
    /// The parent session's name for a worker session, resolved here because a
    /// plugin cannot look one id up against another record. Falls back to a
    /// shortened id when the parent is gone.
    pub parent_name: Option<String>,
    /// Bare host name for a remote session; `None` when local.
    pub remote_host: Option<String>,
    /// Why hook-driven status is degraded, when it is.
    pub hook_wiring: Option<String>,
    /// Latest activity text the agent emitted.
    pub activity: Option<String>,
    /// Latest attention notification the agent emitted.
    pub notification: Option<String>,
    /// Display name of the primary repository, already reduced from a path.
    pub repo_name: Option<String>,
    /// Branch of the primary worktree.
    pub branch: Option<String>,
    /// Display names of the session's additional directories.
    pub additional_dir_names: Vec<String>,
    /// Git change summary, when it has been computed.
    pub git: Option<GitSnapshot>,
    /// What the agent reports, when it reports anything.
    pub agent_metrics: Option<AgentMetricsSnapshot>,
    /// Account usage for this session's agent and host.
    pub usage: Option<UsageSnapshot>,
}

/// This machine's resource usage, and thurbox's own footprint.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SystemSnapshot {
    /// Overall CPU usage, 0–100.
    pub cpu_percent: f32,
    /// RAM in use, in bytes.
    pub memory_used: u64,
    /// RAM installed, in bytes.
    pub memory_total: u64,
    /// The active session's CPU usage, 0–100 and beyond on a multi-core box.
    pub session_cpu_percent: f32,
    /// The active session's resident memory, in bytes.
    pub session_memory_bytes: u64,
    /// Size of thurbox's data directory, when it has been measured.
    pub thurbox_dir_bytes: Option<u64>,
}

/// One scheduled automation that has not fired yet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AutomationSnapshot {
    /// Its name, already truncated to the width a pane shows.
    pub label: String,
    /// Whole seconds until it is due, resolved at publication.
    ///
    /// Seconds rather than milliseconds because that is the granularity the
    /// countdown is *displayed* at: publishing milliseconds would carry no extra
    /// information and would make the snapshot differ on every tick, defeating
    /// the change gate that keeps a pending automation from writing the slot a
    /// hundred times a second.
    pub due_in_secs: u64,
}

/// Everything a plugin may read about the kernel's current state.
///
/// One value rather than three published separately: every section describes the
/// *same instant*, so a pane reading two of them cannot render a session against
/// another moment's metrics.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PaneContext {
    /// The active session, or `None` when there is none.
    pub session: Option<SessionSnapshot>,
    /// Host resource metrics, or `None` before the first sample.
    pub system: Option<SystemSnapshot>,
    /// Automations due to fire, soonest first.
    pub automations: Vec<AutomationSnapshot>,
}

/// The process-wide snapshot slot.
///
/// A `RwLock` rather than a channel because the reader is a plugin worker that
/// must not block on the UI thread and the writer is the UI thread, which must
/// not block on a plugin. Both sides do bounded work: the writer replaces a
/// value, the reader clones one.
static CONTEXT: std::sync::RwLock<Option<PaneContext>> = std::sync::RwLock::new(None);

/// Whether any running plugin can read kernel state.
///
/// Set by the plugin host from the capabilities it granted, read by the
/// publisher. A build with no plugin host never sets it, so publishing is one
/// relaxed load and a return.
static READERS: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Publish the snapshot for this process, replacing whatever was there.
pub fn publish(context: PaneContext) {
    if let Ok(mut slot) = CONTEXT.write() {
        *slot = Some(context);
    }
}

/// What is published, or `None` when nothing is — the normal state of a build
/// without a plugin host, and what a reader sees before the first tick.
pub fn published() -> Option<PaneContext> {
    CONTEXT.read().ok().and_then(|slot| slot.clone())
}

/// Record whether any running plugin holds a state-reading capability.
pub fn set_readers_present(present: bool) {
    READERS.store(present, std::sync::atomic::Ordering::Relaxed);
}

/// Whether building a snapshot is worth doing at all.
pub fn readers_present() -> bool {
    READERS.load(std::sync::atomic::Ordering::Relaxed)
}

/// Serializes every test that touches the two process-wide slots.
///
/// `cargo test` runs a suite in threads and both slots are global, so a snapshot
/// or a demand flag left by one test would make another fail for a reason that
/// has nothing to do with it. One lock for the whole crate rather than one per
/// test module, because two locks would not serialize against each other —
/// which is the bug the lock exists to prevent.
#[cfg(test)]
pub(crate) fn test_lock() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

/// Return the slot to its pristine "nothing published" state.
#[cfg(test)]
pub(crate) fn clear_for_test() {
    if let Ok(mut slot) = CONTEXT.write() {
        *slot = None;
    }
    set_readers_present(false);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::SessionStatus;

    fn context_with_name(name: &str) -> PaneContext {
        PaneContext {
            session: Some(SessionSnapshot {
                id: "id".to_string(),
                name: name.to_string(),
                status: StatusSnapshot::of(SessionStatus::Idle),
                agent: "claude".to_string(),
                parent_name: None,
                remote_host: None,
                hook_wiring: None,
                activity: None,
                notification: None,
                repo_name: None,
                branch: None,
                additional_dir_names: Vec::new(),
                git: None,
                agent_metrics: None,
                usage: None,
            }),
            system: None,
            automations: Vec::new(),
        }
    }

    #[test]
    fn a_publication_is_readable_and_replaceable() {
        let _guard = test_lock();
        publish(context_with_name("first"));
        assert_eq!(
            published().unwrap().session.unwrap().name,
            "first",
            "a published snapshot is what a reader sees"
        );
        publish(context_with_name("second"));
        assert_eq!(published().unwrap().session.unwrap().name, "second");
    }

    #[test]
    fn reader_demand_round_trips() {
        let _guard = test_lock();
        set_readers_present(true);
        assert!(readers_present());
        set_readers_present(false);
        assert!(!readers_present());
    }

    /// The change gate compares whole snapshots, so equality has to be
    /// structural — a snapshot that compared by identity would publish on every
    /// tick and defeat the gate.
    #[test]
    fn equal_snapshots_compare_equal() {
        assert_eq!(context_with_name("a"), context_with_name("a"));
        assert_ne!(context_with_name("a"), context_with_name("b"));
    }

    #[test]
    fn every_status_is_describable_and_names_its_own_token() {
        for status in [
            SessionStatus::Working,
            SessionStatus::Blocked,
            SessionStatus::Done,
            SessionStatus::Idle,
            SessionStatus::Error,
            SessionStatus::Unreachable,
        ] {
            let snap = StatusSnapshot::of(status);
            assert_eq!(snap.label, status.to_string());
            assert_eq!(snap.icon, status.icon());
            // The kernel's own mapping, not a spelling a plugin could guess:
            // the two must be the same function or a plugin's status dot can
            // drift from the session list's.
            assert_eq!(
                snap.token,
                crate::session::view_tree::StyleToken::for_status(status).as_str()
            );
            assert!(!snap.name.is_empty());
        }
    }

    /// Two different statuses must not describe themselves identically, or a
    /// pane branching on `name` would conflate them.
    #[test]
    fn status_names_are_distinct() {
        let names: Vec<&str> = [
            SessionStatus::Working,
            SessionStatus::Blocked,
            SessionStatus::Done,
            SessionStatus::Idle,
            SessionStatus::Error,
            SessionStatus::Unreachable,
        ]
        .into_iter()
        .map(|s| StatusSnapshot::of(s).name)
        .collect();
        let mut sorted = names.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), names.len(), "{names:?}");
    }
}
