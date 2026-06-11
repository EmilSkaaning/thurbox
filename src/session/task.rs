//! Tasks — a todo list that can connect items to coding agents.
//!
//! A task is a named todo item with a status (`Todo`/`InProgress`/`Done`) and an
//! optional [`AutomationAction`]: triggering a task either pastes its title into
//! an existing session (`Send`) or spawns a new session — optionally on a fresh
//! git worktree — and prompts it (`Spawn`). A task with no action is a plain
//! local todo (triggering is a no-op until it is connected to an agent).
//!
//! The `source`/`external_id`/`external_url` fields scaffold future sync with
//! external issue trackers (Jira, GitHub Issues, …); local tasks use
//! `source = "local"` and leave the external fields empty.
//!
//! This module is pure data (no local crate imports beyond `super`), matching
//! the architecture rule for `session`. Persistence lives in `storage::tasks`;
//! dispatch lives in the `app` layer.

use super::AutomationAction;

/// `source` value for a task created locally inside thurbox.
pub const SOURCE_LOCAL: &str = "local";

/// Lifecycle state of a task.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TaskStatus {
    #[default]
    Todo,
    InProgress,
    Done,
}

impl TaskStatus {
    /// Storage discriminant (`status` column).
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Todo => "todo",
            Self::InProgress => "in_progress",
            Self::Done => "done",
        }
    }

    /// Human-readable label for display (e.g. in the editor and details panel).
    /// Unlike [`as_str`](Self::as_str), the multi-word state uses a space.
    pub fn label(self) -> &'static str {
        match self {
            Self::Todo => "todo",
            Self::InProgress => "in progress",
            Self::Done => "done",
        }
    }

    /// Parse a status stored in the database, defaulting unknown values to
    /// `Todo`.
    pub fn from_db(s: &str) -> Self {
        match s {
            "in_progress" => Self::InProgress,
            "done" => Self::Done,
            _ => Self::Todo,
        }
    }

    /// Advance to the next status, wrapping `Todo → InProgress → Done → Todo`.
    /// Drives the `Space` keybinding in the tasks panel.
    pub fn cycle(self) -> Self {
        match self {
            Self::Todo => Self::InProgress,
            Self::InProgress => Self::Done,
            Self::Done => Self::Todo,
        }
    }
}

/// A persisted task (todo item).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Task {
    pub id: i64,
    /// The task text; this is what seeds a `Send`/`Spawn` action when triggered.
    pub title: String,
    /// Optional free-form markdown description (notes, acceptance criteria, …).
    /// `None` when blank. Rendered as markdown in the read-only details panel.
    pub description: Option<String>,
    pub status: TaskStatus,
    /// How the task connects to an agent. `None` = an unconnected local todo.
    pub action: Option<AutomationAction>,
    /// Origin of the task (`"local"` or an external tracker name). Scaffolding
    /// for future external sync.
    pub source: String,
    /// Identifier in the external tracker, when `source` is not `"local"`.
    pub external_id: Option<String>,
    /// Link to the task in the external tracker, when applicable.
    pub external_url: Option<String>,
    pub created_at: u64,
    pub updated_at: u64,
    /// Soft-delete marker (unix millis). `None` = active.
    pub deleted_at: Option<u64>,
}

impl Task {
    /// Build the prompt seeded into an agent when this task is triggered
    /// (`Send` into a running session, or `Spawn` of a fresh one).
    ///
    /// Beyond the bare title it gives the agent enough context to act on its
    /// own: that it is solving a Thurbox task, the markdown description, how to
    /// fetch the full record (`thurbox-cli task show <id>`), and how to mark the
    /// task done when finished (the trigger already advances it to *in
    /// progress*). Shared by the TUI (`app`) and headless (`thurbox-cli task
    /// run`) dispatch paths so the two never drift.
    pub fn agent_prompt(&self) -> String {
        let mut prompt = format!(
            "You are working on Thurbox task #{id}.\n\n# {title}\n",
            id = self.id,
            title = self.title,
        );
        if let Some(desc) = self.description.as_deref().map(str::trim) {
            if !desc.is_empty() {
                prompt.push('\n');
                prompt.push_str(desc);
                prompt.push('\n');
            }
        }
        prompt.push_str(&format!(
            "\n---\nThis is a Thurbox task. Run `thurbox-cli task show {id}` for the full \
             record. The task is now marked **in progress**; when you finish, run \
             `thurbox-cli task edit {id} --status done`.\n",
            id = self.id,
        ));
        prompt
    }

    /// Session name used when this task is dispatched via a `Spawn` action: the
    /// task title made human-readable (case, spaces and accents preserved, e.g.
    /// `Wire up SSH backend`). Runs of whitespace collapse to one space, and the
    /// result is sanitized to satisfy [`crate::paths::validate_safe_name`] (which
    /// the headless spawn enforces) and to round-trip cleanly as a tmux window
    /// name: path separators become spaces, runs of `.` collapse to one, no
    /// leading `.`, and the whole thing is capped to
    /// [`SPAWN_SESSION_NAME_MAX`] **bytes** (never splitting a multi-byte glyph).
    /// Falls back to `task-<id>` when the title yields nothing usable.
    ///
    /// The session no longer carries the task id in its name; the durable
    /// task↔session link is the persisted `spawn_task_id` column instead (see
    /// [`matches_spawn_session`](Self::matches_spawn_session) for the legacy
    /// name-based fallback used only for pre-existing sessions).
    pub fn spawn_session_name(&self) -> String {
        let mut name = String::with_capacity(self.title.len());
        let mut pending_space = false;
        for c in self.title.chars() {
            // Path separators and whitespace both become a single space — the
            // validator rejects `/`/`\\`, and tmux sanitizes them away anyway.
            if c.is_whitespace() || c == '/' || c == '\\' {
                pending_space = !name.is_empty();
                continue;
            }
            if pending_space {
                name.push(' ');
                pending_space = false;
            }
            // Collapse runs of `.` to a single one so `..` (rejected by the
            // validator) can never appear, and drop a leading `.`.
            if c == '.' && (name.is_empty() || name.ends_with('.')) {
                continue;
            }
            name.push(c);
        }
        // Cap by byte length to match the validator's 64-byte limit, stopping on
        // a char boundary so a multi-byte glyph is never split.
        if name.len() > SPAWN_SESSION_NAME_MAX {
            let cut = (0..=SPAWN_SESSION_NAME_MAX)
                .rev()
                .find(|&i| name.is_char_boundary(i))
                .unwrap_or(0);
            name.truncate(cut);
        }
        let name = name.trim_end_matches([' ', '.']).to_string();
        if name.is_empty() {
            format!("task-{}", self.id)
        } else {
            name
        }
    }

    /// Whether `name` is this task's spawned session by **name convention**.
    ///
    /// Spawned sessions are now linked durably by `spawn_task_id`, so this is
    /// only a fallback for legacy sessions created before that column existed:
    /// it accepts the old `task-<id>-<slug>` form and the bare `task-<id>`
    /// (the `task-<id>` prefix is the only significant part, since the title
    /// may have been edited since the spawn).
    pub fn matches_spawn_session(&self, name: &str) -> bool {
        let prefix = format!("task-{}", self.id);
        name == prefix
            || name
                .strip_prefix(&prefix)
                .is_some_and(|r| r.starts_with('-'))
    }
}

/// Cap for [`Task::spawn_session_name`], matching the session-name limit used
/// elsewhere (tmux window names stay readable in the TUI session list).
pub const SPAWN_SESSION_NAME_MAX: usize = 64;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_round_trips_through_db_string() {
        for s in [TaskStatus::Todo, TaskStatus::InProgress, TaskStatus::Done] {
            assert_eq!(TaskStatus::from_db(s.as_str()), s);
        }
    }

    #[test]
    fn unknown_status_defaults_to_todo() {
        assert_eq!(TaskStatus::from_db("garbage"), TaskStatus::Todo);
        assert_eq!(TaskStatus::default(), TaskStatus::Todo);
    }

    #[test]
    fn cycle_walks_todo_in_progress_done() {
        assert_eq!(TaskStatus::Todo.cycle(), TaskStatus::InProgress);
        assert_eq!(TaskStatus::InProgress.cycle(), TaskStatus::Done);
        assert_eq!(TaskStatus::Done.cycle(), TaskStatus::Todo);
    }

    #[test]
    fn label_uses_spaced_form() {
        assert_eq!(TaskStatus::Todo.label(), "todo");
        assert_eq!(TaskStatus::InProgress.label(), "in progress");
        assert_eq!(TaskStatus::Done.label(), "done");
    }

    fn sample_task(description: Option<&str>) -> Task {
        Task {
            id: 42,
            title: "Wire up SSH backend".to_string(),
            description: description.map(str::to_string),
            status: TaskStatus::Todo,
            action: None,
            source: SOURCE_LOCAL.to_string(),
            external_id: None,
            external_url: None,
            created_at: 0,
            updated_at: 0,
            deleted_at: None,
        }
    }

    #[test]
    fn agent_prompt_carries_id_title_and_cli_hints() {
        let prompt = sample_task(Some("Implement `SshTmuxBackend`.")).agent_prompt();
        assert!(prompt.contains("Thurbox task #42"));
        assert!(prompt.contains("# Wire up SSH backend"));
        assert!(prompt.contains("Implement `SshTmuxBackend`."));
        // Self-service context: how to read more and how to close it out.
        assert!(prompt.contains("thurbox-cli task show 42"));
        assert!(prompt.contains("thurbox-cli task edit 42 --status done"));
    }

    #[test]
    fn spawn_session_name_is_the_title_verbatim() {
        assert_eq!(
            sample_task(None).spawn_session_name(),
            "Wire up SSH backend"
        );
    }

    #[test]
    fn spawn_session_name_preserves_punctuation_and_accents() {
        let mut task = sample_task(None);
        task.title = "Fix: TUI crash!! (on concurrent CLI commands)".to_string();
        assert_eq!(
            task.spawn_session_name(),
            "Fix: TUI crash!! (on concurrent CLI commands)"
        );
        // Accents survive (the accept criterion's "Blabla Blablä" case).
        task.title = "Blabla Blablä".to_string();
        assert_eq!(task.spawn_session_name(), "Blabla Blablä");
    }

    #[test]
    fn spawn_session_name_collapses_whitespace_and_caps_length() {
        let mut task = sample_task(None);
        task.title = "  Wire   up\tSSH\n backend  ".to_string();
        assert_eq!(task.spawn_session_name(), "Wire up SSH backend");
        // Length is capped by byte count, and any trailing space is trimmed.
        task.title = "x".repeat(200);
        let name = task.spawn_session_name();
        assert!(name.len() <= SPAWN_SESSION_NAME_MAX);
        assert!(name.starts_with('x'));
        // A multi-byte title is capped by bytes without splitting a glyph.
        task.title = "é".repeat(200);
        let name = task.spawn_session_name();
        assert!(name.len() <= SPAWN_SESSION_NAME_MAX);
        assert!(name.chars().all(|c| c == 'é'));
    }

    /// Mirror of `crate::paths::validate_safe_name`'s rules. Duplicated here
    /// rather than imported because `session` may not reference other crate
    /// modules (architecture rule); the headless spawn enforces the real one.
    fn is_safe_name(name: &str) -> bool {
        !name.is_empty()
            && name.len() <= 64
            && !name.starts_with('.')
            && !name.contains('/')
            && !name.contains('\\')
            && !name.contains("..")
    }

    #[test]
    fn spawn_session_name_always_passes_validate_safe_name() {
        // Titles with the characters the validator rejects (`/`, `\`, `..`,
        // leading `.`) and over-long multi-byte titles must still produce a
        // name the headless spawn accepts.
        let mut task = sample_task(None);
        for title in [
            "feat/foo: wire it up",
            "path\\to\\thing",
            "weird.. name.. here",
            "...leading dots",
            &"é".repeat(200),
            &"a/".repeat(100),
        ] {
            task.title = title.to_string();
            let name = task.spawn_session_name();
            assert!(
                is_safe_name(&name),
                "name {name:?} from title {title:?} is not a safe name"
            );
        }
    }

    #[test]
    fn spawn_session_name_sanitizes_separators_and_dot_runs() {
        let mut task = sample_task(None);
        task.title = "feat/foo".to_string();
        assert_eq!(task.spawn_session_name(), "feat foo");
        task.title = "weird.. name".to_string();
        assert_eq!(task.spawn_session_name(), "weird. name");
        task.title = "...leading".to_string();
        assert_eq!(task.spawn_session_name(), "leading");
    }

    #[test]
    fn spawn_session_name_falls_back_to_bare_id_when_blank() {
        let mut task = sample_task(None);
        task.title = "   ".to_string();
        assert_eq!(task.spawn_session_name(), "task-42");
        // A title of only separators/dots also collapses to nothing usable.
        task.title = "/// ...".to_string();
        assert_eq!(task.spawn_session_name(), "task-42");
    }

    #[test]
    fn matches_spawn_session_accepts_current_and_legacy_names() {
        let task = sample_task(None);
        assert!(task.matches_spawn_session("task-42-wire-up-ssh-backend"));
        assert!(task.matches_spawn_session("task-42-some-older-title")); // title edited
        assert!(task.matches_spawn_session("task-42")); // legacy convention
        assert!(!task.matches_spawn_session("task-421")); // different task id
        assert!(!task.matches_spawn_session("task-4"));
        assert!(!task.matches_spawn_session("my-task-42"));
    }

    #[test]
    fn agent_prompt_omits_empty_description_block() {
        // No description and a blank-only description both skip the body.
        for desc in [None, Some("   ")] {
            let prompt = sample_task(desc).agent_prompt();
            assert!(prompt.contains("# Wire up SSH backend"));
            // Title line is immediately followed by the `---` hint separator.
            assert!(prompt.contains("# Wire up SSH backend\n\n---\n"));
        }
    }
}
