//! Session CRUD and orchestration subcommands.

use std::path::PathBuf;

use clap::Subcommand;
use serde_json::{json, Value};

use super::Outcome;
use crate::session::SessionId;
use crate::storage::Database;
use crate::sync::SharedSession;

// `Create` carries a dozen flag fields; a parsed Action is short-lived and
// never stored in bulk, so the size spread between variants is irrelevant.
#[allow(clippy::large_enum_variant)]
#[derive(Subcommand, Debug)]
pub enum Action {
    /// List all active sessions.
    List {
        /// Only list children of this parent session UUID.
        #[arg(long)]
        parent: Option<String>,
        /// Only sessions carrying this label, as key=value. Repeatable
        /// (all given labels must match).
        #[arg(long = "label", value_name = "KEY=VALUE")]
        labels: Vec<String>,
    },
    /// Get a session by UUID.
    Get {
        /// Session UUID.
        uuid: String,
    },
    /// Create a new session (runs synchronously — tmux window live on return).
    Create {
        /// Session name (1-64 chars, no slashes or leading '.').
        #[arg(long)]
        name: String,
        /// Absolute path to the repository or working directory.
        #[arg(long)]
        repo_path: PathBuf,
        /// Coding agent to launch (e.g. "claude", "codex"). Falls back to the
        /// default agent from `agents.toml`.
        #[arg(long)]
        agent: Option<String>,
        /// If set, create a git worktree on this branch off --base-branch.
        #[arg(long)]
        worktree_branch: Option<String>,
        /// Base branch for the worktree (default: main).
        #[arg(long)]
        base_branch: Option<String>,
        /// Remote host to run the session on (name from `hosts.toml`). The
        /// worktree and tmux window are created on that host over SSH.
        #[arg(long)]
        host: Option<String>,
        /// Parent session UUID (lead/worker relationship for orchestration).
        /// Must reference an existing active session.
        #[arg(long)]
        parent: Option<String>,
        /// Initial prompt typed into the agent's pane after a short boot
        /// delay. One-shot (not re-sent on restart). Local sessions only.
        #[arg(long)]
        prompt: Option<String>,
        /// System prompt passed via the agent's `prompt_args` template from
        /// `agents.toml` (errors if the agent defines none). Persisted and
        /// re-emitted on restart.
        #[arg(long)]
        system_prompt: Option<String>,
        /// Environment variable for the agent process, as KEY=VALUE.
        /// Repeatable. Overrides the agent's `env` defaults; persisted and
        /// reproduced on restart.
        #[arg(long = "env", value_name = "KEY=VALUE")]
        env: Vec<String>,
        /// Extra CLI argument appended to the agent invocation (after all
        /// configured args, so it can override them). Repeatable; persisted
        /// and reproduced on restart.
        #[arg(long = "extra-arg", value_name = "ARG", allow_hyphen_values = true)]
        extra_args: Vec<String>,
        /// Additional member directory: the session launches in a symlink
        /// workspace gathering the repo plus each added dir. Repeatable.
        /// Local sessions only.
        #[arg(long = "add-dir", value_name = "PATH")]
        add_dirs: Vec<PathBuf>,
        /// Free-form label attached to the session, as key=value. Repeatable;
        /// shown in `session list`/`get` and filterable via `list --label`.
        #[arg(long = "label", value_name = "KEY=VALUE")]
        labels: Vec<String>,
    },
    /// Soft-delete a session.
    ///
    /// By default only the DB row is soft-deleted (the TUI cleans up the
    /// tmux window and worktree on next sync). Pass `--force` to also
    /// kill the tmux window, remove worktrees, and cancel pending
    /// scheduled commands — useful for headless cleanup when the TUI
    /// isn't running.
    Delete {
        /// Session UUID.
        uuid: String,
        /// Also kill the tmux window, remove worktrees, and cancel
        /// pending scheduled commands for this session.
        #[arg(long)]
        force: bool,
    },
    /// Restore a soft-deleted session.
    Restore {
        /// Session UUID.
        uuid: String,
    },
    /// Restart a session in-place (kills the window, re-spawns with --resume).
    Restart {
        /// Session UUID.
        uuid: String,
    },
    /// Type text into a session's terminal, followed by Enter.
    Send {
        /// Session UUID.
        uuid: String,
        /// Text to send.
        text: String,
    },
    /// Capture rendered pane contents as text.
    Capture {
        /// Session UUID.
        uuid: String,
        /// Scrollback lines to include (default 200, max 10000).
        #[arg(long, default_value_t = 200)]
        lines: u32,
    },
    /// Block until the session's pane output matches a pattern, the window
    /// exits, or the timeout expires. Local sessions only.
    ///
    /// Exit codes: 0 = pattern matched (or, without --pattern, window
    /// exited); 3 = window died before the pattern matched; 4 = timeout.
    Wait {
        /// Session UUID.
        uuid: String,
        /// Regex matched against the captured pane text. When omitted, waits
        /// for the window to exit instead.
        #[arg(long)]
        pattern: Option<String>,
        /// Give up after this many seconds.
        #[arg(long, default_value_t = 600)]
        timeout: u64,
        /// Poll cadence in milliseconds.
        #[arg(long, default_value_t = 2000)]
        interval: u64,
        /// Scrollback lines scanned per poll (max 10000).
        #[arg(long, default_value_t = 2000)]
        lines: u32,
    },
    /// Extract the sentinel-terminated JSON result from a session's pane.
    ///
    /// Scans the captured pane text for the last occurrence of the marker
    /// and parses the JSON document that follows it. Local sessions only.
    ///
    /// Exit codes: 0 = found and valid (emitted as `result`); 3 = pending
    /// (no marker yet, or the JSON is still streaming); 4 = malformed JSON
    /// after the marker.
    Result {
        /// Session UUID.
        uuid: String,
        /// Sentinel marker preceding the JSON document.
        #[arg(long, default_value = "===RESULT===")]
        marker: String,
        /// Scrollback lines scanned (max 10000).
        #[arg(long, default_value_t = 2000)]
        lines: u32,
    },
}

/// Exit code for "keep polling" outcomes (`wait`: window died before match;
/// `result`: sentinel not there yet).
const EXIT_PENDING: i32 = 3;
/// Exit code for terminal failures (`wait`: timeout; `result`: malformed JSON).
const EXIT_FAILED: i32 = 4;
/// Floor for `session wait --interval` so a zero value can't busy-loop tmux.
const MIN_WAIT_INTERVAL_MS: u64 = 50;

pub fn run(action: Action, db: &Database) -> Result<Outcome, String> {
    match action {
        Action::Wait {
            uuid,
            pattern,
            timeout,
            interval,
            lines,
        } => run_wait(db, &uuid, pattern.as_deref(), timeout, interval, lines),
        Action::Result {
            uuid,
            marker,
            lines,
        } => run_result(db, &uuid, &marker, lines),
        other => run_value(other, db).map(Outcome::from),
    }
}

/// Extract and validate the sentinel JSON from the session's pane (see
/// [`Action::Result`] for the exit-code contract).
fn run_result(db: &Database, uuid: &str, marker: &str, lines: u32) -> Result<Outcome, String> {
    if marker.is_empty() {
        return Err("--marker must not be empty".into());
    }
    let session = resolve(db, uuid)?;
    ensure_local(&session, "result")?;
    let text = crate::agent::tmux::capture_pane_text(&session.name, lines)
        .map_err(|e| format!("capture_pane_text: {e}"))?;

    let sid = session.id.to_string();
    Ok(match extract_result(&text, marker) {
        ResultPayload::Found(result) => Outcome {
            value: json!({ "status": "ok", "session_id": sid, "result": result }),
            code: 0,
        },
        ResultPayload::Pending => Outcome {
            value: json!({ "status": "pending", "session_id": sid }),
            code: EXIT_PENDING,
        },
        ResultPayload::Malformed(error) => Outcome {
            value: json!({ "status": "malformed", "session_id": sid, "error": error }),
            code: EXIT_FAILED,
        },
    })
}

/// Outcome of scanning pane text for a sentinel-terminated JSON document.
#[derive(Debug, PartialEq)]
enum ResultPayload {
    Found(Value),
    /// Marker absent, or the JSON after it is truncated (still streaming).
    Pending,
    /// The text after the marker is not (a prefix of) valid JSON.
    Malformed(String),
}

/// Find the **last** marker (reruns overwrite earlier results) and parse the
/// first JSON document after it. A clean-EOF parse error means the agent is
/// still printing — that's `Pending`, not `Malformed`.
fn extract_result(text: &str, marker: &str) -> ResultPayload {
    let Some(pos) = text.rfind(marker) else {
        return ResultPayload::Pending;
    };
    let rest = &text[pos + marker.len()..];
    if rest.trim().is_empty() {
        return ResultPayload::Pending;
    }
    let mut stream = serde_json::Deserializer::from_str(rest).into_iter::<Value>();
    match stream.next() {
        Some(Ok(value)) => ResultPayload::Found(value),
        Some(Err(e)) if e.is_eof() => ResultPayload::Pending,
        Some(Err(e)) => ResultPayload::Malformed(e.to_string()),
        None => ResultPayload::Pending,
    }
}

/// Poll the session's pane until the wait condition resolves (see
/// [`Action::Wait`] for the exit-code contract).
fn run_wait(
    db: &Database,
    uuid: &str,
    pattern: Option<&str>,
    timeout_secs: u64,
    interval_ms: u64,
    lines: u32,
) -> Result<Outcome, String> {
    let session = resolve(db, uuid)?;
    ensure_local(&session, "wait")?;
    let regex = pattern
        .map(regex::Regex::new)
        .transpose()
        .map_err(|e| format!("Invalid --pattern regex: {e}"))?;

    let start = std::time::Instant::now();
    let deadline = start + std::time::Duration::from_secs(timeout_secs);
    loop {
        let alive = crate::agent::tmux::window_exists(&session.name);
        let text = if alive && regex.is_some() {
            Some(
                crate::agent::tmux::capture_pane_text(&session.name, lines)
                    .map_err(|e| format!("capture_pane_text: {e}"))?,
            )
        } else {
            None
        };

        let elapsed_ms = start.elapsed().as_millis() as u64;
        match evaluate_wait_poll(alive, text.as_deref(), regex.as_ref()) {
            WaitPoll::Matched(matched) => {
                return Ok(Outcome {
                    value: json!({
                        "status": "matched",
                        "match": matched,
                        "session_id": session.id.to_string(),
                        "elapsed_ms": elapsed_ms,
                    }),
                    code: 0,
                });
            }
            WaitPoll::WindowExited => {
                return Ok(Outcome {
                    value: json!({
                        "status": "exited",
                        "session_id": session.id.to_string(),
                        "elapsed_ms": elapsed_ms,
                    }),
                    code: 0,
                });
            }
            WaitPoll::WindowDied => {
                return Ok(Outcome {
                    value: json!({
                        "status": "window-died",
                        "session_id": session.id.to_string(),
                        "elapsed_ms": elapsed_ms,
                    }),
                    code: EXIT_PENDING,
                });
            }
            WaitPoll::KeepWaiting => {}
        }

        let now = std::time::Instant::now();
        if now >= deadline {
            return Ok(Outcome {
                value: json!({
                    "status": "timeout",
                    "session_id": session.id.to_string(),
                    "elapsed_ms": start.elapsed().as_millis() as u64,
                }),
                code: EXIT_FAILED,
            });
        }
        let sleep = std::time::Duration::from_millis(interval_ms.max(MIN_WAIT_INTERVAL_MS))
            .min(deadline - now);
        std::thread::sleep(sleep);
    }
}

/// One poll-cycle decision for `session wait`, pure for unit testing.
#[derive(Debug, PartialEq)]
enum WaitPoll {
    /// Pattern found in the pane text (the matched fragment).
    Matched(String),
    /// No pattern was requested and the window is gone — success.
    WindowExited,
    /// A pattern was requested but the window died before it matched.
    WindowDied,
    KeepWaiting,
}

fn evaluate_wait_poll(alive: bool, text: Option<&str>, regex: Option<&regex::Regex>) -> WaitPoll {
    match (alive, regex) {
        (false, None) => WaitPoll::WindowExited,
        (false, Some(_)) => WaitPoll::WindowDied,
        (true, None) => WaitPoll::KeepWaiting,
        (true, Some(re)) => match text.and_then(|t| re.find(t)) {
            Some(m) => WaitPoll::Matched(m.as_str().to_string()),
            None => WaitPoll::KeepWaiting,
        },
    }
}

/// Reject remote sessions for pane-polling commands: they are local-tmux
/// mechanisms in v1, and probing a possibly-down SSH host could hang.
fn ensure_local(session: &SharedSession, command: &str) -> Result<(), String> {
    if session.backend_type == crate::session::LOCAL_TMUX_BACKEND_TYPE {
        Ok(())
    } else {
        Err(format!(
            "session {command} is local-only; session '{}' runs on backend '{}'",
            session.name, session.backend_type
        ))
    }
}

fn run_value(action: Action, db: &Database) -> Result<Value, String> {
    match action {
        Action::List { parent, labels } => {
            let parent_id = parent.as_deref().map(parse_session_id).transpose()?;
            let filters = parse_kv_pairs(&labels, "--label")?;
            let sessions = db
                .list_active_sessions()
                .map_err(|e| format!("list_active_sessions: {e}"))?;
            let all_labels = db
                .labels_for_all_sessions()
                .map_err(|e| format!("labels_for_all_sessions: {e}"))?;
            let activity = crate::agent::tmux::list_window_activity();

            let none: Vec<(String, String)> = Vec::new();
            Ok(Value::Array(
                sessions
                    .iter()
                    .filter(|s| parent_id.is_none() || s.parent_session_id == parent_id)
                    .filter_map(|s| {
                        let labels = all_labels.get(&s.id.to_string()).unwrap_or(&none);
                        filters
                            .iter()
                            .all(|f| labels.contains(f))
                            .then(|| decorate_session_json(s, labels, &activity))
                    })
                    .collect(),
            ))
        }
        Action::Get { uuid } => {
            let session = resolve(db, &uuid)?;
            let labels = db
                .get_session_labels(session.id)
                .map_err(|e| format!("get_session_labels: {e}"))?;
            let activity = crate::agent::tmux::list_window_activity();
            Ok(decorate_session_json(&session, &labels, &activity))
        }
        Action::Create {
            name,
            repo_path,
            agent,
            worktree_branch,
            base_branch,
            host,
            parent,
            prompt,
            system_prompt,
            env,
            extra_args,
            add_dirs,
            labels,
        } => {
            let parent_session_id = parent.as_deref().map(parse_session_id).transpose()?;
            let env = parse_kv_pairs(&env, "--env")?;
            let labels = parse_kv_pairs(&labels, "--label")?;
            let req = crate::session_ops::SpawnRequest {
                name,
                repo_path,
                worktree_branch,
                base_branch,
                agent,
                agent_session_id: None,
                host,
                parent_session_id,
                prompt,
                system_prompt,
                env,
                extra_args,
                additional_dirs: add_dirs,
                labels,
            };
            let res = crate::session_ops::spawn_session_headless(db, req)?;
            let mut out = json!({
                "id": res.session_id.to_string(),
                "name": res.name,
                "agent": res.agent,
                "agent_session_id": res.agent_session_id,
                "cwd": res.cwd.display().to_string(),
                "parent_session_id": res.parent_session_id.map(|id| id.to_string()),
                "labels": labels_to_json(&res.labels),
            });
            if let Some(err) = res.prompt_delivery_error {
                out["prompt_delivery_error"] = Value::String(err);
            }
            Ok(out)
        }
        Action::Delete { uuid, force } => {
            let session = resolve(db, &uuid)?;
            let report = crate::session_ops::delete_session_headless(db, session.id, force)?;
            Ok(json!({
                "deleted": true,
                "id": session.id.to_string(),
                "name": session.name,
                "forced": force,
                "killed_window": report.killed_window,
                "removed_worktrees": report.removed_worktrees,
                "worktree_errors": report.worktree_errors,
                "disabled_automations": report.disabled_automations,
            }))
        }
        Action::Restore { uuid } => {
            let id: SessionId = uuid
                .parse()
                .map_err(|_| format!("Invalid session UUID: {uuid}"))?;
            let deleted = db
                .get_deleted_session_by_id(id)
                .map_err(|e| format!("get_deleted_session_by_id: {e}"))?
                .ok_or_else(|| format!("Deleted session not found: {uuid}"))?;
            db.restore_session(deleted.id)
                .map_err(|e| format!("restore_session: {e}"))?;
            Ok(json!({
                "restored": true,
                "id": deleted.id.to_string(),
                "name": deleted.name,
            }))
        }
        Action::Restart { uuid } => {
            let session = resolve(db, &uuid)?;
            crate::session_ops::restart_session_headless(db, session.id)?;
            Ok(json!({
                "restarted": true,
                "session_id": session.id.to_string(),
                "session_name": session.name,
            }))
        }
        Action::Send { uuid, text } => {
            let session = resolve(db, &uuid)?;
            if text.is_empty() {
                return Err("text must not be empty".into());
            }
            crate::agent::tmux::send_prompt_now(&session.name, &text)
                .map_err(|e| format!("send_prompt_now: {e}"))?;
            Ok(json!({
                "sent": true,
                "session_id": session.id.to_string(),
                "session_name": session.name,
            }))
        }
        // Routed to dedicated handlers in `run` (they need non-zero exit codes).
        Action::Wait { .. } | Action::Result { .. } => {
            unreachable!("Wait/Result are dispatched in run()")
        }
        Action::Capture { uuid, lines } => {
            let session = resolve(db, &uuid)?;
            let output = crate::agent::tmux::capture_pane_text(&session.name, lines)
                .map_err(|e| format!("capture_pane_text: {e}"))?;
            Ok(json!({
                "session_id": session.id.to_string(),
                "session_name": session.name,
                "lines": lines,
                "output": output,
            }))
        }
    }
}

/// Parse repeatable `KEY=VALUE` flag values, splitting on the first `=`.
///
/// Values may contain further `=` signs; newlines are rejected because the
/// remote tmux control-mode transport is line-oriented.
fn parse_kv_pairs(raw: &[String], flag: &str) -> Result<Vec<(String, String)>, String> {
    raw.iter()
        .map(|s| {
            let (key, value) = s
                .split_once('=')
                .ok_or_else(|| format!("{flag} expects KEY=VALUE, got '{s}'"))?;
            if key.is_empty() {
                return Err(format!("{flag} expects a non-empty key, got '{s}'"));
            }
            if s.contains('\n') {
                return Err(format!("{flag} values must not contain newlines"));
            }
            Ok((key.to_string(), value.to_string()))
        })
        .collect()
}

/// Render `(key, value)` label pairs as a JSON object.
fn labels_to_json(labels: &[(String, String)]) -> Value {
    Value::Object(
        labels
            .iter()
            .map(|(k, v)| (k.clone(), Value::String(v.clone())))
            .collect(),
    )
}

fn parse_session_id(uuid: &str) -> Result<SessionId, String> {
    uuid.parse()
        .map_err(|_| format!("Invalid session UUID: {uuid}"))
}

fn resolve(db: &Database, uuid: &str) -> Result<SharedSession, String> {
    let id = parse_session_id(uuid)?;
    db.get_session_by_id(id)
        .map_err(|e| format!("get_session_by_id: {e}"))?
        .ok_or_else(|| format!("Session not found: {uuid}"))
}

/// [`shared_session_to_json`] plus the orchestration fields: `labels`,
/// `alive` (window present; `null` for remote sessions — `list` never opens
/// an SSH connection), and `last_activity` (epoch seconds, or `null`).
fn decorate_session_json(
    s: &SharedSession,
    labels: &[(String, String)],
    activity: &std::collections::HashMap<String, u64>,
) -> Value {
    let mut v = shared_session_to_json(s);
    v["labels"] = labels_to_json(labels);
    let (alive, last_activity) = if s.backend_type == crate::session::LOCAL_TMUX_BACKEND_TYPE {
        let window = crate::agent::tmux::agent_window_name(&s.name);
        match activity.get(&window) {
            Some(ts) => (Value::Bool(true), json!(ts)),
            None => (Value::Bool(false), Value::Null),
        }
    } else {
        (Value::Null, Value::Null)
    };
    v["alive"] = alive;
    v["last_activity"] = last_activity;
    v
}

fn shared_session_to_json(s: &SharedSession) -> Value {
    json!({
        "id": s.id.to_string(),
        "name": s.name,
        "agent": s.agent,
        "backend_type": s.backend_type,
        "agent_session_id": s.agent_session_id,
        "cwd": s.cwd.as_ref().map(|p| p.display().to_string()),
        "parent_session_id": s.parent_session_id.map(|id| id.to_string()),
        "worktrees": s.worktrees.iter().map(|w| json!({
            "repo_path": w.repo_path.display().to_string(),
            "worktree_path": w.worktree_path.display().to_string(),
            "branch": w.branch,
        })).collect::<Vec<_>>(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn db() -> Database {
        Database::open_in_memory().unwrap()
    }

    #[test]
    fn list_empty_returns_array() {
        let db = db();
        let v = run_value(
            Action::List {
                parent: None,
                labels: vec![],
            },
            &db,
        )
        .unwrap();
        assert!(v.is_array(), "got {v}");
        assert_eq!(v.as_array().unwrap().len(), 0);
    }

    #[test]
    fn list_returns_session_with_expected_shape() {
        let db = db();
        let id = SessionId::default();
        let shared = SharedSession {
            id,
            name: "demo".into(),
            agent: "dev".into(),
            backend_id: "tb-demo".into(),
            backend_type: "local-tmux".into(),
            agent_session_id: Some("agent-1".into()),
            cwd: Some(std::path::PathBuf::from("/tmp/repo")),
            additional_dirs: Vec::new(),
            worktrees: Vec::new(),
            shell_backend_id: None,
            parent_session_id: None,
            tombstone: false,
            tombstone_at: None,
        };
        db.upsert_session(&shared).unwrap();

        let v = run_value(
            Action::List {
                parent: None,
                labels: vec![],
            },
            &db,
        )
        .unwrap();
        let arr = v.as_array().unwrap();
        assert_eq!(arr.len(), 1);
        let s = &arr[0];
        assert_eq!(s["id"].as_str(), Some(id.to_string().as_str()));
        assert_eq!(s["name"].as_str(), Some("demo"));
        assert_eq!(s["agent"].as_str(), Some("dev"));
        assert_eq!(s["backend_type"].as_str(), Some("local-tmux"));
        assert_eq!(s["agent_session_id"].as_str(), Some("agent-1"));
        assert_eq!(s["cwd"].as_str(), Some("/tmp/repo"));
        assert!(s["worktrees"].is_array());
        // Orchestration decoration: labels object; no tmux server running in
        // tests, so the local window is reported dead with no activity.
        assert!(s["labels"].is_object());
        assert_eq!(s["alive"], false);
        assert!(s["last_activity"].is_null());
    }

    #[test]
    fn list_label_filter_is_and_and_remote_alive_is_null() {
        let db = db();
        let mk = |name: &str, backend_type: &str| SharedSession {
            id: SessionId::default(),
            name: name.into(),
            agent: "dev".into(),
            backend_id: String::new(),
            backend_type: backend_type.into(),
            agent_session_id: None,
            cwd: None,
            additional_dirs: Vec::new(),
            worktrees: Vec::new(),
            shell_backend_id: None,
            parent_session_id: None,
            tombstone: false,
            tombstone_at: None,
        };
        let worker = mk("worker", "local-tmux");
        let remote = mk("remote", "ssh:devbox");
        db.upsert_session(&worker).unwrap();
        db.upsert_session(&remote).unwrap();
        db.set_session_labels(
            worker.id,
            &[("wave".into(), "1".into()), ("task".into(), "demo".into())],
        )
        .unwrap();
        db.set_session_labels(remote.id, &[("wave".into(), "1".into())])
            .unwrap();

        // Single label matches both; two labels (AND) narrow to the worker.
        let both = run_value(
            Action::List {
                parent: None,
                labels: vec!["wave=1".into()],
            },
            &db,
        )
        .unwrap();
        assert_eq!(both.as_array().unwrap().len(), 2);

        let narrowed = run_value(
            Action::List {
                parent: None,
                labels: vec!["wave=1".into(), "task=demo".into()],
            },
            &db,
        )
        .unwrap();
        let arr = narrowed.as_array().unwrap();
        assert_eq!(arr.len(), 1);
        assert_eq!(arr[0]["name"], "worker");
        assert_eq!(arr[0]["labels"]["wave"], "1");

        // Remote sessions never get a liveness probe.
        let all = run_value(
            Action::List {
                parent: None,
                labels: vec![],
            },
            &db,
        )
        .unwrap();
        let remote_json = all
            .as_array()
            .unwrap()
            .iter()
            .find(|s| s["name"] == "remote")
            .unwrap();
        assert!(remote_json["alive"].is_null());
        assert!(remote_json["last_activity"].is_null());
    }

    #[test]
    fn list_emits_parent_session_id_and_filters_by_parent() {
        let db = db();
        let parent_id = SessionId::default();
        let parent = SharedSession {
            id: parent_id,
            name: "lead".into(),
            agent: "dev".into(),
            backend_id: String::new(),
            backend_type: "local-tmux".into(),
            agent_session_id: None,
            cwd: None,
            additional_dirs: Vec::new(),
            worktrees: Vec::new(),
            shell_backend_id: None,
            parent_session_id: None,
            tombstone: false,
            tombstone_at: None,
        };
        db.upsert_session(&parent).unwrap();
        let mut child = parent.clone();
        child.id = SessionId::default();
        child.name = "worker".into();
        child.parent_session_id = Some(parent_id);
        db.upsert_session(&child).unwrap();

        let v = run_value(
            Action::List {
                parent: None,
                labels: vec![],
            },
            &db,
        )
        .unwrap();
        let arr = v.as_array().unwrap();
        assert_eq!(arr.len(), 2);
        let worker = arr.iter().find(|s| s["name"] == "worker").unwrap();
        assert_eq!(
            worker["parent_session_id"].as_str(),
            Some(parent_id.to_string().as_str())
        );

        // --parent filters to direct children only.
        let v = run_value(
            Action::List {
                parent: Some(parent_id.to_string()),
                labels: vec![],
            },
            &db,
        )
        .unwrap();
        let arr = v.as_array().unwrap();
        assert_eq!(arr.len(), 1);
        assert_eq!(arr[0]["name"], "worker");
    }

    #[test]
    fn get_unknown_uuid_errors() {
        let db = db();
        let err = run_value(
            Action::Get {
                uuid: "not-a-uuid".into(),
            },
            &db,
        )
        .unwrap_err();
        assert!(err.contains("Invalid session UUID"), "got {err}");
    }

    #[test]
    fn wait_poll_decisions() {
        let re = regex::Regex::new("===RESULT===").unwrap();
        // Window gone: success without a pattern, pending-failure with one.
        assert_eq!(
            evaluate_wait_poll(false, None, None),
            WaitPoll::WindowExited
        );
        assert_eq!(
            evaluate_wait_poll(false, None, Some(&re)),
            WaitPoll::WindowDied
        );
        // Alive without a pattern: keep waiting for exit.
        assert_eq!(evaluate_wait_poll(true, None, None), WaitPoll::KeepWaiting);
        // Alive with a pattern: match decides.
        assert_eq!(
            evaluate_wait_poll(true, Some("...===RESULT=== {}"), Some(&re)),
            WaitPoll::Matched("===RESULT===".into())
        );
        assert_eq!(
            evaluate_wait_poll(true, Some("still working"), Some(&re)),
            WaitPoll::KeepWaiting
        );
    }

    #[test]
    fn wait_rejects_remote_sessions_and_bad_regex() {
        let db = db();
        let id = SessionId::default();
        let shared = SharedSession {
            id,
            name: "remote".into(),
            agent: "dev".into(),
            backend_id: "%1".into(),
            backend_type: "ssh:devbox".into(),
            agent_session_id: None,
            cwd: None,
            additional_dirs: Vec::new(),
            worktrees: Vec::new(),
            shell_backend_id: None,
            parent_session_id: None,
            tombstone: false,
            tombstone_at: None,
        };
        db.upsert_session(&shared).unwrap();

        let err = run_wait(&db, &id.to_string(), None, 1, 100, 200).unwrap_err();
        assert!(err.contains("local-only"), "got {err}");
    }

    #[test]
    fn extract_result_finds_valid_json() {
        let text = "agent chatter\n===RESULT=== {\"status\":\"ok\",\"pr_url\":\"x\"}\n";
        let ResultPayload::Found(v) = extract_result(text, "===RESULT===") else {
            panic!("expected Found");
        };
        assert_eq!(v["status"], "ok");
    }

    #[test]
    fn extract_result_last_marker_wins() {
        let text = "===RESULT=== {\"run\":1}\nretrying…\n===RESULT=== {\"run\":2}";
        let ResultPayload::Found(v) = extract_result(text, "===RESULT===") else {
            panic!("expected Found");
        };
        assert_eq!(v["run"], 2);
    }

    #[test]
    fn extract_result_pending_when_no_marker_or_truncated() {
        // No marker yet.
        assert_eq!(
            extract_result("still working…", "===RESULT==="),
            ResultPayload::Pending
        );
        // Marker present but nothing after it yet.
        assert_eq!(
            extract_result("===RESULT===  \n", "===RESULT==="),
            ResultPayload::Pending
        );
        // JSON truncated mid-stream (clean EOF) — agent is still printing.
        assert_eq!(
            extract_result("===RESULT=== {\"status\":\"ok\",", "===RESULT==="),
            ResultPayload::Pending
        );
    }

    #[test]
    fn extract_result_malformed_on_invalid_json() {
        let got = extract_result("===RESULT=== not-json-at-all", "===RESULT===");
        assert!(
            matches!(got, ResultPayload::Malformed(_)),
            "expected Malformed, got {got:?}"
        );
    }

    #[test]
    fn parse_kv_pairs_splits_on_first_equals() {
        let pairs = parse_kv_pairs(&["A=1".into(), "B=x=y".into()], "--env").unwrap();
        assert_eq!(
            pairs,
            vec![("A".into(), "1".into()), ("B".into(), "x=y".into())]
        );
        assert!(parse_kv_pairs(&[], "--env").unwrap().is_empty());
    }

    #[test]
    fn parse_kv_pairs_rejects_bad_input() {
        for bad in ["no-equals", "=novalue-key", "A=line\nbreak"] {
            let err = parse_kv_pairs(&[bad.into()], "--label").unwrap_err();
            assert!(err.contains("--label"), "got {err}");
        }
    }

    #[test]
    fn send_rejects_empty_text() {
        let db = db();
        let id = SessionId::default();
        let shared = SharedSession {
            id,
            name: "demo".into(),
            agent: "dev".into(),
            backend_id: "tb-demo".into(),
            backend_type: "local-tmux".into(),
            agent_session_id: None,
            cwd: None,
            additional_dirs: Vec::new(),
            worktrees: Vec::new(),
            shell_backend_id: None,
            parent_session_id: None,
            tombstone: false,
            tombstone_at: None,
        };
        db.upsert_session(&shared).unwrap();
        let err = run_value(
            Action::Send {
                uuid: id.to_string(),
                text: String::new(),
            },
            &db,
        )
        .unwrap_err();
        assert!(err.contains("text"), "got {err}");
    }

    #[test]
    fn soft_delete_leaves_session_recoverable() {
        // Bug #3: `session delete` without --force only soft-deletes the
        // DB row, leaving pending scheduled commands and the metadata
        // restorable.
        let db = db();
        let id = SessionId::default();
        let shared = SharedSession {
            id,
            name: "Foo Bar".into(), // exercise bug #1 sanitization path too
            agent: "dev".into(),
            backend_id: String::new(),
            backend_type: "local-tmux".into(),
            agent_session_id: None,
            cwd: None,
            additional_dirs: Vec::new(),
            worktrees: Vec::new(),
            shell_backend_id: None,
            parent_session_id: None,
            tombstone: false,
            tombstone_at: None,
        };
        db.upsert_session(&shared).unwrap();
        let auto = db
            .create_automation(&crate::storage::automations::NewAutomation {
                name: "noop".into(),
                enabled: true,
                schedule: crate::session::AutomationSchedule::Once { at: u64::MAX },
                timezone: None,
                action: crate::session::AutomationAction::Send { session_id: id },
                prompt: "noop".into(),
                next_run_at: Some(u64::MAX),
            })
            .unwrap();

        let v = run_value(
            Action::Delete {
                uuid: id.to_string(),
                force: false,
            },
            &db,
        )
        .unwrap();
        assert_eq!(v["deleted"], true);
        assert_eq!(v["forced"], false);
        assert_eq!(v["disabled_automations"], 0);

        // Row is soft-deleted but recoverable; the automation is untouched.
        assert!(db.get_session_by_id(id).unwrap().is_none());
        assert!(db.get_automation(auto).unwrap().unwrap().enabled);
        let restored = run_value(
            Action::Restore {
                uuid: id.to_string(),
            },
            &db,
        )
        .unwrap();
        assert_eq!(restored["restored"], true);
        assert!(db.get_session_by_id(id).unwrap().is_some());
    }
}
