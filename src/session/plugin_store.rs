//! The storage seam between a plugin VM and thurbox's database.
//!
//! `plugin` may not import `storage` (architecture rule), and a plugin's VM
//! runs on its own thread where a `rusqlite::Connection` cannot be shared. Both
//! problems are solved by the same seam: a trait declared here in the pure-data
//! layer, implemented by `storage`, and handed to a VM as a factory it invokes
//! **on its own thread** so each VM gets its own connection.

use std::sync::Arc;

/// A plugin's view of durable storage.
///
/// Every method takes the plugin name because the *host* applies the
/// namespace — a plugin never names its own namespace, which is what stops one
/// plugin addressing another's keys.
pub trait PluginStore: Send {
    /// Read a key, or `None` when unset.
    fn get(&self, plugin: &str, key: &str) -> Result<Option<String>, String>;
    /// Write a key.
    fn set(&self, plugin: &str, key: &str, value: &str) -> Result<(), String>;
    /// Delete a key, reporting whether it existed.
    fn delete(&self, plugin: &str, key: &str) -> Result<bool, String>;
}

/// Builds a [`PluginStore`] on the calling thread.
///
/// A factory rather than a store, because the implementation owns a database
/// connection and connections are not shared across threads. Returning `None`
/// means storage is unavailable, and a plugin granted a storage capability
/// simply finds its binding failing rather than the host refusing to start.
pub type PluginStoreFactory = Arc<dyn Fn() -> Option<Box<dyn PluginStore>> + Send + Sync>;

/// The advisory lock a service host must hold.
///
/// Declared here for the same reason as [`PluginStore`]: `plugin` may not
/// import `storage`, and keeping the trait in the pure-data layer lets
/// `storage` implement it while the contention cases stay testable without a
/// database.
pub trait ServiceLock: Send {
    /// Take or renew the lock for `plugin`. `Ok(Err(holder))` means someone
    /// else holds it.
    fn claim(&self, plugin: &str) -> Result<Result<(), String>, String>;
    /// Release the lock for `plugin`.
    fn release(&self, plugin: &str) -> Result<(), String>;
}
