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

/// One row of the task list, as a pane draws it.
///
/// Four of the six fields are view facts rather than stored ones, and they are
/// here because the *kernel* owns them: it owns the keyboard that moves the
/// selection and the search that dims and matches, and a task's link to a live
/// session is a join across records — the same class of thing as
/// [`SessionSnapshot::parent_name`]. What is deliberately **not** here is the
/// glyph and the colour: unlike a session status (see [`StatusSnapshot`], whose
/// mapping is shared by two native panes and so must not be re-derived), a task
/// status is drawn in exactly one place, so publishing its rendering would hand a
/// pane the presentation it exists to own.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskSnapshot {
    /// The task's title, as the model knows it.
    ///
    /// Not fitted to any column: a pane's width is resolved during a frame and
    /// this is published on the tick, and the plugin's pane is a different rect
    /// from the native one — rows fitted to someone else's width would be wrong
    /// at their own.
    pub title: String,
    /// Stable wire name of the task's status (`todo`, `in_progress`, `done`).
    pub status: &'static str,
    /// Whether this is the row the user's cursor is on, resolved here because
    /// the selection also moves under a global-search preview.
    pub selected: bool,
    /// Whether a running search filtered this row out.
    pub dimmed: bool,
    /// Whether the task has at least one open related session.
    pub linked: bool,
    /// Byte offsets in `title` a running search matched. Empty when no search is
    /// running or this row did not match; the pane decides how a matched run is
    /// emphasised.
    pub match_positions: Vec<usize>,
}

/// The task list a pane draws.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TasksSnapshot {
    /// One entry per row, in the order the kernel lists them.
    pub entries: Vec<TaskSnapshot>,
    /// Whether the task pane holds focus, which is the one thing besides the
    /// rows that changes what is drawn: the empty-state line names the key that
    /// adds a task only when the pane can receive it.
    pub focused: bool,
}

/// Most task rows a publication carries.
///
/// A bound on the *section*, not on a consumer, because the cost it prevents is
/// the consumer's: a view tree is capped at
/// [`super::view_tree::MAX_NODES`] nodes and a row costs several, so a
/// thousand-task list would make every render of a task pane fail rather than
/// merely scroll. No pane can show this many rows on any terminal, so the bound
/// costs nothing that is visible.
pub const MAX_TASK_ROWS: usize = 200;

/// One row of the file tree, as a pane draws it.
///
/// The section it belongs to is the narrowest state channel in the snapshot, and
/// deliberately so: see [`FilesSnapshot`] for what reading it does **not**
/// confer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileNodeSnapshot {
    /// The node's **basename** — never a path.
    ///
    /// A path is not needed to draw the pane (the native one shows basenames
    /// too), and a plugin holding absolute worktree paths would be a materially
    /// larger disclosure than one holding names. Depth plus name lets a plugin
    /// reconstruct the tree's *shape*, which is inherent to drawing a tree, and
    /// reveals nothing about where on disk the tree is.
    pub name: String,
    /// How deep in the tree it sits; a root is 0. Drives indentation.
    pub depth: usize,
    /// Whether it is a directory.
    pub is_dir: bool,
    /// Whether it is expanded — the user's navigation, which is why the kernel
    /// resolves it and why a plugin listing directories itself could not draw
    /// this pane.
    pub expanded: bool,
    /// Whether a running search matched this row's name.
    ///
    /// `true` when no search is running, so an unsearched tree draws in its
    /// ordinary colours without the pane having to know whether a search
    /// exists. The *verdict* crosses and the query does not: the plugin needs no
    /// matcher, so its case folding cannot drift from the kernel's.
    pub matched: bool,
}

/// The file tree a pane draws — the rows thurbox's file viewer currently has
/// open.
///
/// **This is not a filesystem capability.** Reading it lets a plugin list no
/// directory, read no file, stat no path, and cause no I/O whatsoever: the
/// section is built from a tree the kernel already holds, whose shape is a record
/// of what the *user* expanded. A directory the user has not opened is not in it,
/// a dotfile never entered it, and nothing outside the active session's own
/// directories can appear.
///
/// That is a narrower grant than "read a file tree" suggests, and it is narrower
/// on purpose. Of the five facts a row carries only its name comes from disk;
/// depth and expansion are the user's navigation, the match verdict is a search
/// the kernel runs, and the cursor is the keyboard's. A plugin holding `read_dir`
/// could draw *a* tree but not *this pane*, so the wider power would buy strictly
/// less result.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FilesSnapshot {
    /// One entry per visible row, in the order the pane lists them.
    pub nodes: Vec<FileNodeSnapshot>,
    /// Which row the cursor is on, as a zero-based index into `nodes`.
    ///
    /// One index rather than a flag per row, because that is the form
    /// [`super::view_tree::ViewNode::List`]'s selected row takes — the pane hands
    /// it straight back and the kernel scrolls to it. `None` when there are no
    /// rows, or when the cursor is on a row past [`MAX_FILE_ROWS`]: an index into
    /// rows that were not published would make that windowing meaningless.
    pub selected: Option<usize>,
    /// Whether nerd-font glyphs are enabled, so the pane can pick between its two
    /// marker sets.
    ///
    /// A display *setting*, not a filesystem fact, and it rides here because the
    /// file tree's markers are its only consumer in thurbox. The kernel publishes
    /// the fact rather than the glyph — the rule being that a rendering is
    /// published only when two panes must agree about it, and this mapping has one
    /// consumer. A second consumer should lift this to its own section under its
    /// own capability rather than a copy appearing.
    pub nerd_font: bool,
}

/// Most file rows a publication carries.
///
/// A bound on the *section*, like [`MAX_TASK_ROWS`], and for the same reason: a
/// view tree is capped at [`super::view_tree::MAX_NODES`] nodes and a row costs
/// three, so a tree with a large directory expanded in it would make every render
/// of a file pane **fail** rather than merely scroll. Unlike a task list this can
/// plausibly be exceeded — one `node_modules` does it — so the cost is real and
/// named: past the bound the section carries the first rows and no cursor, and the
/// pane stops scrolling rather than scrolling to a row it does not have.
pub const MAX_FILE_ROWS: usize = 1_000;

/// One line of the open review's diff, as a pane draws it.
///
/// The line's text crosses **raw**: not split into syntax-highlighted runs, not
/// windowed to a horizontal scroll offset, and not padded to any width. How a
/// diff body is coloured is the pane's decision — `crate::ui::code_review` is
/// the only reader of `crate::ui::syntax` in thurbox, so by the rule that a
/// rendering crosses only when two panes must agree about it, this one does not
/// cross at all. A pane arranging runs the kernel had already coloured would be
/// evidence about nothing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewLineSnapshot {
    /// The path of the file this line belongs to, as the diff names it.
    ///
    /// Published because a diff line is not addressable without it, and because
    /// the pane picks its comment style from the extension. This is the diff's
    /// own subject, not a filesystem path a plugin could follow: nothing here
    /// reads it.
    pub path: String,
    /// Its number on the old side, absent on an insertion.
    pub old_no: Option<u32>,
    /// Its number on the new side, absent on a deletion.
    pub new_no: Option<u32>,
    /// Stable wire name of its kind: `add`, `del` or `context`.
    pub kind: &'static str,
    /// The line's text, without its leading diff sign.
    pub text: String,
}

/// The open review's diff stream.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ReviewSnapshot {
    /// One entry per published line, in the order the pane lists them.
    pub lines: Vec<ReviewLineSnapshot>,
    /// The row a list scrolls to, zero-based into `lines`. `None` when there are
    /// none, or when the cursor falls past [`MAX_REVIEW_ROWS`] — for
    /// [`FilesSnapshot::selected`]'s reason.
    pub cursor: Option<usize>,
    /// Width each of the gutter's two number columns is drawn at.
    ///
    /// Published rather than derived by the pane because it is computed over
    /// **every** hunk of **every** file in the review, which a bounded window of
    /// lines does not contain: a pane deriving it from what it received would
    /// draw a narrower gutter than the review's own, and the two copies of the
    /// pane would not line up.
    pub number_width: usize,
}

/// Most diff lines a publication carries.
///
/// A bound on the section like [`MAX_FILE_ROWS`], but for a **different reason**,
/// and the difference is the finding rather than an implementation note. The
/// other sections bound a row count because a pane draws a bounded number of
/// rows, and each row costs a fixed handful of nodes. A diff line's cost is
/// *unbounded*: its body is one node per syntax token, so a single dense line can
/// cost thirty. [`super::view_tree::MAX_NODES`] is a whole-tree budget, so no row
/// cap can guarantee a diff pane stays inside it — which makes this the first
/// pane whose content the model cannot bound locally.
///
/// The number is chosen so that a representative row (a gutter, a fill, and a
/// dozen token runs) leaves the budget comfortable, not so that a pathological
/// one is impossible. Past the bound the section carries the first lines and no
/// cursor.
pub const MAX_REVIEW_ROWS: usize = 60;

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
    /// The task list, empty when there are no tasks or the feature is off.
    pub tasks: TasksSnapshot,
    /// The file tree the file viewer has open, empty when it has none or the
    /// feature is off.
    pub files: FilesSnapshot,
    /// The diff the code-review view has open, empty when it has none or the
    /// feature is off.
    pub review: ReviewSnapshot,
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
            tasks: TasksSnapshot::default(),
            files: FilesSnapshot::default(),
            review: ReviewSnapshot::default(),
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

    /// A task row's status crosses as the name the database stores, so a pane
    /// branching on it and a row read back out of SQLite agree — and so the
    /// vocabulary cannot drift into a second spelling.
    #[test]
    fn a_task_rows_status_is_the_stored_wire_name() {
        for status in [
            crate::session::TaskStatus::Todo,
            crate::session::TaskStatus::InProgress,
            crate::session::TaskStatus::Done,
        ] {
            let row = TaskSnapshot {
                title: "t".to_string(),
                status: status.as_str(),
                selected: false,
                dimmed: false,
                linked: false,
                match_positions: Vec::new(),
            };
            assert_eq!(row.status, status.as_str());
        }
    }

    /// The task section takes part in the change gate like every other, so two
    /// equal lists must compare equal — including their match offsets, which is
    /// the field most likely to be rebuilt into an equal-but-new `Vec`.
    #[test]
    fn equal_task_sections_compare_equal() {
        let row = |selected: bool| TaskSnapshot {
            title: "ship it".to_string(),
            status: "todo",
            selected,
            dimmed: false,
            linked: true,
            match_positions: vec![0, 5],
        };
        assert_eq!(
            TasksSnapshot {
                entries: vec![row(false)],
                focused: true
            },
            TasksSnapshot {
                entries: vec![row(false)],
                focused: true
            }
        );
        assert_ne!(
            TasksSnapshot {
                entries: vec![row(false)],
                focused: true
            },
            TasksSnapshot {
                entries: vec![row(true)],
                focused: true
            }
        );
    }

    fn file_row(name: &str, matched: bool) -> FileNodeSnapshot {
        FileNodeSnapshot {
            name: name.to_string(),
            depth: 1,
            is_dir: false,
            expanded: false,
            matched,
        }
    }

    /// The file section takes part in the change gate too, so two equal trees
    /// must compare equal — otherwise an idle file viewer would republish on
    /// every tick.
    #[test]
    fn equal_file_sections_compare_equal() {
        let section = |selected| FilesSnapshot {
            nodes: vec![file_row("src", true), file_row("main.rs", true)],
            selected,
            nerd_font: false,
        };
        assert_eq!(section(Some(0)), section(Some(0)));
        assert_ne!(section(Some(0)), section(Some(1)));
        assert_ne!(section(Some(0)), section(None));
    }

    /// A row carries a basename and no path, which is the boundary the whole
    /// capability rests on — asserted on the type so a `path` field cannot be
    /// added without this failing.
    #[test]
    fn a_file_row_carries_a_name_and_never_a_path() {
        let row = file_row("main.rs", true);
        assert!(
            !row.name.contains(std::path::MAIN_SEPARATOR),
            "a row's name is a basename: {:?}",
            row.name
        );
        // The fields a row has, spelled out: adding one that reveals a location
        // has to be a deliberate edit here as well as to the struct.
        let FileNodeSnapshot {
            name: _,
            depth: _,
            is_dir: _,
            expanded: _,
            matched: _,
        } = row;
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
