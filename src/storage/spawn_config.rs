//! Per-session spawn configuration persisted for restart fidelity.
//!
//! `session create --env/--extra-arg/--system-prompt` customizations must
//! survive a restart, which rebuilds the agent invocation from scratch. They
//! live in a side table (not `sessions` columns) so the TUI's whole-row
//! session upsert can never wipe them. Written once at headless spawn; read
//! by both the headless and TUI restart paths. Sessions without a row simply
//! restart with no per-spawn customization (the TUI-created default).

use rusqlite::{params, OptionalExtension};

use crate::session::SessionId;
use crate::sync::current_time_millis;

use super::Database;

/// The per-spawn customizations of one session.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SpawnConfigRecord {
    /// Per-spawn environment overrides (`--env`), in insertion order.
    pub env: Vec<(String, String)>,
    /// Per-spawn arguments appended to the agent invocation (`--extra-arg`).
    pub extra_args: Vec<String>,
    /// System prompt substituted into the agent's `prompt_args` template.
    pub system_prompt: Option<String>,
}

impl SpawnConfigRecord {
    /// True when there is nothing to persist.
    pub fn is_empty(&self) -> bool {
        self.env.is_empty() && self.extra_args.is_empty() && self.system_prompt.is_none()
    }
}

impl Database {
    /// Insert or replace the spawn config for a session.
    pub fn set_session_spawn_config(
        &self,
        session_id: SessionId,
        cfg: &SpawnConfigRecord,
    ) -> rusqlite::Result<()> {
        let now = current_time_millis() as i64;
        let env = serde_json::to_string(&cfg.env).unwrap_or_else(|_| "[]".into());
        let extra_args = serde_json::to_string(&cfg.extra_args).unwrap_or_else(|_| "[]".into());

        self.conn.execute(
            "INSERT INTO session_spawn_config \
                 (session_id, env, extra_args, system_prompt, created_at, updated_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?5) \
             ON CONFLICT(session_id) DO UPDATE SET \
                 env = ?2, extra_args = ?3, system_prompt = ?4, updated_at = ?5",
            params![
                session_id.to_string(),
                env,
                extra_args,
                cfg.system_prompt,
                now
            ],
        )?;
        Ok(())
    }

    /// The spawn config for a session, or `None` when it never had one.
    pub fn get_session_spawn_config(
        &self,
        session_id: SessionId,
    ) -> rusqlite::Result<Option<SpawnConfigRecord>> {
        self.conn
            .query_row(
                "SELECT env, extra_args, system_prompt FROM session_spawn_config \
                 WHERE session_id = ?1",
                params![session_id.to_string()],
                |row| {
                    let env: String = row.get(0)?;
                    let extra_args: String = row.get(1)?;
                    let system_prompt: Option<String> = row.get(2)?;
                    Ok(SpawnConfigRecord {
                        env: serde_json::from_str(&env).unwrap_or_default(),
                        extra_args: serde_json::from_str(&extra_args).unwrap_or_default(),
                        system_prompt,
                    })
                },
            )
            .optional()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sync::SharedSession;

    fn setup_db_with_session() -> (Database, SessionId) {
        let db = Database::open_in_memory().unwrap();
        let session = SharedSession {
            id: SessionId::default(),
            name: "S1".to_string(),
            agent: "dev".to_string(),
            backend_id: String::new(),
            backend_type: "local-tmux".to_string(),
            agent_session_id: None,
            cwd: None,
            additional_dirs: Vec::new(),
            worktrees: Vec::new(),
            shell_backend_id: None,
            parent_session_id: None,
            tombstone: false,
            tombstone_at: None,
        };
        let sid = session.id;
        db.upsert_session(&session).unwrap();
        (db, sid)
    }

    #[test]
    fn roundtrip_preserves_env_order_args_and_prompt() {
        let (db, sid) = setup_db_with_session();
        let cfg = SpawnConfigRecord {
            env: vec![
                ("ZED".into(), "last".into()),
                ("ALPHA".into(), "first".into()),
            ],
            extra_args: vec!["--permission-mode".into(), "plan".into()],
            system_prompt: Some("be terse".into()),
        };
        db.set_session_spawn_config(sid, &cfg).unwrap();
        assert_eq!(db.get_session_spawn_config(sid).unwrap(), Some(cfg));
    }

    #[test]
    fn missing_row_returns_none() {
        let (db, sid) = setup_db_with_session();
        assert_eq!(db.get_session_spawn_config(sid).unwrap(), None);
    }

    #[test]
    fn set_replaces_existing_config() {
        let (db, sid) = setup_db_with_session();
        db.set_session_spawn_config(
            sid,
            &SpawnConfigRecord {
                system_prompt: Some("v1".into()),
                ..SpawnConfigRecord::default()
            },
        )
        .unwrap();
        db.set_session_spawn_config(
            sid,
            &SpawnConfigRecord {
                system_prompt: Some("v2".into()),
                ..SpawnConfigRecord::default()
            },
        )
        .unwrap();
        assert_eq!(
            db.get_session_spawn_config(sid)
                .unwrap()
                .unwrap()
                .system_prompt
                .as_deref(),
            Some("v2")
        );
    }

    #[test]
    fn is_empty_detects_default() {
        assert!(SpawnConfigRecord::default().is_empty());
        assert!(!SpawnConfigRecord {
            extra_args: vec!["--x".into()],
            ..SpawnConfigRecord::default()
        }
        .is_empty());
    }
}
