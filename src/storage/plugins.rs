//! Durable plugin state: a namespaced key/value store and the advisory lock
//! that keeps one host running a given plugin's service.
//!
//! The namespace is **applied here**, from the plugin's identity — never passed
//! in by the plugin. That is what makes "a plugin cannot address another
//! plugin's keys" a property of the schema (a composite primary key) rather
//! than of a string check somewhere that could be forgotten.

use rusqlite::{params, OptionalExtension};

use super::Database;

/// Longest accepted key.
pub const MAX_KEY_LEN: usize = 256;

/// Largest accepted value, in bytes.
pub const MAX_VALUE_LEN: usize = 64 * 1024;

/// Most keys one plugin may hold.
///
/// A bound rather than a quota system: the point is that a looping plugin
/// cannot grow the database without limit, not to ration storage finely.
pub const MAX_KEYS_PER_PLUGIN: usize = 1_000;

/// How long a service lock stays valid without being renewed.
///
/// Long enough that an ordinary pause between renewals does not lose the lock,
/// short enough that a host killed with `SIGKILL` cannot wedge a plugin's
/// service for the rest of the day.
pub const LOCK_TTL_SECS: i64 = 180;

/// Why a plugin write was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StorageError {
    /// The key is empty or longer than [`MAX_KEY_LEN`].
    KeyLength(usize),
    /// The value is larger than [`MAX_VALUE_LEN`].
    ValueLength(usize),
    /// The plugin already holds [`MAX_KEYS_PER_PLUGIN`] keys.
    TooManyKeys(usize),
    /// The database refused the operation.
    Db(String),
}

impl std::fmt::Display for StorageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StorageError::KeyLength(n) => {
                write!(f, "key length {n} is outside 1..={MAX_KEY_LEN}")
            }
            StorageError::ValueLength(n) => {
                write!(
                    f,
                    "value of {n} bytes exceeds the {MAX_VALUE_LEN}-byte limit"
                )
            }
            StorageError::TooManyKeys(n) => {
                write!(
                    f,
                    "plugin already holds {n} keys (limit {MAX_KEYS_PER_PLUGIN})"
                )
            }
            StorageError::Db(msg) => write!(f, "{msg}"),
        }
    }
}

impl std::error::Error for StorageError {}

/// Who lost a contended lock to whom.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LockHeld {
    /// The host currently running the service.
    pub holder: String,
}

impl Database {
    /// Read one of a plugin's keys.
    pub fn plugin_kv_get(&self, plugin: &str, key: &str) -> rusqlite::Result<Option<String>> {
        self.conn
            .query_row(
                "SELECT value FROM plugin_kv WHERE plugin = ?1 AND key = ?2",
                params![plugin, key],
                |row| row.get::<_, String>(0),
            )
            .optional()
    }

    /// Write one of a plugin's keys.
    pub fn plugin_kv_set(&self, plugin: &str, key: &str, value: &str) -> Result<(), StorageError> {
        if key.is_empty() || key.len() > MAX_KEY_LEN {
            return Err(StorageError::KeyLength(key.len()));
        }
        if value.len() > MAX_VALUE_LEN {
            return Err(StorageError::ValueLength(value.len()));
        }

        // Counted before inserting, and only when the key is new — overwriting
        // an existing key must stay possible at the limit, or a plugin that
        // filled its allowance could never correct itself.
        let exists = self
            .plugin_kv_get(plugin, key)
            .map_err(|e| StorageError::Db(e.to_string()))?
            .is_some();
        if !exists {
            let count = self
                .plugin_kv_count(plugin)
                .map_err(|e| StorageError::Db(e.to_string()))?;
            if count >= MAX_KEYS_PER_PLUGIN {
                return Err(StorageError::TooManyKeys(count));
            }
        }

        self.conn
            .execute(
                "INSERT INTO plugin_kv (plugin, key, value, updated_at) \
                 VALUES (?1, ?2, ?3, datetime('now')) \
                 ON CONFLICT(plugin, key) DO UPDATE SET \
                 value = excluded.value, updated_at = excluded.updated_at",
                params![plugin, key, value],
            )
            .map_err(|e| StorageError::Db(e.to_string()))?;
        Ok(())
    }

    /// Delete one of a plugin's keys. Returns whether it existed.
    pub fn plugin_kv_delete(&self, plugin: &str, key: &str) -> rusqlite::Result<bool> {
        let n = self.conn.execute(
            "DELETE FROM plugin_kv WHERE plugin = ?1 AND key = ?2",
            params![plugin, key],
        )?;
        Ok(n > 0)
    }

    /// How many keys a plugin holds.
    pub fn plugin_kv_count(&self, plugin: &str) -> rusqlite::Result<usize> {
        self.conn.query_row(
            "SELECT count(*) FROM plugin_kv WHERE plugin = ?1",
            params![plugin],
            |row| row.get::<_, i64>(0).map(|n| n as usize),
        )
    }

    /// Take (or renew) a plugin's service lock.
    ///
    /// A single statement so two hosts racing cannot both win: SQLite
    /// serializes writers, and the `WHERE` clause makes taking an expired or
    /// self-held lock the only ways to succeed.
    pub fn claim_plugin_service(
        &self,
        plugin: &str,
        holder: &str,
        now_unix: i64,
    ) -> rusqlite::Result<Result<(), LockHeld>> {
        let cutoff = now_unix - LOCK_TTL_SECS;
        let claimed = self.conn.execute(
            "INSERT INTO plugin_service_locks (plugin, holder, renewed_at) \
             VALUES (?1, ?2, ?3) \
             ON CONFLICT(plugin) DO UPDATE SET holder = excluded.holder, \
             renewed_at = excluded.renewed_at \
             WHERE plugin_service_locks.holder = excluded.holder \
                OR plugin_service_locks.renewed_at < ?4",
            params![plugin, holder, now_unix, cutoff],
        )?;

        if claimed > 0 {
            return Ok(Ok(()));
        }

        let current: String = self
            .conn
            .query_row(
                "SELECT holder FROM plugin_service_locks WHERE plugin = ?1",
                params![plugin],
                |row| row.get(0),
            )
            .unwrap_or_else(|_| "unknown".to_string());
        Ok(Err(LockHeld { holder: current }))
    }

    /// Release a plugin's service lock, if this holder owns it.
    pub fn release_plugin_service(&self, plugin: &str, holder: &str) -> rusqlite::Result<bool> {
        let n = self.conn.execute(
            "DELETE FROM plugin_service_locks WHERE plugin = ?1 AND holder = ?2",
            params![plugin, holder],
        )?;
        Ok(n > 0)
    }

    /// Who currently holds a plugin's service lock, if anyone.
    pub fn plugin_service_holder(&self, plugin: &str) -> rusqlite::Result<Option<String>> {
        self.conn
            .query_row(
                "SELECT holder FROM plugin_service_locks WHERE plugin = ?1",
                params![plugin],
                |row| row.get::<_, String>(0),
            )
            .optional()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn db() -> Database {
        Database::open_in_memory().unwrap()
    }

    #[test]
    fn a_value_round_trips() {
        let db = db();
        db.plugin_kv_set("demo", "k", "v").unwrap();
        assert_eq!(db.plugin_kv_get("demo", "k").unwrap(), Some("v".into()));
    }

    #[test]
    fn an_unset_key_is_none_not_an_error() {
        let db = db();
        assert_eq!(db.plugin_kv_get("demo", "missing").unwrap(), None);
    }

    #[test]
    fn two_plugins_do_not_share_a_key_name() {
        let db = db();
        db.plugin_kv_set("a", "shared", "from-a").unwrap();
        db.plugin_kv_set("b", "shared", "from-b").unwrap();
        assert_eq!(
            db.plugin_kv_get("a", "shared").unwrap(),
            Some("from-a".into())
        );
        assert_eq!(
            db.plugin_kv_get("b", "shared").unwrap(),
            Some("from-b".into())
        );
    }

    #[test]
    fn a_key_that_looks_like_another_namespace_stays_in_its_own() {
        // The namespace is a column, not a prefix, so there is no key a plugin
        // can spell that reaches another plugin's rows.
        let db = db();
        db.plugin_kv_set("victim", "secret", "theirs").unwrap();
        db.plugin_kv_set("attacker", "victim.secret", "mine")
            .unwrap();
        db.plugin_kv_set("attacker", "../victim/secret", "mine")
            .unwrap();

        assert_eq!(
            db.plugin_kv_get("victim", "secret").unwrap(),
            Some("theirs".into())
        );
        assert_eq!(db.plugin_kv_count("victim").unwrap(), 1);
    }

    #[test]
    fn an_overlong_key_is_refused() {
        let db = db();
        let key = "k".repeat(MAX_KEY_LEN + 1);
        assert_eq!(
            db.plugin_kv_set("demo", &key, "v"),
            Err(StorageError::KeyLength(MAX_KEY_LEN + 1))
        );
    }

    #[test]
    fn an_empty_key_is_refused() {
        let db = db();
        assert_eq!(
            db.plugin_kv_set("demo", "", "v"),
            Err(StorageError::KeyLength(0))
        );
    }

    #[test]
    fn an_oversized_value_is_refused() {
        let db = db();
        let value = "v".repeat(MAX_VALUE_LEN + 1);
        let err = db.plugin_kv_set("demo", "k", &value).unwrap_err();
        assert!(matches!(err, StorageError::ValueLength(_)));
        assert!(err.to_string().contains(&MAX_VALUE_LEN.to_string()));
    }

    #[test]
    fn the_key_count_is_bounded() {
        let db = db();
        for i in 0..MAX_KEYS_PER_PLUGIN {
            db.plugin_kv_set("demo", &format!("k{i}"), "v").unwrap();
        }
        assert!(matches!(
            db.plugin_kv_set("demo", "one-too-many", "v"),
            Err(StorageError::TooManyKeys(_))
        ));
        // Overwriting an existing key must still work at the limit, or a
        // plugin that filled its allowance could never correct itself.
        db.plugin_kv_set("demo", "k0", "updated").unwrap();
        assert_eq!(
            db.plugin_kv_get("demo", "k0").unwrap(),
            Some("updated".into())
        );
    }

    #[test]
    fn deleting_frees_a_slot() {
        let db = db();
        db.plugin_kv_set("demo", "k", "v").unwrap();
        assert!(db.plugin_kv_delete("demo", "k").unwrap());
        assert!(!db.plugin_kv_delete("demo", "k").unwrap());
        assert_eq!(db.plugin_kv_count("demo").unwrap(), 0);
    }

    #[test]
    fn one_host_holds_a_service_lock() {
        let db = db();
        assert_eq!(
            db.claim_plugin_service("demo", "tui", 1_000).unwrap(),
            Ok(())
        );
        assert_eq!(
            db.claim_plugin_service("demo", "keeper", 1_001).unwrap(),
            Err(LockHeld {
                holder: "tui".into()
            })
        );
    }

    #[test]
    fn the_holder_may_renew_its_own_lock() {
        let db = db();
        db.claim_plugin_service("demo", "tui", 1_000)
            .unwrap()
            .unwrap();
        assert_eq!(
            db.claim_plugin_service("demo", "tui", 2_000).unwrap(),
            Ok(())
        );
    }

    #[test]
    fn different_plugins_do_not_contend() {
        let db = db();
        assert_eq!(db.claim_plugin_service("a", "tui", 1_000).unwrap(), Ok(()));
        assert_eq!(
            db.claim_plugin_service("b", "keeper", 1_000).unwrap(),
            Ok(())
        );
    }

    #[test]
    fn an_expired_lock_is_reclaimable() {
        let db = db();
        db.claim_plugin_service("demo", "dead-host", 1_000)
            .unwrap()
            .unwrap();
        // Past the TTL: the previous holder was killed without releasing.
        let later = 1_000 + LOCK_TTL_SECS + 1;
        assert_eq!(
            db.claim_plugin_service("demo", "keeper", later).unwrap(),
            Ok(())
        );
        assert_eq!(
            db.plugin_service_holder("demo").unwrap(),
            Some("keeper".into())
        );
    }

    #[test]
    fn a_live_holder_is_not_displaced() {
        let db = db();
        db.claim_plugin_service("demo", "tui", 1_000)
            .unwrap()
            .unwrap();
        // Just inside the TTL.
        let soon = 1_000 + LOCK_TTL_SECS - 1;
        assert!(db
            .claim_plugin_service("demo", "keeper", soon)
            .unwrap()
            .is_err());
    }

    #[test]
    fn releasing_hands_over_immediately() {
        let db = db();
        db.claim_plugin_service("demo", "tui", 1_000)
            .unwrap()
            .unwrap();
        assert!(db.release_plugin_service("demo", "tui").unwrap());
        assert_eq!(
            db.claim_plugin_service("demo", "keeper", 1_001).unwrap(),
            Ok(())
        );
    }

    #[test]
    fn only_the_holder_can_release() {
        let db = db();
        db.claim_plugin_service("demo", "tui", 1_000)
            .unwrap()
            .unwrap();
        assert!(!db.release_plugin_service("demo", "impostor").unwrap());
        assert_eq!(
            db.plugin_service_holder("demo").unwrap(),
            Some("tui".into())
        );
    }
}
