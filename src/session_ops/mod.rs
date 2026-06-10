//! Headless session operations — spawn and restart sessions without the TUI.
//!
//! Callers (MCP, CLI) use these helpers to drive the same local-tmux-backed
//! sessions the TUI manages, without requiring the TUI event loop. All
//! operations are synchronous against the SQLite database and the `tmux -L
//! thurbox` server.

pub mod delete;
pub mod restart;
pub mod spawn;

pub use delete::{delete_session_headless, ForceDeleteReport};
pub use restart::restart_session_headless;
pub use spawn::{spawn_session_headless, SpawnRequest, SpawnResult};

use std::collections::HashMap;

use crate::session::SessionConfig;

/// Seconds to let a freshly spawned agent CLI boot before an initial prompt
/// is typed into its pane. Shared by `session create --prompt` and the
/// automation/task spawn paths so all headless deliveries behave the same.
pub(crate) const AGENT_BOOT_DELAY_SECS: u64 = 3;

/// Decide whether to pass the agent's resume group vs starting fresh when
/// (re)spawning. Returns `Some(id.clone())` only when a Claude transcript for
/// that id already exists on disk under `CLAUDE_CONFIG_DIR`/`~/.claude`.
///
/// Claude-specific: only the `claude` agent persists resumable transcripts.
/// For agents without an on-disk transcript this returns `None`, which makes
/// restart start a fresh conversation (the live tmux process is what provides
/// cross-restart persistence in the general case).
///
/// Shared by [`restart::restart_session_headless`] and
/// `App::restart_active_session` so the headless and TUI paths agree.
pub(crate) fn resume_id_if_transcript_exists(
    agent_session_id: &str,
    env: &HashMap<String, String>,
) -> Option<String> {
    let config_dir_override = env.get("CLAUDE_CONFIG_DIR").map(std::path::PathBuf::from);
    crate::paths::claude_transcript_exists(agent_session_id, config_dir_override.as_deref())
        .then(|| agent_session_id.to_string())
}

/// Decide the `resume_session_id` to use when restarting a session, given the
/// agent's definition.
///
/// - Agents that resume "the latest session in the launch directory"
///   ([`AgentDef::resumes_latest`]) get the session id back as a non-`None`
///   *trigger*: their `resume_args` are id-less (no `{id}` token), so the value
///   itself is ignored — its presence is what makes [`AgentDef::build_args`]
///   emit the resume group. Restart always reuses the session's directory, so
///   the agent's own "last in cwd" resolution targets the right conversation.
/// - Everyone else (claude) falls back to the transcript check, which returns
///   the pinned id only when a resumable transcript exists on disk.
///
/// Shared by the headless restart path and `App`'s restart/restore paths so
/// they agree on when to resume vs. start fresh.
pub(crate) fn resume_trigger_for(
    def: &crate::session::AgentDef,
    agent_session_id: &str,
    env: &HashMap<String, String>,
) -> Option<String> {
    if def.resumes_latest() {
        return Some(agent_session_id.to_string());
    }
    resume_id_if_transcript_exists(agent_session_id, env)
}

/// Resolve the [`AgentDef`](crate::session::AgentDef) for an agent name from
/// the on-disk registry, mirroring [`build_agent_invocation`]'s fallback chain
/// (named agent → registry default → built-in default).
pub(crate) fn resolve_agent_def(agent: &str) -> crate::session::AgentDef {
    let registry = crate::agent::agent_config::load_or_seed();
    registry
        .get(agent)
        .or_else(|| registry.default_agent())
        .cloned()
        .unwrap_or_else(|| {
            crate::agent::agent_config::builtin_registry()
                .default_agent()
                .cloned()
                .expect("built-in registry always has a default agent")
        })
}

/// Build the `(command, args)` invocation for the agent named by
/// `config.agent`, looked up in the on-disk agent registry (falling back to
/// the registry default, then to the built-in default).
///
/// Centralised here so headless spawn and restart agree on the args.
fn build_agent_invocation(config: &SessionConfig) -> (String, Vec<String>) {
    let def = resolve_agent_def(&config.agent);
    let provider = crate::agent::GenericProvider::new(def);
    // Reach the provider trait methods via fully-qualified call syntax so this
    // module imports nothing from the agent module (architecture rule:
    // session_ops must stay free of agent-layer imports).
    let command =
        <crate::agent::GenericProvider as crate::agent::AgentProvider>::command(&provider)
            .to_string();
    let args = <crate::agent::GenericProvider as crate::agent::AgentProvider>::build_args(
        &provider, config,
    );
    (command, args)
}

/// Inject the standard thurbox env hints (`THURBOX_SESSION_ID`,
/// `THURBOX_METRICS_DIR`) into a session config.
///
/// Mirrors `App::do_spawn_session` so headless and TUI sessions look
/// identical to the spawned process.
fn inject_thurbox_env(config: &mut SessionConfig, agent_session_id: &str) {
    config
        .env
        .insert("THURBOX_SESSION_ID".into(), agent_session_id.into());
    if let Some(dir) = crate::paths::metrics_directory() {
        config
            .env
            .insert("THURBOX_METRICS_DIR".into(), dir.to_string_lossy().into());
    }
}

/// Merge the three environment layers into `config.env`, lowest precedence
/// first: agent-level defaults from `agents.toml`, then per-spawn `--env`
/// overrides, then the thurbox-internal variables (which stay authoritative).
///
/// Shared by headless spawn and restart so both produce the same process env.
pub(crate) fn merge_spawn_env(
    config: &mut SessionConfig,
    def: &crate::session::AgentDef,
    spawn_env: &[(String, String)],
    agent_session_id: &str,
) {
    for (k, v) in &def.env {
        // `or_insert`: values already on the config (e.g. set by a TUI flow)
        // outrank agent-level defaults, same as `App::build_spawn_inputs`.
        config.env.entry(k.clone()).or_insert_with(|| v.clone());
    }
    for (k, v) in spawn_env {
        config.env.insert(k.clone(), v.clone());
    }
    inject_thurbox_env(config, agent_session_id);
}

/// The directory a headless session's agent process launches in: the primary
/// cwd directly, or the multi-repo symlink workspace when the session spans
/// several member dirs (worktree checkouts first, then non-duplicate
/// additional dirs — mirroring the TUI's `session_member_dirs` ordering).
///
/// Shared by headless spawn and restart so both land in the same workspace.
pub(crate) fn resolve_headless_process_cwd(
    agent_session_id: &str,
    primary_cwd: Option<std::path::PathBuf>,
    worktrees: &[crate::sync::SharedWorktree],
    additional_dirs: &[std::path::PathBuf],
) -> Option<std::path::PathBuf> {
    let mut members: Vec<(Option<String>, std::path::PathBuf)> = Vec::new();
    if worktrees.is_empty() {
        if let Some(cwd) = &primary_cwd {
            members.push((crate::git::repo_display_name(cwd), cwd.clone()));
        }
    } else {
        for wt in worktrees {
            members.push((
                crate::git::repo_display_name(&wt.repo_path),
                wt.worktree_path.clone(),
            ));
        }
    }
    for dir in additional_dirs {
        if !members.iter().any(|(_, d)| d == dir) {
            members.push((crate::git::repo_display_name(dir), dir.clone()));
        }
    }
    crate::workspace::resolve_process_cwd(Some(agent_session_id), primary_cwd, &members)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merge_spawn_env_layers_def_then_spawn_then_thurbox() {
        let def = crate::session::AgentDef {
            name: "x".into(),
            command: "x".into(),
            env: [
                ("FROM_DEF".to_string(), "def".to_string()),
                ("SHARED".to_string(), "def".to_string()),
                ("THURBOX_SESSION_ID".to_string(), "spoofed".to_string()),
            ]
            .into_iter()
            .collect(),
            ..crate::session::AgentDef::default()
        };
        let spawn_env = vec![("SHARED".to_string(), "spawn".to_string())];

        let mut config = SessionConfig::default();
        config.env.insert("PRESET".into(), "config".into());
        merge_spawn_env(&mut config, &def, &spawn_env, "sid-1");

        // Agent defaults fill gaps but never override pre-existing values…
        assert_eq!(config.env.get("FROM_DEF").map(String::as_str), Some("def"));
        assert_eq!(config.env.get("PRESET").map(String::as_str), Some("config"));
        // …per-spawn values beat agent defaults…
        assert_eq!(config.env.get("SHARED").map(String::as_str), Some("spawn"));
        // …and thurbox internals beat everything.
        assert_eq!(
            config.env.get("THURBOX_SESSION_ID").map(String::as_str),
            Some("sid-1")
        );
    }

    #[test]
    fn headless_process_cwd_dedups_members_and_keeps_primary_for_single() {
        use std::path::PathBuf;
        let wt = crate::sync::SharedWorktree {
            repo_path: PathBuf::from("/src/repo"),
            worktree_path: PathBuf::from("/src/repo/worktrees/feat"),
            branch: "feat".into(),
        };
        // The additional dir duplicates the worktree checkout: after dedup
        // only one member remains, so the primary cwd is used directly (no
        // workspace is built, no filesystem touched).
        let out = resolve_headless_process_cwd(
            "sid-1",
            Some(wt.worktree_path.clone()),
            std::slice::from_ref(&wt),
            std::slice::from_ref(&wt.worktree_path),
        );
        assert_eq!(out, Some(wt.worktree_path));
    }

    #[test]
    fn resume_id_is_none_when_no_transcript() {
        let tmp = tempfile::tempdir().unwrap();
        let mut env = HashMap::new();
        env.insert("CLAUDE_CONFIG_DIR".into(), tmp.path().display().to_string());
        assert_eq!(resume_id_if_transcript_exists("some-uuid", &env), None);
    }

    #[test]
    fn resume_id_is_some_when_transcript_exists() {
        let tmp = tempfile::tempdir().unwrap();
        let sid = "77777777-8888-9999-aaaa-bbbbbbbbbbbb";
        let proj = tmp.path().join("projects").join("-slug");
        std::fs::create_dir_all(&proj).unwrap();
        std::fs::write(proj.join(format!("{sid}.jsonl")), b"").unwrap();

        let mut env = HashMap::new();
        env.insert("CLAUDE_CONFIG_DIR".into(), tmp.path().display().to_string());
        assert_eq!(
            resume_id_if_transcript_exists(sid, &env),
            Some(sid.to_string())
        );
    }

    #[test]
    fn resume_trigger_latest_agent_always_triggers() {
        // A resume_latest agent (codex) triggers resume regardless of any
        // on-disk claude transcript; the returned id is just the trigger.
        let codex = crate::agent::agent_config::builtin_registry()
            .get("codex")
            .unwrap()
            .clone();
        assert!(codex.resumes_latest());
        let env = HashMap::new();
        assert_eq!(
            resume_trigger_for(&codex, "thurbox-uuid", &env),
            Some("thurbox-uuid".to_string())
        );
    }

    #[test]
    fn resume_trigger_claude_defers_to_transcript() {
        // claude is not resume_latest, so it only resumes when a transcript
        // exists — same behaviour as resume_id_if_transcript_exists.
        let claude = crate::agent::agent_config::builtin_registry()
            .get("claude")
            .unwrap()
            .clone();
        assert!(!claude.resumes_latest());

        let tmp = tempfile::tempdir().unwrap();
        let mut env = HashMap::new();
        env.insert("CLAUDE_CONFIG_DIR".into(), tmp.path().display().to_string());
        // No transcript yet -> no resume.
        assert_eq!(resume_trigger_for(&claude, "missing", &env), None);

        // With a transcript -> resume by the pinned id.
        let sid = "11111111-2222-3333-4444-555555555555";
        let proj = tmp.path().join("projects").join("-slug");
        std::fs::create_dir_all(&proj).unwrap();
        std::fs::write(proj.join(format!("{sid}.jsonl")), b"").unwrap();
        assert_eq!(
            resume_trigger_for(&claude, sid, &env),
            Some(sid.to_string())
        );
    }
}
