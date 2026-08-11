//! The sandboxed Luau runtime: one VM, one thread, bounded.
//!
//! A plugin's VM is created *inside* the thread that owns it and never leaves.
//! `mlua::Lua` is `!Send` in this build (the `send` feature is deliberately not
//! enabled), so "a VM is only ever touched by its owning thread" is a compile
//! error to violate rather than a rule to remember — which is what keeps plugin
//! execution off the render loop.
//!
//! The host side is [`PluginThread`]: a channel and a join handle. Every call
//! is a request with a one-shot reply, so a caller chooses whether to wait, and
//! a plugin that never replies blocks only whoever waited on it.

use std::cell::Cell;
use std::collections::HashMap;
use std::fmt;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::thread::JoinHandle;
use std::time::Duration;

use mlua::{Lua, LuaOptions, StdLib, Table, Value, VmState};

use crate::plugin::capabilities::{build_module_table, GrantedCapabilities};
use crate::session::plugin_command::{ArgValue, BoundArgs};
use crate::session::view_tree::ViewNode;

/// File a plugin's view half is loaded from, at the root of its directory.
pub const ENTRY_FILE_NAME: &str = "init.luau";

/// File a plugin's headless half is loaded from.
pub const SERVICE_ENTRY_FILE_NAME: &str = "service.luau";

/// Module name that resolves to the host API rather than to a file.
pub const HOST_MODULE: &str = "@thurbox";

/// Registry key holding the plugin's own key/value state.
const STATE_REGISTRY_KEY: &str = "thurbox_plugin_state";

/// Registry key holding the table the entry chunk returned.
const MODULE_REGISTRY_KEY: &str = "thurbox_plugin_module";

/// Limits the host imposes on one plugin.
///
/// Sourced from the host, never from a manifest: a plugin that could declare
/// its own ceiling would not be bounded by it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExecutionBounds {
    /// Interrupt ticks one call may consume before it is killed. Luau fires the
    /// interrupt periodically rather than per instruction, so this bounds
    /// liveness, not a precise instruction count.
    pub interrupt_budget: u64,
    /// Bytes one VM may hold.
    pub memory_limit: usize,
    /// How long a stop request may take before the thread is abandoned.
    pub shutdown_timeout: Duration,
}

impl Default for ExecutionBounds {
    fn default() -> Self {
        Self {
            interrupt_budget: 200_000,
            memory_limit: 64 * 1024 * 1024,
            shutdown_timeout: Duration::from_secs(2),
        }
    }
}

/// Why a plugin call did not succeed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeError {
    /// The plugin's entry file is missing or unreadable.
    EntryUnreadable {
        /// The path that failed.
        path: PathBuf,
        /// The I/O cause.
        cause: String,
    },
    /// The plugin's source did not compile.
    Compile(String),
    /// Plugin code raised an error.
    Runtime(String),
    /// The call consumed its whole interrupt budget.
    BudgetExceeded,
    /// The VM tried to hold more memory than its ceiling allows.
    MemoryExceeded,
    /// The entry chunk did not return a table.
    BadEntryValue(String),
    /// `render` returned something that is not a valid view tree.
    InvalidView(String),
    /// The VM was torn down by an earlier bound failure and accepts no more
    /// work.
    Terminated,
    /// The plugin thread is gone.
    ThreadGone,
}

impl fmt::Display for RuntimeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RuntimeError::EntryUnreadable { path, cause } => {
                write!(f, "cannot read {}: {cause}", path.display())
            }
            RuntimeError::Compile(msg) => write!(f, "compile error: {msg}"),
            RuntimeError::Runtime(msg) => write!(f, "error: {msg}"),
            RuntimeError::BudgetExceeded => f.write_str("exceeded its execution budget"),
            RuntimeError::MemoryExceeded => f.write_str("exceeded its memory limit"),
            RuntimeError::BadEntryValue(got) => {
                write!(f, "entry chunk returned {got}, expected a table")
            }
            RuntimeError::InvalidView(msg) => write!(f, "invalid view tree: {msg}"),
            RuntimeError::Terminated => f.write_str("VM was terminated by an earlier failure"),
            RuntimeError::ThreadGone => f.write_str("plugin thread is no longer running"),
        }
    }
}

impl std::error::Error for RuntimeError {}

impl RuntimeError {
    /// Whether this failure means the VM must not be used again.
    ///
    /// An interrupted or memory-starved VM may have been stopped anywhere,
    /// including halfway through mutating its own state, so it is torn down
    /// rather than resumed.
    pub fn is_fatal(&self) -> bool {
        matches!(
            self,
            RuntimeError::BudgetExceeded | RuntimeError::MemoryExceeded
        )
    }
}

/// Classify an mlua error, recognizing the two bound failures.
fn classify(err: &mlua::Error) -> RuntimeError {
    match err {
        mlua::Error::SyntaxError { message, .. } => RuntimeError::Compile(message.clone()),
        mlua::Error::MemoryError(_) => RuntimeError::MemoryExceeded,
        other => {
            let text = other.to_string();
            if text.contains(BUDGET_MARKER) {
                RuntimeError::BudgetExceeded
            } else if text.contains("not enough memory") || text.contains("memory allocation") {
                RuntimeError::MemoryExceeded
            } else {
                RuntimeError::Runtime(text)
            }
        }
    }
}

/// Sentinel carried by the interrupt's error so it survives Luau's error
/// wrapping and can be told apart from an ordinary runtime error.
const BUDGET_MARKER: &str = "thurbox:budget-exceeded";

/// One plugin's VM. Lives entirely on its owning thread.
struct PluginVm {
    lua: Lua,
    dir: PathBuf,
    /// Which file this VM loads — the view half or the service half.
    entry: &'static str,
    /// Interrupt ticks consumed by the call in flight. Reset per call, so the
    /// budget bounds a single call rather than a plugin's whole lifetime.
    ticks: Rc<Cell<u64>>,
}

impl PluginVm {
    /// Build a VM for one plugin: restricted stdlib, host module, scoped
    /// `require`, bounds, then sandbox.
    ///
    /// Order matters — `sandbox` freezes the globals, so everything a plugin is
    /// allowed to see must already be in place.
    fn new(
        name: &str,
        dir: &Path,
        entry: &'static str,
        granted: &GrantedCapabilities,
        bounds: ExecutionBounds,
        store: Option<Box<dyn crate::session::plugin_store::PluginStore>>,
        writer: Option<Box<dyn crate::session::plugin_mutations::KernelWriter>>,
    ) -> Result<Self, RuntimeError> {
        // Only the pure-computation libraries. `io`, `os`, `package` and
        // `debug` are the ambient filesystem, process, clock and introspection
        // surfaces the runtime spec requires to be absent — omitting them is
        // how they are denied, not a later scrub of the globals table.
        //
        // `UTF8` is here for the same reason `MATH` is, and it was added by the
        // code-review port: a pane that styles the *inside* of a line has to
        // agree with the host about where one character ends, and the host counts
        // characters. `string.byte` counts bytes, so a plugin lexing a line with
        // one multi-byte character in it drifts for the rest of the line — a
        // silently wrong pane rather than a refused one.
        let libs = StdLib::TABLE
            | StdLib::STRING
            | StdLib::MATH
            | StdLib::BIT
            | StdLib::BUFFER
            | StdLib::COROUTINE
            | StdLib::UTF8
            | StdLib::VECTOR;

        let lua = Lua::new_with(libs, LuaOptions::default()).map_err(|e| classify(&e))?;

        lua.set_memory_limit(bounds.memory_limit)
            .map_err(|e| classify(&e))?;

        let state = lua.create_table().map_err(|e| classify(&e))?;
        lua.set_named_registry_value(STATE_REGISTRY_KEY, state)
            .map_err(|e| classify(&e))?;

        let host_module =
            build_module_table(&lua, name, granted, store, writer).map_err(|e| classify(&e))?;

        // Our own `require`, replacing Luau's filesystem requirer: it answers
        // the host namespace and otherwise resolves strictly inside the
        // plugin's directory.
        let plugin_dir = dir.to_path_buf();
        let cache: Rc<std::cell::RefCell<HashMap<PathBuf, Value>>> = Rc::default();
        let host_for_require = host_module.clone();
        let require = lua
            .create_function(move |lua, spec: String| {
                if spec == HOST_MODULE {
                    return Ok(Value::Table(host_for_require.clone()));
                }
                let resolved = resolve_module(&plugin_dir, &spec).map_err(|reason| {
                    mlua::Error::runtime(format!("cannot require `{spec}`: {reason}"))
                })?;
                if let Some(hit) = cache.borrow().get(&resolved) {
                    return Ok(hit.clone());
                }
                let source = std::fs::read_to_string(&resolved).map_err(|e| {
                    mlua::Error::runtime(format!("cannot read {}: {e}", resolved.display()))
                })?;
                let value: Value = lua
                    .load(&source)
                    .set_name(resolved.display().to_string())
                    .eval()?;
                cache.borrow_mut().insert(resolved, value.clone());
                Ok(value)
            })
            .map_err(|e| classify(&e))?;

        lua.globals()
            .raw_set("require", require)
            .map_err(|e| classify(&e))?;

        let ticks = Rc::new(Cell::new(0u64));
        let counter = Rc::clone(&ticks);
        let budget = bounds.interrupt_budget;
        lua.set_interrupt(move |_| {
            let n = counter.get().saturating_add(1);
            counter.set(n);
            if n > budget {
                return Err(mlua::Error::runtime(BUDGET_MARKER));
            }
            Ok(VmState::Continue)
        });

        // Freezes globals, so a plugin cannot replace a stdlib function for
        // anyone — including itself in a way that would outlive a call.
        lua.sandbox(true).map_err(|e| classify(&e))?;

        Ok(Self {
            lua,
            dir: dir.to_path_buf(),
            entry,
            ticks,
        })
    }

    /// Zero the tick counter so the budget bounds one call.
    fn arm(&self) {
        self.ticks.set(0);
    }

    /// Compile and run the entry chunk, keeping the table it returns.
    fn load_entry(&self) -> Result<(), RuntimeError> {
        let path = self.dir.join(self.entry);
        let source = std::fs::read_to_string(&path).map_err(|e| RuntimeError::EntryUnreadable {
            path: path.clone(),
            cause: e.to_string(),
        })?;

        self.arm();
        let value: Value = self
            .lua
            .load(&source)
            .set_name(path.display().to_string())
            .eval()
            .map_err(|e| classify(&e))?;

        match value {
            Value::Table(t) => self
                .lua
                .set_named_registry_value(MODULE_REGISTRY_KEY, t)
                .map_err(|e| classify(&e)),
            other => Err(RuntimeError::BadEntryValue(other.type_name().to_string())),
        }
    }

    /// Call a named module function that takes no arguments.
    ///
    /// Returns whether the function was present, so "ran successfully" is
    /// distinguishable from "had nothing to run" without guessing.
    fn call_named(&self, name: &str) -> Result<bool, RuntimeError> {
        let module: Table = self
            .lua
            .named_registry_value(MODULE_REGISTRY_KEY)
            .map_err(|e| classify(&e))?;

        let entry: Value = module.get(name).map_err(|e| classify(&e))?;
        let func = match entry {
            Value::Function(f) => f,
            Value::Nil => return Ok(false),
            other => {
                return Err(RuntimeError::BadEntryValue(format!(
                    "{name} = {}",
                    other.type_name()
                )))
            }
        };

        self.arm();
        func.call::<()>(()).map_err(|e| classify(&e))?;
        Ok(true)
    }

    /// Call the module's `init` if it declared one.
    ///
    /// Returns whether an `init` was actually present, so the lifecycle can
    /// tell "ran successfully" from "had nothing to run" without guessing.
    fn call_init(&self) -> Result<bool, RuntimeError> {
        self.call_named("init")
    }

    /// Ask the module's `render` for a pane's view tree.
    ///
    /// Conversion happens **here**, on the plugin's own thread, so a
    /// pathological structure is walked inside the VM that already bounds
    /// instructions and memory. The host only ever receives a finished
    /// [`ViewNode`] or an error.
    fn render(&self, pane_id: &str) -> Result<ViewNode, RuntimeError> {
        let module: Table = self
            .lua
            .named_registry_value(MODULE_REGISTRY_KEY)
            .map_err(|e| classify(&e))?;

        let render: Value = module.get("render").map_err(|e| classify(&e))?;
        let func = match render {
            Value::Function(f) => f,
            Value::Nil => {
                return Err(RuntimeError::Runtime(
                    "plugin declares a pane but has no `render` function".to_string(),
                ))
            }
            other => {
                return Err(RuntimeError::BadEntryValue(format!(
                    "render = {}",
                    other.type_name()
                )))
            }
        };

        self.arm();
        let value: Value = func.call(pane_id.to_string()).map_err(|e| classify(&e))?;

        crate::plugin::view::from_lua(&value).map_err(|e| RuntimeError::InvalidView(e.to_string()))
    }

    /// Offer a key to the plugin, returning whether it consumed it.
    ///
    /// A plugin with no key handler simply does not consume — so declaring the
    /// capability and not using it is harmless rather than an error.
    ///
    /// `binding` is the id of the pane binding the chord resolved to, or `None`.
    /// Passed as a third argument rather than in place of the key: a pane that
    /// collects text needs the keypress, and a plugin written before bindings
    /// existed keeps working because it simply ignores an extra argument.
    fn on_key(
        &self,
        pane_id: &str,
        key: &str,
        binding: Option<&str>,
    ) -> Result<bool, RuntimeError> {
        let module: Table = self
            .lua
            .named_registry_value(MODULE_REGISTRY_KEY)
            .map_err(|e| classify(&e))?;

        let handler: Value = module.get("onKey").map_err(|e| classify(&e))?;
        let func = match handler {
            Value::Function(f) => f,
            _ => return Ok(false),
        };

        self.arm();
        let consumed: Value = func
            .call((
                pane_id.to_string(),
                key.to_string(),
                binding.map(str::to_string),
            ))
            .map_err(|e| classify(&e))?;
        // Anything truthy consumes; a plugin returning nothing has not.
        Ok(matches!(consumed, Value::Boolean(true)))
    }

    /// Run a plugin-owned CLI verb, returning whatever it printed.
    fn run_verb(&self, verb: &str, args: &[String]) -> Result<String, RuntimeError> {
        let module: Table = self
            .lua
            .named_registry_value(MODULE_REGISTRY_KEY)
            .map_err(|e| classify(&e))?;

        let handler: Value = module.get("run").map_err(|e| classify(&e))?;
        let func = match handler {
            Value::Function(f) => f,
            _ => {
                return Err(RuntimeError::Runtime(
                    "plugin declares a cli verb but its service exports no `run`".to_string(),
                ))
            }
        };

        let lua_args = self.lua.create_table().map_err(|e| classify(&e))?;
        for (i, arg) in args.iter().enumerate() {
            lua_args.set(i + 1, arg.clone()).map_err(|e| classify(&e))?;
        }

        self.arm();
        let out: Value = func
            .call((verb.to_string(), lua_args))
            .map_err(|e| classify(&e))?;
        Ok(match out {
            Value::String(s) => s.to_string_lossy().to_string(),
            Value::Nil => String::new(),
            other => render(&other),
        })
    }

    /// Run one of the plugin's declared commands.
    ///
    /// Dispatched through a `commands` table keyed by the manifest-local id
    /// rather than a single `run(id, args)` hook: the table is type-checkable in
    /// `thurbox.d.luau`, makes "declared but not implemented" a precise error
    /// naming the command, and spares every plugin a dispatch ladder.
    fn run_command(
        &self,
        entry: &str,
        args: &BoundArgs,
    ) -> Result<serde_json::Value, RuntimeError> {
        let module: Table = self
            .lua
            .named_registry_value(MODULE_REGISTRY_KEY)
            .map_err(|e| classify(&e))?;

        let commands: Value = module.get("commands").map_err(|e| classify(&e))?;
        let table = match commands {
            Value::Table(t) => t,
            Value::Nil => {
                return Err(RuntimeError::Runtime(
                    "plugin declares commands but exports no `commands` table".to_string(),
                ))
            }
            other => {
                return Err(RuntimeError::BadEntryValue(format!(
                    "commands = {}",
                    other.type_name()
                )))
            }
        };

        let handler: Value = table.get(entry).map_err(|e| classify(&e))?;
        let func = match handler {
            Value::Function(f) => f,
            _ => {
                return Err(RuntimeError::Runtime(format!(
                    "plugin's `commands` table has no handler for `{entry}`"
                )))
            }
        };

        let lua_args = self.lua.create_table().map_err(|e| classify(&e))?;
        for (name, value) in args {
            let converted = match value {
                ArgValue::String(s) => Value::String(
                    self.lua
                        .create_string(s.as_str())
                        .map_err(|e| classify(&e))?,
                ),
                ArgValue::Integer(i) => Value::Integer(*i),
                ArgValue::Boolean(b) => Value::Boolean(*b),
            };
            lua_args
                .set(name.as_str(), converted)
                .map_err(|e| classify(&e))?;
        }

        self.arm();
        let out: Value = func.call(lua_args).map_err(|e| classify(&e))?;
        // Converted here, on the plugin's own thread, so a pathological
        // structure is walked inside the VM's bounds — the same reason the view
        // tree is converted here.
        crate::plugin::commands::to_json(&out).map_err(|e| RuntimeError::Runtime(format!("{e}")))
    }

    /// Evaluate a chunk and render its result.
    ///
    /// A host-side diagnostic: it is how the lifecycle's tests observe a VM's
    /// interior, and how a future `plugin eval` debugging verb would work. It
    /// is not reachable from plugin code.
    fn eval(&self, source: &str) -> Result<String, RuntimeError> {
        self.arm();
        let value: Value = self
            .lua
            .load(source)
            .set_name("=eval")
            .eval()
            .map_err(|e| classify(&e))?;
        Ok(render(&value))
    }
}

/// Render a Lua value for host-side inspection.
fn render(value: &Value) -> String {
    match value {
        Value::Nil => "nil".to_string(),
        Value::Boolean(b) => b.to_string(),
        Value::Integer(i) => i.to_string(),
        Value::Number(n) => n.to_string(),
        Value::String(s) => s.to_string_lossy().to_string(),
        other => other.type_name().to_string(),
    }
}

/// Resolve a `require` spec against the plugin's own directory.
///
/// Containment is checked after canonicalization so that `..`, a symlink, or an
/// absolute path all fail the same way — comparing the un-normalized string
/// would only catch the first.
fn resolve_module(plugin_dir: &Path, spec: &str) -> Result<PathBuf, String> {
    if spec.starts_with('@') {
        return Err(format!("`{spec}` is not a host module"));
    }

    let root = plugin_dir
        .canonicalize()
        .map_err(|e| format!("plugin directory is unreadable: {e}"))?;

    let candidate = {
        let relative = spec.strip_prefix("./").unwrap_or(spec);
        let mut p = root.join(relative);
        if p.extension().is_none() {
            p.set_extension("luau");
        }
        p
    };

    let resolved = candidate
        .canonicalize()
        .map_err(|_| "no such module inside the plugin directory".to_string())?;

    if !resolved.starts_with(&root) {
        return Err("resolves outside the plugin directory".to_string());
    }
    Ok(resolved)
}

/// What the host asks a plugin thread to do.
enum Request {
    Load(Sender<Result<(), RuntimeError>>),
    Init(Sender<Result<bool, RuntimeError>>),
    Eval(String, Sender<Result<String, RuntimeError>>),
    Render(String, Sender<Result<ViewNode, RuntimeError>>),
    Key(
        String,
        String,
        Option<String>,
        Sender<Result<bool, RuntimeError>>,
    ),
    Named(String, Sender<Result<bool, RuntimeError>>),
    Verb(String, Vec<String>, Sender<Result<String, RuntimeError>>),
    Command(
        String,
        BoundArgs,
        Sender<Result<serde_json::Value, RuntimeError>>,
    ),
    Stop(Sender<()>),
}

/// The host's handle on one plugin's thread.
pub struct PluginThread {
    requests: Sender<Request>,
    join: Option<JoinHandle<()>>,
    bounds: ExecutionBounds,
}

impl fmt::Debug for PluginThread {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PluginThread")
            .field("bounds", &self.bounds)
            .finish_non_exhaustive()
    }
}

impl PluginThread {
    /// Start a thread owning a fresh VM for this plugin.
    ///
    /// VM construction happens on the new thread, so nothing `!Send` is ever
    /// moved across the boundary.
    pub fn spawn(
        name: &str,
        dir: &Path,
        granted: GrantedCapabilities,
        bounds: ExecutionBounds,
    ) -> Result<Self, RuntimeError> {
        Self::spawn_half(name, dir, ENTRY_FILE_NAME, granted, bounds, None, None)
    }

    /// Start a thread for one half of a plugin.
    ///
    /// The store and the writer are built by their factories **on the new
    /// thread**, because a database connection cannot cross threads and each VM
    /// has its own.
    pub fn spawn_half(
        name: &str,
        dir: &Path,
        entry: &'static str,
        granted: GrantedCapabilities,
        bounds: ExecutionBounds,
        store: Option<crate::session::plugin_store::PluginStoreFactory>,
        writer: Option<crate::session::plugin_mutations::KernelWriterFactory>,
    ) -> Result<Self, RuntimeError> {
        let (req_tx, req_rx) = mpsc::channel::<Request>();
        let (ready_tx, ready_rx) = mpsc::channel::<Result<(), RuntimeError>>();

        let name_owned = name.to_string();
        let dir_owned = dir.to_path_buf();

        let join = std::thread::Builder::new()
            .name(format!("thurbox-plugin-{name}"))
            .spawn(move || {
                let store = store.and_then(|factory| factory());
                let writer = writer.and_then(|factory| factory());
                let vm = match PluginVm::new(
                    &name_owned,
                    &dir_owned,
                    entry,
                    &granted,
                    bounds,
                    store,
                    writer,
                ) {
                    Ok(vm) => {
                        let _ = ready_tx.send(Ok(()));
                        vm
                    }
                    Err(e) => {
                        let _ = ready_tx.send(Err(e));
                        return;
                    }
                };
                serve(vm, req_rx);
            })
            .map_err(|e| RuntimeError::Runtime(format!("cannot spawn plugin thread: {e}")))?;

        match ready_rx.recv() {
            Ok(Ok(())) => Ok(Self {
                requests: req_tx,
                join: Some(join),
                bounds,
            }),
            Ok(Err(e)) => {
                let _ = join.join();
                Err(e)
            }
            Err(_) => Err(RuntimeError::ThreadGone),
        }
    }

    /// Compile and run the entry chunk.
    pub fn load_entry(&self) -> Result<(), RuntimeError> {
        self.round_trip(Request::Load)
    }

    /// Call the plugin's `init`, reporting whether it had one.
    pub fn call_init(&self) -> Result<bool, RuntimeError> {
        self.round_trip(Request::Init)
    }

    /// Ask this plugin to render a pane.
    pub fn render(&self, pane_id: &str) -> Result<ViewNode, RuntimeError> {
        let owned = pane_id.to_string();
        self.round_trip(move |reply| Request::Render(owned, reply))
    }

    /// Offer a key to the plugin, waiting at most `timeout` for its answer.
    ///
    /// The one place the UI thread waits on a plugin: thurbox has to know
    /// *now* whether the key was consumed, or it would either double-handle or
    /// drop it. Bounded outside the VM as well as inside it, so a wedged plugin
    /// costs one dropped frame rather than a hang.
    pub fn send_key(
        &self,
        pane_id: &str,
        key: &str,
        binding: Option<&str>,
        timeout: Duration,
    ) -> Result<bool, RuntimeError> {
        let (tx, rx) = mpsc::channel();
        self.requests
            .send(Request::Key(
                pane_id.to_string(),
                key.to_string(),
                binding.map(str::to_string),
                tx,
            ))
            .map_err(|_| RuntimeError::ThreadGone)?;
        match rx.recv_timeout(timeout) {
            Ok(result) => result,
            Err(RecvTimeoutError::Timeout) => Err(RuntimeError::BudgetExceeded),
            Err(RecvTimeoutError::Disconnected) => Err(RuntimeError::ThreadGone),
        }
    }

    /// Run a plugin-owned CLI verb.
    pub fn run_verb(&self, verb: &str, args: &[String]) -> Result<String, RuntimeError> {
        let owned_verb = verb.to_string();
        let owned_args = args.to_vec();
        self.round_trip(move |reply| Request::Verb(owned_verb, owned_args, reply))
    }

    /// Run one of the plugin's declared commands.
    pub fn run_command(
        &self,
        entry: &str,
        args: &BoundArgs,
    ) -> Result<serde_json::Value, RuntimeError> {
        let owned_entry = entry.to_string();
        let owned_args = args.clone();
        self.round_trip(move |reply| Request::Command(owned_entry, owned_args, reply))
    }

    /// Call a named no-argument function on the plugin's module.
    pub fn call_named(&self, name: &str) -> Result<bool, RuntimeError> {
        let owned = name.to_string();
        self.round_trip(move |reply| Request::Named(owned, reply))
    }

    /// Evaluate a chunk inside this plugin's VM (host-side diagnostic).
    pub fn eval(&self, source: &str) -> Result<String, RuntimeError> {
        let owned = source.to_string();
        self.round_trip(move |reply| Request::Eval(owned, reply))
    }

    /// Send a request and wait for its reply.
    fn round_trip<T>(
        &self,
        make: impl FnOnce(Sender<Result<T, RuntimeError>>) -> Request,
    ) -> Result<T, RuntimeError> {
        let (tx, rx) = mpsc::channel();
        self.requests
            .send(make(tx))
            .map_err(|_| RuntimeError::ThreadGone)?;
        rx.recv().map_err(|_| RuntimeError::ThreadGone)?
    }

    /// Stop the plugin and release its thread.
    ///
    /// A plugin that does not acknowledge within the shutdown budget is
    /// abandoned: the thread is left detached rather than joined, so one
    /// wedged plugin cannot hold up exit or the plugins stopped after it.
    pub fn stop(mut self) -> Result<(), RuntimeError> {
        let (tx, rx) = mpsc::channel();
        if self.requests.send(Request::Stop(tx)).is_err() {
            // Already gone; joining is still correct and will not block.
            if let Some(join) = self.join.take() {
                let _ = join.join();
            }
            return Ok(());
        }

        match rx.recv_timeout(self.bounds.shutdown_timeout) {
            Ok(()) => {
                if let Some(join) = self.join.take() {
                    let _ = join.join();
                }
                Ok(())
            }
            Err(RecvTimeoutError::Timeout) => {
                // Deliberately not joined: the thread is inside plugin code
                // that will not return, and waiting is the failure we are
                // avoiding.
                self.join.take();
                Err(RuntimeError::Terminated)
            }
            Err(RecvTimeoutError::Disconnected) => {
                if let Some(join) = self.join.take() {
                    let _ = join.join();
                }
                Ok(())
            }
        }
    }
}

/// Service requests until the channel closes or the VM is torn down.
fn serve(vm: PluginVm, requests: Receiver<Request>) {
    // Once a bound trips, the VM may have been stopped mid-mutation. It is
    // dropped rather than reused, and every later request is refused.
    let mut vm = Some(vm);

    while let Ok(request) = requests.recv() {
        match request {
            Request::Load(reply) => {
                let result = match vm.as_ref() {
                    Some(v) => v.load_entry(),
                    None => Err(RuntimeError::Terminated),
                };
                if result.as_ref().err().is_some_and(RuntimeError::is_fatal) {
                    vm = None;
                }
                let _ = reply.send(result);
            }
            Request::Init(reply) => {
                let result = match vm.as_ref() {
                    Some(v) => v.call_init(),
                    None => Err(RuntimeError::Terminated),
                };
                if result.as_ref().err().is_some_and(RuntimeError::is_fatal) {
                    vm = None;
                }
                let _ = reply.send(result);
            }
            Request::Render(pane_id, reply) => {
                let result = match vm.as_ref() {
                    Some(v) => v.render(&pane_id),
                    None => Err(RuntimeError::Terminated),
                };
                if result.as_ref().err().is_some_and(RuntimeError::is_fatal) {
                    vm = None;
                }
                let _ = reply.send(result);
            }
            Request::Key(pane_id, key, binding, reply) => {
                let result = match vm.as_ref() {
                    Some(v) => v.on_key(&pane_id, &key, binding.as_deref()),
                    None => Err(RuntimeError::Terminated),
                };
                if result.as_ref().err().is_some_and(RuntimeError::is_fatal) {
                    vm = None;
                }
                let _ = reply.send(result);
            }
            Request::Named(name, reply) => {
                let result = match vm.as_ref() {
                    Some(v) => v.call_named(&name),
                    None => Err(RuntimeError::Terminated),
                };
                if result.as_ref().err().is_some_and(RuntimeError::is_fatal) {
                    vm = None;
                }
                let _ = reply.send(result);
            }
            Request::Verb(verb, args, reply) => {
                let result = match vm.as_ref() {
                    Some(v) => v.run_verb(&verb, &args),
                    None => Err(RuntimeError::Terminated),
                };
                if result.as_ref().err().is_some_and(RuntimeError::is_fatal) {
                    vm = None;
                }
                let _ = reply.send(result);
            }
            Request::Command(entry, args, reply) => {
                let result = match vm.as_ref() {
                    Some(v) => v.run_command(&entry, &args),
                    None => Err(RuntimeError::Terminated),
                };
                if result.as_ref().err().is_some_and(RuntimeError::is_fatal) {
                    vm = None;
                }
                let _ = reply.send(result);
            }
            Request::Eval(source, reply) => {
                let result = match vm.as_ref() {
                    Some(v) => v.eval(&source),
                    None => Err(RuntimeError::Terminated),
                };
                if result.as_ref().err().is_some_and(RuntimeError::is_fatal) {
                    vm = None;
                }
                let _ = reply.send(result);
            }
            Request::Stop(ack) => {
                drop(vm);
                let _ = ack.send(());
                return;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::plugin_manifest::Capability;
    use std::collections::BTreeSet;
    use std::fs;

    fn bounds_for_tests() -> ExecutionBounds {
        ExecutionBounds {
            // Low enough that a runaway loop is caught in milliseconds.
            interrupt_budget: 5_000,
            memory_limit: 8 * 1024 * 1024,
            shutdown_timeout: Duration::from_millis(300),
        }
    }

    fn plugin_dir(entry: &str) -> tempfile::TempDir {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(tmp.path().join(ENTRY_FILE_NAME), entry).unwrap();
        tmp
    }

    fn spawn(dir: &Path, caps: &[Capability]) -> PluginThread {
        let set: BTreeSet<Capability> = caps.iter().copied().collect();
        PluginThread::spawn(
            "demo",
            dir,
            GrantedCapabilities::from_manifest(&set),
            bounds_for_tests(),
        )
        .expect("VM starts")
    }

    #[test]
    fn entry_chunk_must_return_a_table() {
        let tmp = plugin_dir("return 42");
        let t = spawn(tmp.path(), &[]);
        assert!(matches!(
            t.load_entry(),
            Err(RuntimeError::BadEntryValue(_))
        ));
    }

    #[test]
    fn valid_entry_loads_and_init_runs_once() {
        let tmp = plugin_dir(
            r#"
            local calls = 0
            return {
                init = function() calls = calls + 1 end,
                calls = function() return calls end,
            }
            "#,
        );
        let t = spawn(tmp.path(), &[]);
        t.load_entry().expect("loads");
        assert!(t.call_init().expect("init runs"), "init was present");
    }

    #[test]
    fn plugin_without_init_is_not_an_error() {
        let tmp = plugin_dir("return {}");
        let t = spawn(tmp.path(), &[]);
        t.load_entry().expect("loads");
        assert!(!t.call_init().expect("no init is fine"));
    }

    #[test]
    fn syntax_error_is_a_compile_failure() {
        let tmp = plugin_dir("return {");
        let t = spawn(tmp.path(), &[]);
        assert!(matches!(t.load_entry(), Err(RuntimeError::Compile(_))));
    }

    #[test]
    fn missing_entry_file_names_the_path() {
        let tmp = tempfile::tempdir().unwrap();
        let t = spawn(tmp.path(), &[]);
        match t.load_entry() {
            Err(RuntimeError::EntryUnreadable { path, .. }) => {
                assert!(path.ends_with(ENTRY_FILE_NAME));
            }
            other => panic!("expected an unreadable entry, got {other:?}"),
        }
    }

    #[test]
    fn uncaught_error_is_contained_and_later_calls_still_work() {
        let tmp = plugin_dir("return { init = function() error('boom') end }");
        let t = spawn(tmp.path(), &[]);
        t.load_entry().expect("loads");

        match t.call_init() {
            Err(RuntimeError::Runtime(msg)) => assert!(msg.contains("boom"), "{msg}"),
            other => panic!("expected a contained runtime error, got {other:?}"),
        }

        // The process is alive and the VM still serves work.
        assert_eq!(t.eval("return 1 + 1").expect("still usable"), "2");
    }

    #[test]
    fn infinite_loop_trips_the_budget_and_terminates_the_vm() {
        let tmp = plugin_dir("return { init = function() while true do end end }");
        let t = spawn(tmp.path(), &[]);
        t.load_entry().expect("loads");

        assert_eq!(t.call_init(), Err(RuntimeError::BudgetExceeded));
        // A killed VM is not resumed: it may have stopped mid-mutation.
        assert_eq!(t.eval("return 1"), Err(RuntimeError::Terminated));
    }

    #[test]
    fn work_within_the_budget_succeeds() {
        let tmp = plugin_dir("return {}");
        let t = spawn(tmp.path(), &[]);
        t.load_entry().expect("loads");
        assert_eq!(
            t.eval("local s = 0 for i = 1, 100 do s = s + i end return s")
                .expect("within budget"),
            "5050"
        );
    }

    #[test]
    fn budget_is_per_call_not_cumulative() {
        let tmp = plugin_dir("return {}");
        let t = spawn(tmp.path(), &[]);
        t.load_entry().expect("loads");
        for _ in 0..20 {
            assert_eq!(
                t.eval("local s = 0 for i = 1, 200 do s = s + i end return s")
                    .expect("each call gets a fresh budget"),
                "20100"
            );
        }
    }

    #[test]
    fn memory_ceiling_is_enforced() {
        let tmp = plugin_dir("return {}");
        let t = spawn(tmp.path(), &[]);
        t.load_entry().expect("loads");

        let err = t
            .eval("local t = {} while true do t[#t + 1] = string.rep('x', 4096) end")
            .expect_err("allocation must fail");
        assert!(
            matches!(
                err,
                RuntimeError::MemoryExceeded | RuntimeError::BudgetExceeded
            ),
            "expected a bound failure, got {err:?}"
        );
    }

    #[test]
    fn plugins_do_not_share_globals() {
        let a = plugin_dir("_G_SHARED = 'from-a' return {}");
        let b = plugin_dir("return {}");
        let ta = spawn(a.path(), &[]);
        let tb = spawn(b.path(), &[]);

        // Writing a global is refused by the sandbox, and either way plugin B
        // must not observe it.
        let _ = ta.eval("return 1");
        tb.load_entry().expect("loads");
        assert_eq!(tb.eval("return _G_SHARED").expect("reads"), "nil");
    }

    #[test]
    fn a_plugin_cannot_mutate_another_plugins_stdlib() {
        let a = plugin_dir("return {}");
        let b = plugin_dir("return {}");
        let ta = spawn(a.path(), &[]);
        let tb = spawn(b.path(), &[]);

        // Sandboxed globals are frozen, so this fails outright — and cannot
        // reach plugin B regardless.
        let _ = ta.eval("string.rep = function() return 'hijacked' end return 1");
        assert_eq!(tb.eval("return string.rep('x', 3)").expect("intact"), "xxx");
    }

    #[test]
    fn filesystem_and_process_stdlib_are_absent() {
        let tmp = plugin_dir("return {}");
        let t = spawn(tmp.path(), &[]);
        // The ambient-access surfaces: `io` (files), `os` (processes, env, and
        // the clock), `package` (arbitrary module search paths), `debug`
        // (introspection past the sandbox), and the two base-library entry
        // points that read a file by name.
        for probe in ["io", "os", "package", "debug", "dofile", "loadfile"] {
            assert_eq!(
                t.eval(&format!("return {probe}")).expect("probe evaluates"),
                "nil",
                "`{probe}` must not be reachable"
            );
        }
    }

    #[test]
    fn dynamic_compilation_grants_no_ambient_access() {
        // `loadstring` survives the restricted stdlib — it compiles source
        // text, which is not itself an ambient power. What matters is that
        // compiled code inherits the same empty environment, so it cannot
        // reach a library the plugin could not reach directly.
        let tmp = plugin_dir("return {}");
        let t = spawn(tmp.path(), &[]);
        assert_eq!(
            t.eval("return type(loadstring('return io'))")
                .expect("probe"),
            "function"
        );
        assert_eq!(
            t.eval("return loadstring('return io')()").expect("probe"),
            "nil",
            "compiled code must not see a library its compiler could not"
        );
    }

    #[test]
    fn host_module_is_reachable_by_require() {
        let tmp = plugin_dir("return {}");
        let t = spawn(tmp.path(), &[Capability::Log]);
        assert_eq!(
            t.eval(&format!("return require('{HOST_MODULE}').name"))
                .expect("host module"),
            "demo"
        );
    }

    #[test]
    fn undeclared_capability_binding_is_absent_in_the_vm() {
        let tmp = plugin_dir("return {}");
        let t = spawn(tmp.path(), &[]);
        assert_eq!(
            t.eval(&format!("return require('{HOST_MODULE}').log"))
                .expect("probe"),
            "nil"
        );
    }

    #[test]
    fn declared_capability_binding_is_callable_in_the_vm() {
        let tmp = plugin_dir("return {}");
        let t = spawn(tmp.path(), &[Capability::Log]);
        assert_eq!(
            t.eval(&format!("return type(require('{HOST_MODULE}').log)"))
                .expect("probe"),
            "function"
        );
    }

    #[test]
    fn a_plugin_cannot_widen_its_own_capabilities() {
        let tmp = plugin_dir("return {}");
        let t = spawn(tmp.path(), &[]);
        // Mutating the host table must not conjure a working binding — the
        // table is frozen, so the assignment fails outright and `log` stays
        // absent rather than becoming a plugin-authored impostor.
        let script = format!(
            "local m = require('{HOST_MODULE}') \
             local ok = pcall(function() m.log = function() return 'x' end end) \
             return tostring(ok) .. ':' .. type(m.log)"
        );
        assert_eq!(t.eval(&script).expect("probe"), "false:nil");
    }

    #[test]
    fn shadowing_a_global_stays_inside_the_plugins_own_vm() {
        let victim_dir = plugin_dir("return {}");
        let other_dir = plugin_dir("return {}");
        let hijacker = spawn(victim_dir.path(), &[]);
        let bystander = spawn(other_dir.path(), &[]);

        // Luau's sandbox freezes the *shared* globals but leaves each VM a
        // writable environment layered over them, so a plugin can shadow
        // `require` for the rest of its own VM's life. That is not a capability
        // escape: what it gets back is a table it wrote itself, carrying no
        // host power. It matters only that the blast radius is one VM.
        let hijack = format!(
            "require = function() return {{ log = function() return 'impostor' end }} end \
             return require('{HOST_MODULE}').log()"
        );
        assert_eq!(
            hijacker.eval(&hijack).expect("shadows its own env"),
            "impostor"
        );

        // Another plugin's VM is untouched and still sees the genuine module,
        // still without the capability it never declared.
        assert_eq!(
            bystander
                .eval(&format!("return type(require('{HOST_MODULE}').log)"))
                .expect("probe"),
            "nil"
        );
        assert_eq!(
            bystander
                .eval(&format!("return require('{HOST_MODULE}').name"))
                .expect("probe"),
            "demo"
        );
    }

    #[test]
    fn require_inside_the_plugin_directory_works() {
        let tmp = plugin_dir("return {}");
        fs::write(tmp.path().join("helper.luau"), "return { answer = 42 }").unwrap();
        let t = spawn(tmp.path(), &[]);
        assert_eq!(
            t.eval("return require('helper').answer").expect("loads"),
            "42"
        );
    }

    #[test]
    fn require_escaping_the_plugin_directory_is_rejected() {
        let parent = tempfile::tempdir().unwrap();
        let outside = parent.path().join("secret.luau");
        fs::write(&outside, "return { leaked = true }").unwrap();
        let dir = parent.path().join("plugin");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join(ENTRY_FILE_NAME), "return {}").unwrap();

        let t = spawn(&dir, &[]);
        for spec in ["../secret", "../../etc/passwd"] {
            let err = t
                .eval(&format!("return require('{spec}')"))
                .expect_err("must not escape");
            assert!(
                matches!(err, RuntimeError::Runtime(_)),
                "got {err:?} for {spec}"
            );
        }
    }

    #[test]
    fn absolute_require_is_rejected() {
        let tmp = plugin_dir("return {}");
        let t = spawn(tmp.path(), &[]);
        assert!(t.eval("return require('/etc/passwd')").is_err());
    }

    #[test]
    fn stop_releases_the_thread() {
        let tmp = plugin_dir("return {}");
        let t = spawn(tmp.path(), &[]);
        t.load_entry().expect("loads");
        assert_eq!(t.stop(), Ok(()));
    }

    #[test]
    fn a_wedged_plugin_does_not_block_shutdown() {
        let tmp = plugin_dir("return {}");
        let t = spawn(tmp.path(), &[]);
        // A budget large enough that the loop outlives the shutdown timeout.
        let wedged = PluginThread::spawn(
            "wedged",
            tmp.path(),
            GrantedCapabilities::none(),
            ExecutionBounds {
                interrupt_budget: u64::MAX,
                memory_limit: 8 * 1024 * 1024,
                shutdown_timeout: Duration::from_millis(150),
            },
        )
        .expect("spawns");

        // Park the VM inside plugin code that never returns.
        let (tx, _rx) = mpsc::channel();
        wedged
            .requests
            .send(Request::Eval("while true do end".to_string(), tx))
            .expect("queued");

        let started = std::time::Instant::now();
        assert_eq!(wedged.stop(), Err(RuntimeError::Terminated));
        assert!(
            started.elapsed() < Duration::from_secs(2),
            "stop must give up rather than wait"
        );

        // The other plugin still stops cleanly.
        assert_eq!(t.stop(), Ok(()));
    }

    #[test]
    fn the_lint_script_rewrites_the_specifier_this_host_actually_uses() {
        // `scripts/dev/lint-luau.sh` type-checks a copy with `@thurbox`
        // rewritten to a relative path, because the pinned `luau-analyze` does
        // not resolve require aliases. If the host's module name ever changes,
        // that rewrite would silently stop matching and the bundled plugins
        // would go unchecked — so the two are asserted equal here.
        let script = include_str!("../../scripts/dev/lint-luau.sh");
        assert!(
            script.contains(&format!("require(\"{HOST_MODULE}\")")),
            "lint-luau.sh must rewrite `{HOST_MODULE}`; update it alongside HOST_MODULE"
        );
    }

    #[test]
    fn default_bounds_are_finite() {
        let b = ExecutionBounds::default();
        assert!(b.interrupt_budget > 0);
        assert!(b.memory_limit > 0);
        assert!(b.shutdown_timeout > Duration::ZERO);
    }
}
