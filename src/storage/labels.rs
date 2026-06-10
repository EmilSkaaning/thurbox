//! Free-form `key=value` labels attached to sessions.
//!
//! Labels let orchestration scripts tag worker sessions (wave, task id,
//! parent reference, …) without overloading the session name. They live in
//! their own table keyed by `(session_id, key)`, so soft-deleting/restoring
//! the session row leaves them untouched.

use std::collections::HashMap;

use rusqlite::params;

use crate::session::SessionId;
use crate::sync::current_time_millis;

use super::audit::{AuditAction, EntityType};
use super::Database;

impl Database {
    /// Replace all labels for a session.
    pub fn set_session_labels(
        &self,
        session_id: SessionId,
        labels: &[(String, String)],
    ) -> rusqlite::Result<()> {
        let now = current_time_millis() as i64;
        let sid = session_id.to_string();

        self.conn.execute(
            "DELETE FROM session_labels WHERE session_id = ?1",
            params![sid],
        )?;
        for (key, value) in labels {
            self.conn.execute(
                "INSERT INTO session_labels (session_id, key, value, created_at) \
                 VALUES (?1, ?2, ?3, ?4)",
                params![sid, key, value, now],
            )?;
        }

        if !labels.is_empty() {
            self.log_audit(
                EntityType::Session,
                &sid,
                AuditAction::Updated,
                Some("labels"),
                None,
                Some(&labels.len().to_string()),
            )?;
        }

        Ok(())
    }

    /// All labels for one session, sorted by key.
    pub fn get_session_labels(
        &self,
        session_id: SessionId,
    ) -> rusqlite::Result<Vec<(String, String)>> {
        let mut stmt = self
            .conn
            .prepare("SELECT key, value FROM session_labels WHERE session_id = ?1 ORDER BY key")?;
        let rows = stmt.query_map(params![session_id.to_string()], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;
        rows.collect()
    }

    /// Labels for every session in one query: session id (text) → sorted
    /// `(key, value)` pairs. Used to decorate/filter `session list` without
    /// a per-session round-trip.
    pub fn labels_for_all_sessions(
        &self,
    ) -> rusqlite::Result<HashMap<String, Vec<(String, String)>>> {
        let mut stmt = self
            .conn
            .prepare("SELECT session_id, key, value FROM session_labels ORDER BY key")?;
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })?;

        let mut out: HashMap<String, Vec<(String, String)>> = HashMap::new();
        for row in rows {
            let (sid, key, value) = row?;
            out.entry(sid).or_default().push((key, value));
        }
        Ok(out)
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

    fn labels(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    #[test]
    fn set_and_get_roundtrip_sorted_by_key() {
        let (db, sid) = setup_db_with_session();
        db.set_session_labels(sid, &labels(&[("wave", "1"), ("task", "demo")]))
            .unwrap();
        assert_eq!(
            db.get_session_labels(sid).unwrap(),
            labels(&[("task", "demo"), ("wave", "1")])
        );
    }

    #[test]
    fn set_replaces_existing_labels() {
        let (db, sid) = setup_db_with_session();
        db.set_session_labels(sid, &labels(&[("wave", "1")]))
            .unwrap();
        db.set_session_labels(sid, &labels(&[("wave", "2")]))
            .unwrap();
        assert_eq!(
            db.get_session_labels(sid).unwrap(),
            labels(&[("wave", "2")])
        );
    }

    #[test]
    fn labels_for_all_sessions_batches() {
        let (db, sid) = setup_db_with_session();
        db.set_session_labels(sid, &labels(&[("wave", "1")]))
            .unwrap();
        let all = db.labels_for_all_sessions().unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[&sid.to_string()], labels(&[("wave", "1")]));
    }

    #[test]
    fn labels_survive_soft_delete_and_restore() {
        let (db, sid) = setup_db_with_session();
        db.set_session_labels(sid, &labels(&[("wave", "1")]))
            .unwrap();
        db.soft_delete_session(sid).unwrap();
        db.restore_session(sid).unwrap();
        assert_eq!(
            db.get_session_labels(sid).unwrap(),
            labels(&[("wave", "1")])
        );
    }

    #[test]
    fn no_labels_returns_empty() {
        let (db, sid) = setup_db_with_session();
        assert!(db.get_session_labels(sid).unwrap().is_empty());
        assert!(db.labels_for_all_sessions().unwrap().is_empty());
    }
}
