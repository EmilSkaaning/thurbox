//! The plugin state machine and the host that drives it.
//!
//! Every known plugin sits in exactly one [`PluginState`], and a plugin that
//! failed carries which [`Transition`] it failed in and why. That matters more
//! than it sounds: without it, "my plugin isn't running" is answered by
//! inference from missing output, which is how a typo in a manifest turns into
//! an afternoon.
//!
//! Failure is always per-plugin. Nothing one plugin does — a bad manifest, a
//! syntax error, an infinite loop in `init` — stops another from loading or
//! keeps thurbox from starting.

use std::fmt;
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver};

use crate::plugin::capabilities::GrantedCapabilities;
use crate::plugin::discovery::{self, DiscoveredPlugin, DiscoveryOutcome, PluginSource};
use crate::plugin::pane::PluginPane;
use crate::plugin::runtime::{ExecutionBounds, PluginThread, RuntimeError};
use crate::session::plugin_manifest::Capability;
use crate::session::view_tree::ViewNode;
use std::time::Duration;

/// A step between two lifecycle states.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Transition {
    /// Creating the VM and compiling the entry chunk.
    Load,
    /// Calling the plugin's `init`.
    Initialize,
    /// Releasing the VM and its thread.
    Stop,
}

impl fmt::Display for Transition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Transition::Load => "load",
            Transition::Initialize => "initialize",
            Transition::Stop => "stop",
        })
    }
}

/// Where a plugin is in its life.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PluginState {
    /// Found and validated; no VM yet.
    Discovered,
    /// VM created and entry chunk compiled.
    Loaded,
    /// `init` has run (or there was none).
    Running,
    /// Stopped cleanly.
    Stopped,
    /// Failed, and will not be retried.
    Failed {
        /// Which step failed.
        transition: Transition,
        /// Why, rendered for display.
        cause: String,
    },
}

impl PluginState {
    /// Whether this state can serve calls.
    pub fn is_running(&self) -> bool {
        matches!(self, PluginState::Running)
    }

    /// Whether this plugin failed.
    pub fn is_failed(&self) -> bool {
        matches!(self, PluginState::Failed { .. })
    }
}

impl fmt::Display for PluginState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PluginState::Discovered => f.write_str("discovered"),
            PluginState::Loaded => f.write_str("loaded"),
            PluginState::Running => f.write_str("running"),
            PluginState::Stopped => f.write_str("stopped"),
            PluginState::Failed { transition, cause } => {
                write!(f, "failed during {transition}: {cause}")
            }
        }
    }
}

/// What the host can report about one plugin.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginStatus {
    /// The plugin's name.
    pub name: String,
    /// Where its directory is.
    pub dir: PathBuf,
    /// Which source it came from.
    pub source: PluginSource,
    /// Its current state.
    pub state: PluginState,
    /// The capabilities it was granted, in a stable order.
    pub capabilities: Vec<Capability>,
}

/// Everything the host can say about itself, in one value.
///
/// Exists so a caller — the CLI, a future pane — answers "what is installed"
/// and "what was rejected" from a single snapshot, rather than reaching into
/// the host for each half and risking the two disagreeing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginReport {
    /// Every known plugin, each exactly once.
    pub plugins: Vec<PluginStatus>,
    /// What discovery refused, rendered with its cause.
    pub problems: Vec<String>,
}

impl PluginReport {
    /// Whether nothing was discovered and nothing was rejected.
    pub fn is_empty(&self) -> bool {
        self.plugins.is_empty() && self.problems.is_empty()
    }
}

/// One plugin's slot inside the host.
struct Slot {
    plugin: DiscoveredPlugin,
    granted: GrantedCapabilities,
    state: PluginState,
    /// Present only while the plugin holds a VM.
    thread: Option<PluginThread>,
}

impl Slot {
    fn status(&self) -> PluginStatus {
        PluginStatus {
            name: self.plugin.name().to_string(),
            dir: self.plugin.dir.clone(),
            source: self.plugin.source,
            state: self.state.clone(),
            capabilities: self.granted.iter().collect(),
        }
    }

    /// Record a failure, dropping any VM the plugin still held.
    fn fail(&mut self, transition: Transition, cause: impl fmt::Display) {
        self.thread.take();
        self.state = PluginState::Failed {
            transition,
            cause: cause.to_string(),
        };
    }
}

/// The plugin host: every known plugin, its state, and its VM.
pub struct PluginHost {
    slots: Vec<Slot>,
    /// Discovery problems, kept so the host can explain a plugin that never
    /// became a slot at all.
    problems: Vec<String>,
    bounds: ExecutionBounds,
    /// VM renders performed, cumulative.
    ///
    /// Exists so "a hidden pane costs no render" is checkable: a pane filtered
    /// out before the call is indistinguishable, in the returned results, from
    /// one rendered and discarded. Counted inside [`PluginHost::render_pane`], so
    /// no caller can satisfy the rule by skipping in one path and not another.
    render_calls: std::sync::atomic::AtomicU64,
}

impl fmt::Debug for PluginHost {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PluginHost")
            .field("plugins", &self.slots.len())
            .field("problems", &self.problems.len())
            .finish_non_exhaustive()
    }
}

impl PluginHost {
    /// Build a host from a discovery outcome without starting anything.
    ///
    /// Initialization order is discovery order, which is itself deterministic —
    /// and no plugin may depend on another having gone first, so the order is
    /// an implementation detail a plugin cannot observe.
    pub fn from_discovery(outcome: DiscoveryOutcome, bounds: ExecutionBounds) -> Self {
        let slots = outcome
            .loadable
            .into_iter()
            .map(|plugin| Slot {
                granted: GrantedCapabilities::from_manifest(&plugin.manifest.capabilities),
                plugin,
                state: PluginState::Discovered,
                thread: None,
            })
            .collect();

        Self {
            slots,
            problems: outcome.problems.iter().map(|p| p.to_string()).collect(),
            bounds,
            render_calls: std::sync::atomic::AtomicU64::new(0),
        }
    }

    /// Discover from the standard sources and build a host.
    pub fn discover(bounds: ExecutionBounds) -> Self {
        Self::from_discovery(discovery::discover(), bounds)
    }

    /// How many plugins are known.
    pub fn len(&self) -> usize {
        self.slots.len()
    }

    /// Whether no plugin was discovered.
    ///
    /// When true, `start_all` creates no VM and spawns no thread — the cost of
    /// the plugin host with nothing installed is the cost of this check.
    pub fn is_empty(&self) -> bool {
        self.slots.is_empty()
    }

    /// Discovery problems that never became plugins.
    pub fn problems(&self) -> &[String] {
        &self.problems
    }

    /// Every known plugin's status, each exactly once.
    pub fn statuses(&self) -> Vec<PluginStatus> {
        self.slots.iter().map(Slot::status).collect()
    }

    /// One plugin's status.
    pub fn status(&self, name: &str) -> Option<PluginStatus> {
        self.slots
            .iter()
            .find(|s| s.plugin.name() == name)
            .map(Slot::status)
    }

    /// The plugins currently running.
    pub fn running(&self) -> Vec<&str> {
        self.slots
            .iter()
            .filter(|s| s.state.is_running())
            .map(|s| s.plugin.name())
            .collect()
    }

    /// Load and initialize every discovered plugin.
    ///
    /// Returns the number that reached `Running`. A failure anywhere is
    /// recorded against its own plugin and never propagates.
    pub fn start_all(&mut self) -> usize {
        for index in 0..self.slots.len() {
            self.start_one(index);
        }
        self.log_failures();
        self.publish_state_demand();
        self.slots.iter().filter(|s| s.state.is_running()).count()
    }

    /// Tell the kernel what the running set demands of it: whether anything can
    /// read its state, and whether any pane exists whose visibility it owns.
    ///
    /// Answered from the grants the host already holds, and published rather than
    /// queried because the reader is `app`'s tick and asking the host would mean
    /// `app` holding it — across a thread boundary, on the render loop. Called
    /// from every entry point that can change what is running, so a plugin that
    /// failed to start, was reloaded, or was stopped stops being counted.
    ///
    /// Getting this wrong is not a correctness bug in either direction: `true`
    /// with no reader wastes a snapshot per tick, and `false` with a reader hands
    /// the plugin a stale one. Both are recoverable on the next call, which is
    /// why it is derived here rather than tracked incrementally.
    fn publish_state_demand(&self) {
        let running = || self.slots.iter().filter(|s| s.state.is_running());
        let wanted = running().any(|s| s.granted.iter().any(Capability::reads_kernel_state));
        crate::session::pane_context::set_readers_present(wanted);
        // The same answer for the other direction of the boundary: with no pane
        // declared there is nothing for the kernel to keep off screen, so the
        // tick's visibility publisher can return on one atomic load.
        let panes = running().any(|s| !s.plugin.manifest.panes.is_empty());
        crate::session::pane_visibility::set_panes_present(panes);
    }

    /// Everything the host knows, as one snapshot.
    pub fn report(&self) -> PluginReport {
        PluginReport {
            plugins: self.statuses(),
            problems: self.problems.clone(),
        }
    }

    /// Log every plugin that failed to start, and every discovery problem.
    ///
    /// Startup failures have to be discoverable without running a command:
    /// a plugin that silently does not appear is the failure mode the whole
    /// reporting surface exists to prevent.
    fn log_failures(&self) {
        for slot in &self.slots {
            if let PluginState::Failed { transition, cause } = &slot.state {
                tracing::warn!(
                    plugin = slot.plugin.name(),
                    transition = %transition,
                    "plugin failed to start: {cause}"
                );
            }
        }
        for problem in &self.problems {
            tracing::warn!("plugin not loaded: {problem}");
        }
    }

    /// Discover and start every plugin on a worker thread.
    ///
    /// Returns immediately with a receiver for the finished host. Plugin
    /// startup is a sequence of blocking round-trips into VMs — a plugin's
    /// `init` may run for its whole interrupt budget — so it must never happen
    /// on the thread that draws frames. Moving it off makes "plugins do not
    /// delay the first frame" structural rather than a rule to remember.
    pub fn start_detached(bounds: ExecutionBounds) -> Receiver<PluginHost> {
        let (tx, rx) = mpsc::channel();
        // Detached deliberately: nothing waits on this handle, and a plugin
        // wedged in `init` must not keep the process from exiting.
        std::thread::Builder::new()
            .name("thurbox-plugin-startup".to_string())
            .spawn(move || {
                let mut host = PluginHost::discover(bounds);
                host.start_all();
                // A closed channel means the process is already shutting down;
                // dropping the host here still stops its plugins.
                let _ = tx.send(host);
            })
            .map_err(|e| tracing::warn!("cannot start the plugin host: {e}"))
            .ok();
        rx
    }

    /// Drive one plugin from `Discovered` to `Running`.
    fn start_one(&mut self, index: usize) {
        let bounds = self.bounds;
        let slot = &mut self.slots[index];

        // The state machine has no skipping: a plugin that did not load is
        // never initialized.
        if slot.state != PluginState::Discovered {
            return;
        }

        let thread = match PluginThread::spawn(
            slot.plugin.name(),
            &slot.plugin.dir,
            slot.granted.clone(),
            bounds,
        ) {
            Ok(t) => t,
            Err(e) => {
                slot.fail(Transition::Load, e);
                return;
            }
        };

        if let Err(e) = thread.load_entry() {
            slot.fail(Transition::Load, e);
            return;
        }
        slot.thread = Some(thread);
        slot.state = PluginState::Loaded;

        let init_result = slot.thread.as_ref().expect("thread set above").call_init();

        match init_result {
            Ok(_) => slot.state = PluginState::Running,
            Err(e) => slot.fail(Transition::Initialize, e),
        }
    }

    /// Every pane declared by a running plugin, in the host's stable order.
    ///
    /// A pane belonging to a plugin that failed or has not started is absent:
    /// showing an empty frame for a plugin that will never fill it is worse
    /// than not showing it at all.
    pub fn panes(&self) -> Vec<PluginPane> {
        self.slots
            .iter()
            .filter(|s| s.state.is_running())
            .flat_map(|s| {
                let accepts_input = s.granted.has(Capability::Input);
                s.plugin.manifest.panes.iter().map(move |p| {
                    let mut pane = PluginPane::loading(
                        s.plugin.name(),
                        &p.id,
                        p.title.as_deref().unwrap_or(&p.id),
                        p.slot,
                        p.default_visible,
                    );
                    pane.accepts_input = accepts_input;
                    pane
                })
            })
            .collect()
    }

    /// Every keymap entry the running plugins declare, in the host's stable
    /// order.
    ///
    /// Read from each slot's **granted** set rather than from its manifest's
    /// request, because the grant is what delivery will enforce: registering a
    /// binding whose plugin would then be refused a key would put a chord in the
    /// keymap that does nothing.
    pub fn pane_bindings(&self) -> Vec<crate::session::keybindings::PaneBindingDecl> {
        self.slots
            .iter()
            .filter(|s| s.state.is_running())
            .flat_map(|s| {
                super::keymap::pane_bindings_for(
                    &s.plugin.manifest,
                    s.granted.has(Capability::Input),
                )
            })
            .collect()
    }

    /// Render every pane the kernel is showing, returning each result.
    ///
    /// Runs on the caller's thread — a plugin-render worker, never the UI
    /// thread.
    ///
    /// A pane the kernel publishes as hidden is **not** rendered: its VM is not
    /// entered at all. Filtering here rather than after the render is the whole
    /// point — the UI already discards a hidden pane's tree, so rendering one
    /// spends a Luau call per cycle on something nobody can see, and Phase 4
    /// ships seven panes. A pane nothing has been published about is rendered
    /// (see [`crate::session::pane_visibility`]).
    pub fn render_all_panes_collected(&self) -> Vec<(String, String, Result<ViewNode, String>)> {
        self.panes()
            .into_iter()
            .filter(|pane| !crate::session::pane_visibility::is_hidden(&pane.plugin, &pane.id))
            .map(|pane| {
                let result = self
                    .render_pane(&pane.plugin, &pane.id)
                    .map_err(|e| e.to_string());
                (pane.plugin, pane.id, result)
            })
            .collect()
    }

    /// Offer a key to a plugin, returning whether it consumed it.
    ///
    /// Refused unless the plugin declared `input`: the capability bounds what
    /// the *host* hands over, not only what the plugin can reach.
    pub fn send_key(
        &self,
        plugin: &str,
        pane_id: &str,
        key: &str,
        binding: Option<&str>,
        timeout: Duration,
    ) -> Result<bool, RuntimeError> {
        let slot = self
            .slots
            .iter()
            .find(|s| s.plugin.name() == plugin)
            .ok_or(RuntimeError::ThreadGone)?;

        if !slot.granted.has(Capability::Input) {
            return Err(RuntimeError::Runtime(
                "plugin was not granted the `input` capability".to_string(),
            ));
        }

        slot.thread
            .as_ref()
            .ok_or(RuntimeError::ThreadGone)?
            .send_key(pane_id, key, binding, timeout)
    }

    /// Whether a plugin may receive keys.
    pub fn accepts_input(&self, plugin: &str) -> bool {
        self.slots
            .iter()
            .any(|s| s.plugin.name() == plugin && s.granted.has(Capability::Input))
    }

    /// Stop a plugin and start it again from its current source.
    ///
    /// Built on `reset`, which the lifecycle was designed to admit — so "a
    /// reloaded plugin gets a genuinely fresh VM" is the property already
    /// proven there rather than a second, subtly different path.
    pub fn reload(&mut self, name: &str) -> Result<PluginState, RuntimeError> {
        if !self.reset(name) {
            return Err(RuntimeError::ThreadGone);
        }
        let index = self
            .slots
            .iter()
            .position(|s| s.plugin.name() == name)
            .ok_or(RuntimeError::ThreadGone)?;
        self.start_one(index);
        self.log_failures();
        self.publish_state_demand();
        Ok(self.slots[index].state.clone())
    }

    /// Reload every known plugin.
    pub fn reload_all(&mut self) -> Vec<(String, PluginState)> {
        let names: Vec<String> = self
            .slots
            .iter()
            .map(|s| s.plugin.name().to_string())
            .collect();
        names
            .into_iter()
            .map(|name| {
                let state = self.reload(&name).unwrap_or(PluginState::Failed {
                    transition: Transition::Load,
                    cause: "plugin vanished during reload".to_string(),
                });
                (name, state)
            })
            .collect()
    }

    /// Entry-file mtimes for every loaded plugin, for change detection.
    pub fn source_stamps(&self) -> Vec<(String, Option<std::time::SystemTime>)> {
        self.slots
            .iter()
            .map(|s| {
                let entry = s.plugin.dir.join(crate::plugin::runtime::ENTRY_FILE_NAME);
                let stamp = std::fs::metadata(&entry).and_then(|m| m.modified()).ok();
                (s.plugin.name().to_string(), stamp)
            })
            .collect()
    }

    /// Ask a plugin to render one of its panes.
    ///
    /// The render capability is checked here rather than inside the VM: a
    /// plugin that never asked to draw is never asked to, so the capability
    /// bounds what the *host* does as well as what the plugin can reach.
    pub fn render_pane(&self, plugin: &str, pane_id: &str) -> Result<ViewNode, RuntimeError> {
        let slot = self
            .slots
            .iter()
            .find(|s| s.plugin.name() == plugin)
            .ok_or(RuntimeError::ThreadGone)?;

        if !slot.granted.has(Capability::Render) {
            return Err(RuntimeError::Runtime(
                "plugin was not granted the `render` capability".to_string(),
            ));
        }

        let thread = slot.thread.as_ref().ok_or(RuntimeError::ThreadGone)?;
        self.render_calls
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        thread.render(pane_id)
    }

    /// How many times a plugin VM has been entered to render a pane.
    ///
    /// A diagnostic, and the evidence behind "a hidden pane costs no render".
    pub fn render_calls(&self) -> u64 {
        self.render_calls.load(std::sync::atomic::Ordering::Relaxed)
    }

    /// Evaluate a chunk inside one plugin's VM (host-side diagnostic).
    pub fn eval(&self, name: &str, source: &str) -> Result<String, RuntimeError> {
        self.slots
            .iter()
            .find(|s| s.plugin.name() == name)
            .and_then(|s| s.thread.as_ref())
            .ok_or(RuntimeError::ThreadGone)?
            .eval(source)
    }

    /// Stop every running plugin.
    ///
    /// Each stop is independent: one plugin that will not stop is abandoned and
    /// recorded, and the rest are stopped anyway.
    pub fn stop_all(&mut self) {
        for slot in &mut self.slots {
            let Some(thread) = slot.thread.take() else {
                continue;
            };
            match thread.stop() {
                Ok(()) => slot.state = PluginState::Stopped,
                Err(e) => slot.fail(Transition::Stop, e),
            }
        }
        self.publish_state_demand();
    }

    /// Return a stopped or failed plugin to `Discovered` so it can be started
    /// again with a fresh VM.
    ///
    /// Nothing calls this yet — reload is a later change. It exists because the
    /// state machine was designed to admit it, and a lifecycle that cannot be
    /// re-entered would need redesigning rather than extending.
    pub fn reset(&mut self, name: &str) -> bool {
        let Some(slot) = self.slots.iter_mut().find(|s| s.plugin.name() == name) else {
            return false;
        };
        if let Some(thread) = slot.thread.take() {
            let _ = thread.stop();
        }
        slot.state = PluginState::Discovered;
        self.publish_state_demand();
        true
    }
}

impl Drop for PluginHost {
    fn drop(&mut self) {
        self.stop_all();
    }
}

/// `PluginHost` must stay `Send` so startup can run on a worker and hand the
/// finished host back (see `start_detached`). It holds only channel senders,
/// join handles and plain data — never a `Lua`, which is `!Send` by choice so
/// that every VM stays pinned to its own thread. A field that broke this would
/// force plugin startup back onto the UI thread, so it fails the build instead.
const _: fn() = || {
    fn assert_send<T: Send>() {}
    let _ = assert_send::<PluginHost>;
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin::discovery::discover_in;
    use crate::plugin::runtime::ENTRY_FILE_NAME;
    use std::fs;
    use std::path::Path;
    use std::time::Duration;

    fn bounds() -> ExecutionBounds {
        ExecutionBounds {
            interrupt_budget: 5_000,
            memory_limit: 8 * 1024 * 1024,
            shutdown_timeout: Duration::from_millis(300),
        }
    }

    /// Write a plugin directory with a manifest and an entry file.
    fn write_plugin(root: &Path, name: &str, caps: &str, entry: &str) {
        let dir = root.join(name);
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("plugin.toml"),
            format!("name = \"{name}\"\napi_version = 1\n{caps}"),
        )
        .unwrap();
        fs::write(dir.join(ENTRY_FILE_NAME), entry).unwrap();
    }

    fn host_over(root: &Path) -> PluginHost {
        PluginHost::from_discovery(discover_in(&[], Some(root)), bounds())
    }

    #[test]
    fn normal_progression_reaches_running() {
        let tmp = tempfile::tempdir().unwrap();
        write_plugin(tmp.path(), "alpha", "", "return { init = function() end }");

        let mut host = host_over(tmp.path());
        assert_eq!(host.status("alpha").unwrap().state, PluginState::Discovered);
        assert_eq!(host.start_all(), 1);
        assert_eq!(host.status("alpha").unwrap().state, PluginState::Running);
    }

    #[test]
    fn compile_failure_stops_before_initialize() {
        let tmp = tempfile::tempdir().unwrap();
        write_plugin(tmp.path(), "broken", "", "return {");

        let mut host = host_over(tmp.path());
        assert_eq!(host.start_all(), 0);

        match host.status("broken").unwrap().state {
            PluginState::Failed { transition, cause } => {
                assert_eq!(transition, Transition::Load);
                assert!(cause.contains("compile"), "{cause}");
            }
            other => panic!("expected a load failure, got {other:?}"),
        }
    }

    #[test]
    fn init_failure_is_recorded_against_initialize() {
        let tmp = tempfile::tempdir().unwrap();
        write_plugin(
            tmp.path(),
            "angry",
            "",
            "return { init = function() error('nope') end }",
        );

        let mut host = host_over(tmp.path());
        assert_eq!(host.start_all(), 0);

        match host.status("angry").unwrap().state {
            PluginState::Failed { transition, cause } => {
                assert_eq!(transition, Transition::Initialize);
                assert!(cause.contains("nope"), "{cause}");
            }
            other => panic!("expected an init failure, got {other:?}"),
        }
    }

    #[test]
    fn a_plugin_without_init_still_runs() {
        let tmp = tempfile::tempdir().unwrap();
        write_plugin(tmp.path(), "quiet", "", "return {}");

        let mut host = host_over(tmp.path());
        assert_eq!(host.start_all(), 1);
        assert!(host.status("quiet").unwrap().state.is_running());
    }

    #[test]
    fn one_failure_does_not_affect_the_others() {
        let tmp = tempfile::tempdir().unwrap();
        write_plugin(tmp.path(), "good-one", "", "return {}");
        write_plugin(tmp.path(), "bad-one", "", "return {");
        write_plugin(tmp.path(), "good-two", "", "return {}");

        let mut host = host_over(tmp.path());
        assert_eq!(host.start_all(), 2);

        let mut running = host.running();
        running.sort_unstable();
        assert_eq!(running, vec!["good-one", "good-two"]);
        assert!(host.status("bad-one").unwrap().state.is_failed());
    }

    #[test]
    fn every_plugin_failing_still_starts_the_host() {
        let tmp = tempfile::tempdir().unwrap();
        write_plugin(tmp.path(), "one", "", "return {");
        write_plugin(tmp.path(), "two", "", "return {");

        let mut host = host_over(tmp.path());
        assert_eq!(host.start_all(), 0);
        assert_eq!(host.statuses().len(), 2);
        assert!(host.statuses().iter().all(|s| s.state.is_failed()));
    }

    #[test]
    fn every_known_plugin_is_reported_exactly_once() {
        let tmp = tempfile::tempdir().unwrap();
        for n in ["alpha", "beta", "gamma"] {
            write_plugin(tmp.path(), n, "", "return {}");
        }

        let mut host = host_over(tmp.path());
        host.start_all();

        let statuses = host.statuses();
        assert_eq!(statuses.len(), 3);
        let mut names: Vec<&str> = statuses.iter().map(|s| s.name.as_str()).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), 3);
    }

    #[test]
    fn initialization_order_is_stable() {
        let tmp = tempfile::tempdir().unwrap();
        for n in ["zulu", "alpha", "mike"] {
            write_plugin(tmp.path(), n, "", "return {}");
        }

        let order = |root: &Path| {
            let mut h = host_over(root);
            h.start_all();
            h.statuses().into_iter().map(|s| s.name).collect::<Vec<_>>()
        };

        assert_eq!(order(tmp.path()), order(tmp.path()));
        assert_eq!(order(tmp.path()), vec!["alpha", "mike", "zulu"]);
    }

    #[test]
    fn a_plugin_cannot_observe_another_plugins_state() {
        let tmp = tempfile::tempdir().unwrap();
        write_plugin(tmp.path(), "writer", "", "SHARED = 1 return {}");
        write_plugin(tmp.path(), "reader", "", "return {}");

        let mut host = host_over(tmp.path());
        host.start_all();

        assert_eq!(host.eval("reader", "return SHARED").unwrap(), "nil");
    }

    /// The publisher's gate is derived from what is *running*: a plugin that
    /// asks for no kernel state must leave the kernel building no snapshot,
    /// which is what keeps an installed plugin off the idle loop (ADR-27).
    #[test]
    fn a_plugin_that_reads_no_state_leaves_the_publisher_idle() {
        let _lock = crate::session::pane_context::test_lock();
        crate::session::pane_context::clear_for_test();
        let tmp = tempfile::tempdir().unwrap();
        write_plugin(
            tmp.path(),
            "quiet",
            "capabilities = [\"log\"]\n",
            "return {}",
        );
        let mut host = host_over(tmp.path());
        host.start_all();
        assert!(!crate::session::pane_context::readers_present());
        crate::session::pane_context::clear_for_test();
    }

    #[test]
    fn a_plugin_that_reads_state_asks_the_kernel_to_publish() {
        let _lock = crate::session::pane_context::test_lock();
        crate::session::pane_context::clear_for_test();
        let tmp = tempfile::tempdir().unwrap();
        write_plugin(
            tmp.path(),
            "watcher",
            "capabilities = [\"sessions\"]\n",
            "return {}",
        );
        let mut host = host_over(tmp.path());
        host.start_all();
        assert!(crate::session::pane_context::readers_present());

        // And stops asking once nothing is running: a stopped plugin must not
        // keep the kernel gathering state for nobody.
        host.stop_all();
        assert!(!crate::session::pane_context::readers_present());
        crate::session::pane_context::clear_for_test();
    }

    /// A plugin that asked for state but never started must not count either —
    /// the gate is about readers that exist, not requests that were made.
    #[test]
    fn a_failed_plugin_does_not_ask_the_kernel_to_publish() {
        let _lock = crate::session::pane_context::test_lock();
        crate::session::pane_context::clear_for_test();
        let tmp = tempfile::tempdir().unwrap();
        write_plugin(
            tmp.path(),
            "broken",
            "capabilities = [\"sessions\"]\n",
            "error(\"nope\")",
        );
        let mut host = host_over(tmp.path());
        host.start_all();
        assert!(host.statuses()[0].state.is_failed());
        assert!(!crate::session::pane_context::readers_present());
        crate::session::pane_context::clear_for_test();
    }

    #[test]
    fn granted_capabilities_are_reported() {
        let tmp = tempfile::tempdir().unwrap();
        write_plugin(
            tmp.path(),
            "chatty",
            "capabilities = [\"log\"]\n",
            "return {}",
        );

        let mut host = host_over(tmp.path());
        host.start_all();

        assert_eq!(
            host.status("chatty").unwrap().capabilities,
            vec![Capability::Log]
        );
    }

    #[test]
    fn context_exposes_only_declared_capabilities() {
        let tmp = tempfile::tempdir().unwrap();
        write_plugin(
            tmp.path(),
            "chatty",
            "capabilities = [\"log\"]\n",
            "return {}",
        );
        write_plugin(tmp.path(), "silent", "", "return {}");

        let mut host = host_over(tmp.path());
        host.start_all();

        assert_eq!(
            host.eval("chatty", "return type(require('@thurbox').log)")
                .unwrap(),
            "function"
        );
        assert_eq!(
            host.eval("silent", "return type(require('@thurbox').log)")
                .unwrap(),
            "nil"
        );
    }

    #[test]
    fn no_plugins_means_no_threads() {
        let tmp = tempfile::tempdir().unwrap();
        let mut host = host_over(tmp.path());

        assert!(host.is_empty());
        assert_eq!(host.start_all(), 0);
        assert!(host.statuses().is_empty());
    }

    #[test]
    fn clean_shutdown_stops_everything() {
        let tmp = tempfile::tempdir().unwrap();
        write_plugin(tmp.path(), "alpha", "", "return {}");
        write_plugin(tmp.path(), "beta", "", "return {}");

        let mut host = host_over(tmp.path());
        host.start_all();
        host.stop_all();

        for status in host.statuses() {
            assert_eq!(status.state, PluginState::Stopped, "{}", status.name);
        }
        assert!(host.running().is_empty());
    }

    #[test]
    fn discovery_problems_survive_into_the_host() {
        let tmp = tempfile::tempdir().unwrap();
        write_plugin(tmp.path(), "fine", "", "return {}");
        let bad = tmp.path().join("bad");
        fs::create_dir_all(&bad).unwrap();
        fs::write(bad.join("plugin.toml"), "name = \"Bad\"\napi_version = 1").unwrap();

        let host = host_over(tmp.path());

        assert_eq!(host.len(), 1);
        assert_eq!(host.problems().len(), 1);
        assert!(host.problems()[0].contains("lowercase"));
    }

    #[test]
    fn a_reset_plugin_starts_with_a_fresh_vm() {
        let tmp = tempfile::tempdir().unwrap();
        write_plugin(tmp.path(), "counter", "", "return {}");

        let mut host = host_over(tmp.path());
        host.start_all();

        // The registry-backed state is per-VM, so a value stashed before the
        // reset must not survive it.
        host.eval("counter", "return 1").unwrap();
        host.stop_all();

        assert!(host.reset("counter"));
        assert_eq!(
            host.status("counter").unwrap().state,
            PluginState::Discovered
        );
        assert_eq!(host.start_all(), 1);
        assert!(host.status("counter").unwrap().state.is_running());
    }

    #[test]
    fn a_report_carries_both_plugins_and_problems() {
        let tmp = tempfile::tempdir().unwrap();
        write_plugin(tmp.path(), "fine", "", "return {}");
        let bad = tmp.path().join("bad");
        fs::create_dir_all(&bad).unwrap();
        fs::write(bad.join("plugin.toml"), "name = \"Bad\"\napi_version = 1").unwrap();

        let mut host = host_over(tmp.path());
        host.start_all();
        let report = host.report();

        assert_eq!(report.plugins.len(), 1);
        assert_eq!(report.problems.len(), 1);
        assert!(!report.is_empty());
    }

    #[test]
    fn an_empty_report_is_empty() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(host_over(tmp.path()).report().is_empty());
    }

    #[test]
    fn a_hanging_plugin_does_not_stop_another_from_starting() {
        let tmp = tempfile::tempdir().unwrap();
        write_plugin(
            tmp.path(),
            "hangs",
            "",
            "return { init = function() while true do end end }",
        );
        write_plugin(tmp.path(), "quick", "", "return {}");

        let mut host = host_over(tmp.path());
        assert_eq!(host.start_all(), 1, "the healthy plugin must still run");

        assert!(host.status("quick").unwrap().state.is_running());
        match host.status("hangs").unwrap().state {
            PluginState::Failed { transition, .. } => {
                assert_eq!(transition, Transition::Initialize);
            }
            other => panic!("a hung init must trip its budget, got {other:?}"),
        }
    }

    #[test]
    fn a_hung_plugin_still_lets_the_host_report_and_stop() {
        let tmp = tempfile::tempdir().unwrap();
        write_plugin(
            tmp.path(),
            "hangs",
            "",
            "return { init = function() while true do end end }",
        );

        let mut host = host_over(tmp.path());
        host.start_all();

        // Reporting and shutdown both complete rather than blocking on the
        // plugin that would not return.
        assert_eq!(host.report().plugins.len(), 1);
        host.stop_all();
    }

    #[test]
    fn detached_startup_yields_the_same_result_as_starting_inline() {
        // `start_detached` discovers from the real config dir, which a test
        // must not touch. What is asserted here is the property that makes it
        // safe: the host it would send back is `Send`, and an inline start over
        // an explicit root produces the same states the worker would.
        let tmp = tempfile::tempdir().unwrap();
        write_plugin(tmp.path(), "alpha", "", "return {}");

        let mut host = host_over(tmp.path());
        host.start_all();
        let inline = host.report();

        let moved = std::thread::spawn(move || host.report())
            .join()
            .expect("PluginHost crosses a thread boundary");

        assert_eq!(inline, moved);
    }

    /// A plugin that declares one pane and renders the given Luau expression.
    fn write_pane_plugin(root: &Path, name: &str, render_body: &str) {
        let dir = root.join(name);
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("plugin.toml"),
            format!(
                "name = \"{name}\"\napi_version = 1\ncapabilities = [\"render\"]\n\
                 [[panes]]\nid = \"board\"\ntitle = \"Board\"\n"
            ),
        )
        .unwrap();
        fs::write(
            dir.join(ENTRY_FILE_NAME),
            format!("return {{ render = function(paneId) {render_body} end }}"),
        )
        .unwrap();
    }

    /// The kernel keeps a pane off screen, so the host must not spend a VM call
    /// building a tree for it. Asserted on the render count rather than on the
    /// returned results, because a pane filtered out before the call and one
    /// rendered and discarded produce the identical result list — which is how
    /// the discarding version survived until now.
    #[test]
    fn a_hidden_pane_is_not_rendered() {
        use crate::session::pane_visibility as pv;

        let _lock = pv::test_lock();
        pv::clear_for_test();

        let tmp = tempfile::tempdir().unwrap();
        write_pane_plugin(tmp.path(), "alpha", r#"return { kind = "divider" }"#);
        write_pane_plugin(tmp.path(), "beta", r#"return { kind = "divider" }"#);

        let mut host = host_over(tmp.path());
        host.start_all();
        assert_eq!(host.render_calls(), 0);

        // Nothing published: both panes draw, exactly as before this rule.
        assert_eq!(host.render_all_panes_collected().len(), 2);
        assert_eq!(host.render_calls(), 2);

        pv::publish_hidden(vec![pv::HiddenPane::new("beta", "board")]);
        let rendered = host.render_all_panes_collected();
        assert_eq!(
            rendered
                .iter()
                .map(|(p, ..)| p.as_str())
                .collect::<Vec<_>>(),
            vec!["alpha"],
            "only the visible pane is returned"
        );
        assert_eq!(
            host.render_calls(),
            3,
            "and only one VM was entered, not two"
        );

        // Unhiding restores it: the skip is a function of what is published, not
        // a decision the host remembers.
        pv::publish_hidden(Vec::new());
        assert_eq!(host.render_all_panes_collected().len(), 2);
        assert_eq!(host.render_calls(), 5);

        pv::clear_for_test();
    }

    /// The host's own pane list must keep a hidden pane: it is what the picker
    /// that turns the pane back on is built from, and what the stored choice is
    /// applied to.
    #[test]
    fn a_hidden_pane_is_still_declared() {
        use crate::session::pane_visibility as pv;

        let _lock = pv::test_lock();
        pv::clear_for_test();

        let tmp = tempfile::tempdir().unwrap();
        write_pane_plugin(tmp.path(), "alpha", r#"return { kind = "divider" }"#);

        let mut host = host_over(tmp.path());
        host.start_all();
        pv::publish_hidden(vec![pv::HiddenPane::new("alpha", "board")]);

        assert_eq!(host.panes().len(), 1, "a hidden pane still exists");
        assert!(host.render_all_panes_collected().is_empty());

        pv::clear_for_test();
    }

    /// The demand flag mirrors `pane_context`'s: derived from what is running, so
    /// a build whose plugins declare no pane never reaches the publisher.
    #[test]
    fn pane_presence_is_published_from_the_running_set() {
        use crate::session::pane_visibility as pv;

        let _lock = pv::test_lock();
        pv::clear_for_test();

        let tmp = tempfile::tempdir().unwrap();
        write_plugin(
            tmp.path(),
            "paneless",
            "",
            "return { init = function() end }",
        );
        let mut host = host_over(tmp.path());
        host.start_all();
        assert!(!pv::panes_present(), "no pane, so nothing to publish about");

        let tmp = tempfile::tempdir().unwrap();
        write_pane_plugin(tmp.path(), "alpha", r#"return { kind = "divider" }"#);
        let mut host = host_over(tmp.path());
        host.start_all();
        assert!(pv::panes_present());

        host.stop_all();
        assert!(!pv::panes_present(), "and stops being true when it stops");

        pv::clear_for_test();
    }

    #[test]
    fn a_running_plugins_pane_is_offered() {
        let tmp = tempfile::tempdir().unwrap();
        write_pane_plugin(tmp.path(), "board-plugin", r#"return { kind = "divider" }"#);

        let mut host = host_over(tmp.path());
        host.start_all();

        let panes = host.panes();
        assert_eq!(panes.len(), 1);
        assert_eq!(panes[0].plugin, "board-plugin");
        assert_eq!(panes[0].id, "board");
        assert_eq!(panes[0].title, "Board");
    }

    #[test]
    fn a_failed_plugins_pane_is_not_offered() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("broken");
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("plugin.toml"),
            "name = \"broken\"\napi_version = 1\ncapabilities = [\"render\"]\n\
             [[panes]]\nid = \"board\"\n",
        )
        .unwrap();
        fs::write(dir.join(ENTRY_FILE_NAME), "return {").unwrap();

        let mut host = host_over(tmp.path());
        host.start_all();

        assert!(
            host.panes().is_empty(),
            "a pane that can never fill is not shown"
        );
    }

    #[test]
    fn rendering_a_pane_returns_its_tree() {
        let tmp = tempfile::tempdir().unwrap();
        write_pane_plugin(
            tmp.path(),
            "renderer",
            r#"return { kind = "list", children = {
                 { kind = "text", content = "row " .. paneId, style = "accent" },
               }}"#,
        );

        let mut host = host_over(tmp.path());
        host.start_all();

        let tree = host.render_pane("renderer", "board").expect("renders");
        match &tree {
            crate::session::view_tree::ViewNode::List { children, .. } => {
                assert_eq!(children.len(), 1);
                // The pane id is passed in, so one plugin can serve several.
                assert_eq!(
                    children[0],
                    crate::session::view_tree::ViewNode::styled(
                        "row board",
                        crate::session::view_tree::TextStyle {
                            token: Some(crate::session::view_tree::StyleToken::Accent),
                            ..crate::session::view_tree::TextStyle::default()
                        }
                    )
                );
            }
            other => panic!("expected a list, got {other:?}"),
        }
    }

    #[test]
    fn a_render_that_raises_is_contained() {
        let tmp = tempfile::tempdir().unwrap();
        write_pane_plugin(tmp.path(), "angry", "error('render blew up')");

        let mut host = host_over(tmp.path());
        host.start_all();

        match host.render_pane("angry", "board") {
            Err(RuntimeError::Runtime(msg)) => assert!(msg.contains("blew up"), "{msg}"),
            other => panic!("expected a contained error, got {other:?}"),
        }
        // The plugin is still running and can be asked again.
        assert!(host.status("angry").unwrap().state.is_running());
    }

    #[test]
    fn a_render_returning_an_invalid_tree_is_reported_as_such() {
        let tmp = tempfile::tempdir().unwrap();
        write_pane_plugin(tmp.path(), "confused", r#"return { kind = "canvas" }"#);

        let mut host = host_over(tmp.path());
        host.start_all();

        match host.render_pane("confused", "board") {
            Err(RuntimeError::InvalidView(msg)) => assert!(msg.contains("canvas"), "{msg}"),
            other => panic!("expected an invalid-view error, got {other:?}"),
        }
    }

    #[test]
    fn a_render_that_loops_trips_the_budget() {
        let tmp = tempfile::tempdir().unwrap();
        write_pane_plugin(tmp.path(), "spinner", "while true do end");

        let mut host = host_over(tmp.path());
        host.start_all();

        assert_eq!(
            host.render_pane("spinner", "board"),
            Err(RuntimeError::BudgetExceeded)
        );
    }

    #[test]
    fn a_plugin_without_the_render_capability_is_never_asked() {
        let tmp = tempfile::tempdir().unwrap();
        // No pane, no render capability — but something still asks it to draw.
        write_plugin(
            tmp.path(),
            "quiet",
            "",
            "return { render = function() end }",
        );

        let mut host = host_over(tmp.path());
        host.start_all();

        match host.render_pane("quiet", "board") {
            Err(RuntimeError::Runtime(msg)) => assert!(msg.contains("render"), "{msg}"),
            other => panic!("expected a capability refusal, got {other:?}"),
        }
    }

    #[test]
    fn a_plugin_with_a_pane_but_no_render_function_reports_it() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("forgetful");
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("plugin.toml"),
            "name = \"forgetful\"\napi_version = 1\ncapabilities = [\"render\"]\n\
             [[panes]]\nid = \"board\"\n",
        )
        .unwrap();
        fs::write(dir.join(ENTRY_FILE_NAME), "return {}").unwrap();

        let mut host = host_over(tmp.path());
        host.start_all();

        match host.render_pane("forgetful", "board") {
            Err(RuntimeError::Runtime(msg)) => assert!(msg.contains("render"), "{msg}"),
            other => panic!("expected a missing-render error, got {other:?}"),
        }
    }

    #[test]
    fn one_plugins_render_failure_does_not_affect_another() {
        let tmp = tempfile::tempdir().unwrap();
        write_pane_plugin(tmp.path(), "good-render", r#"return { kind = "divider" }"#);
        write_pane_plugin(tmp.path(), "bad-render", "error('nope')");

        let mut host = host_over(tmp.path());
        host.start_all();

        assert!(host.render_pane("bad-render", "board").is_err());
        assert!(
            host.render_pane("good-render", "board").is_ok(),
            "a neighbour's failure must not spread"
        );
    }

    /// A plugin declaring input, whose key handler is the given Luau body.
    fn write_input_plugin(root: &Path, name: &str, on_key: &str) {
        write_input_plugin_with(root, name, on_key, "");
    }

    /// The same, plus extra manifest lines (a `[[keybindings]]` block).
    fn write_input_plugin_with(root: &Path, name: &str, on_key: &str, manifest_extra: &str) {
        let dir = root.join(name);
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("plugin.toml"),
            format!(
                "name = \"{name}\"\napi_version = 1\ncapabilities = [\"render\", \"input\"]\n\
                 [[panes]]\nid = \"board\"\n{manifest_extra}"
            ),
        )
        .unwrap();
        fs::write(
            dir.join(ENTRY_FILE_NAME),
            format!(
                "return {{ render = function() return {{ kind = \"divider\" }} end, \
                 onKey = function(paneId, key, binding) {on_key} end }}"
            ),
        )
        .unwrap();
    }

    fn key_timeout() -> Duration {
        Duration::from_millis(500)
    }

    #[test]
    fn a_plugin_can_consume_a_key() {
        let tmp = tempfile::tempdir().unwrap();
        write_input_plugin(tmp.path(), "keys", "return key == \"j\"");

        let mut host = host_over(tmp.path());
        host.start_all();

        assert_eq!(
            host.send_key("keys", "board", "j", None, key_timeout()),
            Ok(true)
        );
        assert_eq!(
            host.send_key("keys", "board", "k", None, key_timeout()),
            Ok(false)
        );
    }

    /// The point of a binding: the plugin acts on the *name* it declared, so the
    /// chord a user has bound is not part of its code.
    #[test]
    fn a_key_carries_the_binding_it_resolved_to() {
        let tmp = tempfile::tempdir().unwrap();
        write_input_plugin(tmp.path(), "keys", "return binding == \"delete-row\"");

        let mut host = host_over(tmp.path());
        host.start_all();

        assert_eq!(
            host.send_key("keys", "board", "x", Some("delete-row"), key_timeout()),
            Ok(true),
            "the plugin should see the binding, whatever chord delivered it"
        );
        assert_eq!(
            host.send_key("keys", "board", "x", None, key_timeout()),
            Ok(false),
            "a chord that resolved to no binding reports none"
        );
    }

    /// A plugin written before bindings existed takes two arguments; the third
    /// must cost it nothing.
    #[test]
    fn a_two_argument_key_handler_still_works() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("old");
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("plugin.toml"),
            "name = \"old\"\napi_version = 1\ncapabilities = [\"render\", \"input\"]\n\
             [[panes]]\nid = \"board\"\n",
        )
        .unwrap();
        fs::write(
            dir.join(ENTRY_FILE_NAME),
            "return { render = function() return { kind = \"divider\" } end, \
             onKey = function(paneId, key) return key == \"j\" end }",
        )
        .unwrap();

        let mut host = host_over(tmp.path());
        host.start_all();
        assert_eq!(
            host.send_key("old", "board", "j", Some("next"), key_timeout()),
            Ok(true)
        );
    }

    #[test]
    fn declared_keybindings_reach_the_keymap_from_running_plugins() {
        use crate::session::keybindings::{KeyChord, PaneBindingId};

        let tmp = tempfile::tempdir().unwrap();
        write_input_plugin_with(
            tmp.path(),
            "keys",
            "return false",
            "[[keybindings]]\nid = \"next\"\npane = \"board\"\nchord = \"j\"\n",
        );

        let mut host = host_over(tmp.path());
        // Nothing is running yet, so nothing is published — a pane's keys arrive
        // with its pane, and a plugin that never started has neither.
        assert!(host.pane_bindings().is_empty());

        host.start_all();
        let decls = host.pane_bindings();
        assert_eq!(decls.len(), 1);
        assert_eq!(decls[0].id, PaneBindingId::new("keys", "board", "next"));
        assert_eq!(decls[0].chord, KeyChord::parse("j"));
    }

    #[test]
    fn a_plugin_without_a_key_handler_consumes_nothing() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("quiet");
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("plugin.toml"),
            "name = \"quiet\"\napi_version = 1\ncapabilities = [\"input\"]\n",
        )
        .unwrap();
        fs::write(dir.join(ENTRY_FILE_NAME), "return {}").unwrap();

        let mut host = host_over(tmp.path());
        host.start_all();
        assert_eq!(
            host.send_key("quiet", "board", "j", None, key_timeout()),
            Ok(false)
        );
    }

    #[test]
    fn a_plugin_without_the_input_capability_is_never_handed_a_key() {
        let tmp = tempfile::tempdir().unwrap();
        write_plugin(
            tmp.path(),
            "mute",
            "",
            "return { onKey = function() return true end }",
        );

        let mut host = host_over(tmp.path());
        host.start_all();

        assert!(!host.accepts_input("mute"));
        match host.send_key("mute", "board", "j", None, key_timeout()) {
            Err(RuntimeError::Runtime(msg)) => assert!(msg.contains("input"), "{msg}"),
            other => panic!("expected a capability refusal, got {other:?}"),
        }
    }

    #[test]
    fn a_raising_key_handler_is_contained() {
        let tmp = tempfile::tempdir().unwrap();
        write_input_plugin(tmp.path(), "angry", "error(\"key blew up\")");

        let mut host = host_over(tmp.path());
        host.start_all();
        assert!(host
            .send_key("angry", "board", "j", None, key_timeout())
            .is_err());
        // Still alive for the next key.
        assert!(host.status("angry").unwrap().state.is_running());
    }

    #[test]
    fn a_hanging_key_handler_is_bounded() {
        let tmp = tempfile::tempdir().unwrap();
        write_input_plugin(tmp.path(), "hangs", "while true do end");

        let mut host = host_over(tmp.path());
        host.start_all();

        let started = std::time::Instant::now();
        assert!(host
            .send_key("hangs", "board", "j", None, key_timeout())
            .is_err());
        assert!(
            started.elapsed() < Duration::from_secs(3),
            "a wedged key handler must not hang the caller"
        );
    }

    #[test]
    fn reload_picks_up_edited_source() {
        let tmp = tempfile::tempdir().unwrap();
        write_plugin(
            tmp.path(),
            "edited",
            "",
            "return { tag = function() return \"before\" end }",
        );

        let mut host = host_over(tmp.path());
        host.start_all();
        assert_eq!(host.eval("edited", "return 1").unwrap(), "1");

        fs::write(
            tmp.path().join("edited").join(ENTRY_FILE_NAME),
            "AFTER = true return {}",
        )
        .unwrap();

        assert_eq!(host.reload("edited").unwrap(), PluginState::Running);
        assert_eq!(host.eval("edited", "return AFTER").unwrap(), "true");
    }

    #[test]
    fn no_state_survives_a_reload() {
        let tmp = tempfile::tempdir().unwrap();
        write_plugin(tmp.path(), "stateful", "", "return {}");

        let mut host = host_over(tmp.path());
        host.start_all();
        host.eval("stateful", "LEFTOVER = 1 return 1").unwrap();
        assert_eq!(host.eval("stateful", "return LEFTOVER").unwrap(), "1");

        host.reload("stateful").unwrap();
        assert_eq!(host.eval("stateful", "return LEFTOVER").unwrap(), "nil");
    }

    #[test]
    fn a_failed_reload_is_recorded_and_recoverable() {
        let tmp = tempfile::tempdir().unwrap();
        write_plugin(tmp.path(), "fragile", "", "return {}");
        write_plugin(tmp.path(), "bystander", "", "return {}");

        let mut host = host_over(tmp.path());
        host.start_all();

        let entry = tmp.path().join("fragile").join(ENTRY_FILE_NAME);
        fs::write(&entry, "return {").unwrap();
        let state = host.reload("fragile").unwrap();
        assert!(matches!(state, PluginState::Failed { .. }), "{state:?}");
        assert!(
            host.status("bystander").unwrap().state.is_running(),
            "a neighbour must be unaffected"
        );

        fs::write(&entry, "return {}").unwrap();
        assert_eq!(host.reload("fragile").unwrap(), PluginState::Running);
    }

    #[test]
    fn reloading_an_unknown_plugin_fails() {
        let tmp = tempfile::tempdir().unwrap();
        let mut host = host_over(tmp.path());
        assert!(host.reload("ghost").is_err());
    }

    #[test]
    fn source_stamps_move_when_a_file_is_edited() {
        let tmp = tempfile::tempdir().unwrap();
        write_plugin(tmp.path(), "watched", "", "return {}");

        let mut host = host_over(tmp.path());
        host.start_all();
        let before = host.source_stamps();
        assert_eq!(before.len(), 1);
        assert!(before[0].1.is_some());

        // Sleep past mtime granularity so the change is observable.
        std::thread::sleep(Duration::from_millis(1100));
        fs::write(
            tmp.path().join("watched").join(ENTRY_FILE_NAME),
            "return { changed = true }",
        )
        .unwrap();

        assert_ne!(host.source_stamps(), before);
    }

    #[test]
    fn state_machine_never_skips_a_transition() {
        let tmp = tempfile::tempdir().unwrap();
        write_plugin(tmp.path(), "alpha", "", "return {}");

        let mut host = host_over(tmp.path());
        host.start_all();
        // Already running: starting again is a no-op, not a second init.
        assert_eq!(host.start_all(), 1);
        assert!(host.status("alpha").unwrap().state.is_running());
    }
}
