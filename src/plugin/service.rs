//! The headless half of the plugin system.
//!
//! A plugin's service runs whether or not a TUI exists — hosted by the TUI, by
//! the tmux heartbeat keeper's `automation tick`, or by a one-off CLI
//! invocation, whichever needs it first. That is what lets a plugin inherit
//! v1's guarantee that scheduled work fires with the TUI closed, instead of
//! silently revoking it.
//!
//! Two properties matter and are enforced here:
//!
//! - **One host per plugin.** A machine-wide advisory lock, taken per plugin,
//!   means a running TUI and a background tick never both drive one poll loop.
//! - **The halves are independent.** A service runs in its own VM with its own
//!   capability grant, so a broken sync loop degrades a plugin's background
//!   work without removing its pane, and vice versa.

use std::collections::HashMap;

use crate::plugin::capabilities::GrantedCapabilities;
use crate::plugin::discovery::DiscoveredPlugin;
use crate::plugin::runtime::{
    ExecutionBounds, PluginThread, RuntimeError, SERVICE_ENTRY_FILE_NAME,
};
use crate::session::plugin_manifest::PluginHalf;
use crate::session::plugin_store::{PluginStoreFactory, ServiceLock};

/// Why a service did not start.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ServiceError {
    /// Another host is already running this plugin's service.
    AlreadyHosted {
        /// Who holds it.
        holder: String,
    },
    /// The service's own code failed.
    Runtime(RuntimeError),
    /// The lock could not be consulted.
    Lock(String),
}

impl std::fmt::Display for ServiceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ServiceError::AlreadyHosted { holder } => {
                write!(f, "service is already hosted by {holder}")
            }
            ServiceError::Runtime(e) => write!(f, "{e}"),
            ServiceError::Lock(msg) => write!(f, "cannot take the service lock: {msg}"),
        }
    }
}

impl std::error::Error for ServiceError {}

/// One plugin's running service.
struct RunningService {
    thread: PluginThread,
}

/// Hosts the service half of every plugin that has one.
pub struct ServiceHost {
    running: HashMap<String, RunningService>,
    bounds: ExecutionBounds,
}

impl std::fmt::Debug for ServiceHost {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ServiceHost")
            .field("running", &self.running.len())
            .finish_non_exhaustive()
    }
}

impl ServiceHost {
    /// A host running nothing yet.
    pub fn new(bounds: ExecutionBounds) -> Self {
        Self {
            running: HashMap::new(),
            bounds,
        }
    }

    /// Which services this host is running.
    pub fn running(&self) -> Vec<&str> {
        let mut names: Vec<&str> = self.running.keys().map(String::as_str).collect();
        names.sort_unstable();
        names
    }

    /// Whether this host runs a given plugin's service.
    pub fn is_running(&self, plugin: &str) -> bool {
        self.running.contains_key(plugin)
    }

    /// Start one plugin's service, taking its lock first.
    ///
    /// A plugin with no service half is not an error — it simply has nothing
    /// to host, which is the common case.
    pub fn start(
        &mut self,
        plugin: &DiscoveredPlugin,
        lock: &dyn ServiceLock,
        store: Option<PluginStoreFactory>,
    ) -> Result<bool, ServiceError> {
        if !plugin.manifest.has_service() {
            return Ok(false);
        }
        let name = plugin.name().to_string();
        if self.running.contains_key(&name) {
            return Ok(true);
        }

        // The lock before the VM: starting and then discovering a contender
        // would mean a poll loop already ran twice.
        match lock.claim(&name).map_err(ServiceError::Lock)? {
            Ok(()) => {}
            Err(holder) => return Err(ServiceError::AlreadyHosted { holder }),
        }

        let granted = GrantedCapabilities::from_manifest(
            &plugin.manifest.capabilities_for(PluginHalf::Service),
        );

        let started = PluginThread::spawn_half(
            &name,
            &plugin.dir,
            SERVICE_ENTRY_FILE_NAME,
            granted,
            self.bounds,
            store,
        )
        .and_then(|thread| {
            thread.load_entry()?;
            thread.call_init()?;
            Ok(thread)
        });

        match started {
            Ok(thread) => {
                self.running.insert(name, RunningService { thread });
                Ok(true)
            }
            Err(e) => {
                // Hold nothing we are not running: another host should be free
                // to try, and it may well succeed where this one failed.
                let _ = lock.release(&name);
                Err(ServiceError::Runtime(e))
            }
        }
    }

    /// Stop one plugin's service and release its lock.
    pub fn stop(&mut self, plugin: &str, lock: &dyn ServiceLock) {
        if let Some(service) = self.running.remove(plugin) {
            let _ = service.thread.stop();
        }
        let _ = lock.release(plugin);
    }

    /// Stop everything this host runs.
    pub fn stop_all(&mut self, lock: &dyn ServiceLock) {
        for name in self.running.keys().cloned().collect::<Vec<_>>() {
            self.stop(&name, lock);
        }
    }

    /// Run one unit of a service's work.
    ///
    /// A short-lived host (the headless tick) starts the service, ticks it
    /// once, and stops — so a plugin's background work is expressed as "do a
    /// unit" rather than "loop forever", which is the only shape that works in
    /// both a long-lived TUI and a 60-second keeper invocation.
    pub fn tick(&self, plugin: &str) -> Result<bool, RuntimeError> {
        self.running
            .get(plugin)
            .ok_or(RuntimeError::ThreadGone)?
            .thread
            .call_named("tick")
    }

    /// Run a plugin-owned CLI verb against its service.
    pub fn run_verb(
        &self,
        plugin: &str,
        verb: &str,
        args: &[String],
    ) -> Result<String, RuntimeError> {
        self.running
            .get(plugin)
            .ok_or(RuntimeError::ThreadGone)?
            .thread
            .run_verb(verb, args)
    }

    /// Evaluate a chunk inside a running service's VM (host-side diagnostic).
    pub fn eval(&self, plugin: &str, source: &str) -> Result<String, RuntimeError> {
        self.running
            .get(plugin)
            .ok_or(RuntimeError::ThreadGone)?
            .thread
            .eval(source)
    }
}

impl Drop for ServiceHost {
    fn drop(&mut self) {
        // Threads are released even without a lock to release against; the
        // lock's own expiry covers a host that dies without cleaning up.
        for (_, service) in self.running.drain() {
            let _ = service.thread.stop();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin::discovery::discover_in;
    use std::fs;
    use std::path::Path;
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    /// An in-memory lock, so contention is testable without a database.
    #[derive(Default, Clone)]
    struct FakeLock {
        held: Arc<Mutex<HashMap<String, String>>>,
        holder: String,
    }

    impl FakeLock {
        fn named(holder: &str) -> Self {
            Self {
                held: Arc::new(Mutex::new(HashMap::new())),
                holder: holder.to_string(),
            }
        }

        /// A second host sharing the same lock table.
        fn peer(&self, holder: &str) -> Self {
            Self {
                held: Arc::clone(&self.held),
                holder: holder.to_string(),
            }
        }
    }

    impl ServiceLock for FakeLock {
        fn claim(&self, plugin: &str) -> Result<Result<(), String>, String> {
            let mut held = self.held.lock().unwrap();
            match held.get(plugin) {
                Some(other) if other != &self.holder => Ok(Err(other.clone())),
                _ => {
                    held.insert(plugin.to_string(), self.holder.clone());
                    Ok(Ok(()))
                }
            }
        }

        fn release(&self, plugin: &str) -> Result<(), String> {
            let mut held = self.held.lock().unwrap();
            if held.get(plugin) == Some(&self.holder) {
                held.remove(plugin);
            }
            Ok(())
        }
    }

    fn bounds() -> ExecutionBounds {
        ExecutionBounds {
            interrupt_budget: 5_000,
            memory_limit: 8 * 1024 * 1024,
            shutdown_timeout: Duration::from_millis(300),
        }
    }

    /// Write a plugin with the given halves. `None` omits that entry file.
    fn write_plugin(
        root: &Path,
        name: &str,
        manifest_extra: &str,
        view: Option<&str>,
        service: Option<&str>,
    ) {
        let dir = root.join(name);
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("plugin.toml"),
            format!("name = \"{name}\"\napi_version = 1\n{manifest_extra}"),
        )
        .unwrap();
        if let Some(src) = view {
            fs::write(dir.join("init.luau"), src).unwrap();
        }
        if let Some(src) = service {
            fs::write(dir.join("service.luau"), src).unwrap();
        }
    }

    fn discovered(root: &Path, name: &str) -> DiscoveredPlugin {
        discover_in(&[], Some(root))
            .loadable
            .into_iter()
            .find(|p| p.name() == name)
            .expect("plugin discovered")
    }

    #[test]
    fn a_plugin_without_a_service_has_nothing_to_host() {
        let tmp = tempfile::tempdir().unwrap();
        write_plugin(tmp.path(), "viewonly", "", Some("return {}"), None);

        let mut host = ServiceHost::new(bounds());
        let lock = FakeLock::named("tui");
        assert_eq!(
            host.start(&discovered(tmp.path(), "viewonly"), &lock, None),
            Ok(false)
        );
        assert!(host.running().is_empty());
    }

    #[test]
    fn a_service_only_plugin_runs_with_no_view() {
        let tmp = tempfile::tempdir().unwrap();
        write_plugin(
            tmp.path(),
            "syncer",
            "[service]\ncapabilities = []\n",
            None,
            Some("STARTED = true return {}"),
        );

        let mut host = ServiceHost::new(bounds());
        let lock = FakeLock::named("keeper");
        assert_eq!(
            host.start(&discovered(tmp.path(), "syncer"), &lock, None),
            Ok(true)
        );
        assert_eq!(host.running(), vec!["syncer"]);
        assert_eq!(host.eval("syncer", "return STARTED").unwrap(), "true");
    }

    #[test]
    fn the_service_init_runs() {
        let tmp = tempfile::tempdir().unwrap();
        write_plugin(
            tmp.path(),
            "initd",
            "[service]\n",
            None,
            "return { init = function() RAN = true end }".into(),
        );

        let mut host = ServiceHost::new(bounds());
        let lock = FakeLock::named("tui");
        host.start(&discovered(tmp.path(), "initd"), &lock, None)
            .unwrap();
        assert_eq!(host.eval("initd", "return RAN").unwrap(), "true");
    }

    #[test]
    fn a_second_host_is_refused_the_same_service() {
        let tmp = tempfile::tempdir().unwrap();
        write_plugin(tmp.path(), "solo", "[service]\n", None, Some("return {}"));
        let plugin = discovered(tmp.path(), "solo");

        let tui_lock = FakeLock::named("tui");
        let keeper_lock = tui_lock.peer("keeper");

        let mut tui = ServiceHost::new(bounds());
        let mut keeper = ServiceHost::new(bounds());

        assert_eq!(tui.start(&plugin, &tui_lock, None), Ok(true));
        assert_eq!(
            keeper.start(&plugin, &keeper_lock, None),
            Err(ServiceError::AlreadyHosted {
                holder: "tui".to_string()
            })
        );
        assert!(keeper.running().is_empty());
    }

    #[test]
    fn stopping_hands_the_service_over() {
        let tmp = tempfile::tempdir().unwrap();
        write_plugin(tmp.path(), "solo", "[service]\n", None, Some("return {}"));
        let plugin = discovered(tmp.path(), "solo");

        let tui_lock = FakeLock::named("tui");
        let keeper_lock = tui_lock.peer("keeper");
        let mut tui = ServiceHost::new(bounds());
        let mut keeper = ServiceHost::new(bounds());

        tui.start(&plugin, &tui_lock, None).unwrap();
        tui.stop("solo", &tui_lock);
        assert_eq!(keeper.start(&plugin, &keeper_lock, None), Ok(true));
    }

    #[test]
    fn a_failed_service_releases_its_lock() {
        // Otherwise one host's broken start would keep every other host from
        // trying — and another host may well succeed.
        let tmp = tempfile::tempdir().unwrap();
        write_plugin(tmp.path(), "broken", "[service]\n", None, Some("return {"));
        let plugin = discovered(tmp.path(), "broken");

        let tui_lock = FakeLock::named("tui");
        let mut tui = ServiceHost::new(bounds());
        assert!(tui.start(&plugin, &tui_lock, None).is_err());
        assert!(!tui.is_running("broken"));

        // The lock is free again.
        assert_eq!(tui_lock.peer("keeper").claim("broken").unwrap(), Ok(()));
    }

    #[test]
    fn the_halves_do_not_share_state() {
        let tmp = tempfile::tempdir().unwrap();
        write_plugin(
            tmp.path(),
            "both",
            "capabilities = [\"render\"]\n[service]\n[[panes]]\nid = \"p\"\n",
            Some("return { render = function() return { kind = \"divider\" } end }"),
            Some("SERVICE_ONLY = true return {}"),
        );
        let plugin = discovered(tmp.path(), "both");

        let lock = FakeLock::named("tui");
        let mut services = ServiceHost::new(bounds());
        services.start(&plugin, &lock, None).unwrap();

        let mut views =
            crate::plugin::PluginHost::from_discovery(discover_in(&[], Some(tmp.path())), bounds());
        views.start_all();

        assert_eq!(
            services.eval("both", "return SERVICE_ONLY").unwrap(),
            "true"
        );
        assert_eq!(views.eval("both", "return SERVICE_ONLY").unwrap(), "nil");
    }

    #[test]
    fn a_broken_view_leaves_the_service_running() {
        let tmp = tempfile::tempdir().unwrap();
        write_plugin(
            tmp.path(),
            "half",
            "capabilities = [\"render\"]\n[service]\n[[panes]]\nid = \"p\"\n",
            Some("return {"),
            Some("return {}"),
        );
        let plugin = discovered(tmp.path(), "half");

        let lock = FakeLock::named("tui");
        let mut services = ServiceHost::new(bounds());
        assert_eq!(services.start(&plugin, &lock, None), Ok(true));

        let mut views =
            crate::plugin::PluginHost::from_discovery(discover_in(&[], Some(tmp.path())), bounds());
        assert_eq!(views.start_all(), 0, "the view half fails");
        assert!(
            services.is_running("half"),
            "the service half is unaffected"
        );
    }

    #[test]
    fn service_capabilities_are_separate_from_view_capabilities() {
        let tmp = tempfile::tempdir().unwrap();
        write_plugin(
            tmp.path(),
            "split",
            "capabilities = [\"render\"]\n[service]\ncapabilities = [\"log\"]\n\
             [[panes]]\nid = \"p\"\n",
            Some("return { render = function() return { kind = \"divider\" } end }"),
            Some("return {}"),
        );
        let plugin = discovered(tmp.path(), "split");

        let lock = FakeLock::named("tui");
        let mut services = ServiceHost::new(bounds());
        services.start(&plugin, &lock, None).unwrap();

        let mut views =
            crate::plugin::PluginHost::from_discovery(discover_in(&[], Some(tmp.path())), bounds());
        views.start_all();

        // `log` was granted to the service half only.
        assert_eq!(
            services
                .eval("split", "return type(require('@thurbox').log)")
                .unwrap(),
            "function"
        );
        assert_eq!(
            views
                .eval("split", "return type(require('@thurbox').log)")
                .unwrap(),
            "nil"
        );
    }

    #[test]
    fn starting_twice_is_idempotent() {
        let tmp = tempfile::tempdir().unwrap();
        write_plugin(tmp.path(), "solo", "[service]\n", None, Some("return {}"));
        let plugin = discovered(tmp.path(), "solo");

        let lock = FakeLock::named("tui");
        let mut host = ServiceHost::new(bounds());
        assert_eq!(host.start(&plugin, &lock, None), Ok(true));
        assert_eq!(host.start(&plugin, &lock, None), Ok(true));
        assert_eq!(host.running().len(), 1);
    }
}
