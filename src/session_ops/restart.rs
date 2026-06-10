//! Headless session restart — tears down the tmux window and re-launches
//! the agent CLI, resuming the existing conversation when the agent supports
//! it and a transcript exists, starting fresh otherwise.

use crate::session::{SessionConfig, SessionId};
use crate::storage::Database;
use crate::sync::SharedSession;

/// Restart an existing session in-place — kills its tmux window and
/// re-spawns the agent CLI.
///
/// For the `claude` agent, uses its resume group when a transcript for the
/// session id exists on disk, otherwise pins the same id for a fresh start.
/// For `resume_latest` agents (codex, opencode, gemini, aider) it resumes the
/// latest session in the (unchanged) launch directory. Other agents degrade to
/// "start fresh" (the live tmux process is what carries state across restarts).
///
/// Per-spawn customizations (`--env`/`--extra-arg`/`--system-prompt`) are read
/// back from the session's persisted spawn config so the relaunched process
/// matches the original, and multi-dir sessions land back in their symlink
/// workspace.
pub fn restart_session_headless(db: &Database, session_id: SessionId) -> Result<(), String> {
    let session = db
        .get_session_by_id(session_id)
        .map_err(|e| format!("Failed to load session: {e}"))?
        .ok_or_else(|| format!("Session not found: {session_id}"))?;

    let (config, command, args) = build_restart_invocation(db, &session)?;

    crate::agent::tmux::kill_window(&session.name)
        .map_err(|e| format!("Failed to kill tmux window: {e}"))?;
    crate::agent::tmux::spawn_window(
        &session.name,
        &command,
        &args,
        config.cwd.as_deref(),
        &config.env,
    )
    .map_err(|e| format!("Failed to re-spawn tmux window: {e}"))?;

    Ok(())
}

/// Rebuild the full launch invocation for an existing session: persisted
/// spawn config (env/extra args/system prompt), the resume decision, and the
/// process cwd (symlink workspace for multi-dir sessions). Split from
/// [`restart_session_headless`] so it is testable without tmux.
pub(crate) fn build_restart_invocation(
    db: &Database,
    session: &SharedSession,
) -> Result<(SessionConfig, String, Vec<String>), String> {
    let agent_session_id = session.agent_session_id.clone().ok_or_else(|| {
        format!(
            "Cannot restart session {} without agent_session_id",
            session.id
        )
    })?;

    let spawn_cfg = db
        .get_session_spawn_config(session.id)
        .map_err(|e| format!("Failed to load spawn config: {e}"))?
        .unwrap_or_default();

    let cwd = super::resolve_headless_process_cwd(
        &agent_session_id,
        session.cwd.clone(),
        &session.worktrees,
        &session.additional_dirs,
    );

    let mut config = SessionConfig {
        agent_session_id: Some(agent_session_id.clone()),
        cwd,
        agent: session.agent.clone(),
        extra_args: spawn_cfg.extra_args,
        system_prompt: spawn_cfg.system_prompt,
        ..SessionConfig::default()
    };
    let def = super::resolve_agent_def(&config.agent);
    super::merge_spawn_env(&mut config, &def, &spawn_cfg.env, &agent_session_id);
    config.resume_session_id = super::resume_trigger_for(&def, &agent_session_id, &config.env);

    let (command, args) = super::build_agent_invocation(&config);
    Ok((config, command, args))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::SpawnConfigRecord;

    fn session_with_spawn_config(db: &Database) -> SharedSession {
        let session = SharedSession {
            id: SessionId::default(),
            name: "w1".into(),
            agent: "claude".into(),
            backend_id: String::new(),
            backend_type: "local-tmux".into(),
            agent_session_id: Some("11111111-2222-3333-4444-555555555555".into()),
            cwd: Some(std::path::PathBuf::from("/src/repo")),
            additional_dirs: Vec::new(),
            worktrees: Vec::new(),
            shell_backend_id: None,
            parent_session_id: None,
            tombstone: false,
            tombstone_at: None,
        };
        db.upsert_session(&session).unwrap();
        db.set_session_spawn_config(
            session.id,
            &SpawnConfigRecord {
                env: vec![("WORKER".into(), "1".into())],
                extra_args: vec!["--permission-mode".into(), "plan".into()],
                system_prompt: None,
            },
        )
        .unwrap();
        session
    }

    #[test]
    fn restart_reproduces_persisted_spawn_config() {
        let temp = tempfile::TempDir::new().unwrap();
        let _guard = crate::paths::TestPathGuard::new(temp.path());
        let db = Database::open_in_memory().unwrap();
        let session = session_with_spawn_config(&db);

        let (config, command, args) = build_restart_invocation(&db, &session).unwrap();

        assert_eq!(command, "claude");
        // Per-spawn extra args come back, appended after the arg groups.
        assert_eq!(
            &args[args.len() - 2..],
            &["--permission-mode".to_string(), "plan".to_string()]
        );
        // Per-spawn env comes back, with thurbox internals layered on top.
        assert_eq!(config.env.get("WORKER").map(String::as_str), Some("1"));
        assert_eq!(
            config.env.get("THURBOX_SESSION_ID").map(String::as_str),
            session.agent_session_id.as_deref()
        );
        // Single-dir session restarts in its primary cwd.
        assert_eq!(config.cwd, session.cwd);
    }

    #[test]
    fn restart_without_spawn_config_row_is_clean() {
        let temp = tempfile::TempDir::new().unwrap();
        let _guard = crate::paths::TestPathGuard::new(temp.path());
        let db = Database::open_in_memory().unwrap();
        let session = SharedSession {
            id: SessionId::default(),
            name: "plain".into(),
            agent: "claude".into(),
            backend_id: String::new(),
            backend_type: "local-tmux".into(),
            agent_session_id: Some("66666666-7777-8888-9999-aaaaaaaaaaaa".into()),
            cwd: Some(std::path::PathBuf::from("/src/repo")),
            additional_dirs: Vec::new(),
            worktrees: Vec::new(),
            shell_backend_id: None,
            parent_session_id: None,
            tombstone: false,
            tombstone_at: None,
        };
        db.upsert_session(&session).unwrap();

        let (config, _, args) = build_restart_invocation(&db, &session).unwrap();
        assert!(config.extra_args.is_empty());
        assert!(config.system_prompt.is_none());
        // No transcript on disk -> fresh start pinning the same id.
        assert_eq!(
            args,
            vec!["--session-id", "66666666-7777-8888-9999-aaaaaaaaaaaa"]
        );
    }

    #[test]
    fn restart_without_agent_session_id_errors() {
        let db = Database::open_in_memory().unwrap();
        let session = SharedSession {
            id: SessionId::default(),
            name: "x".into(),
            agent: "claude".into(),
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
        let err = build_restart_invocation(&db, &session).unwrap_err();
        assert!(err.contains("agent_session_id"), "got {err}");
    }
}
