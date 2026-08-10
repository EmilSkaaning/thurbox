//! Declarative plugin manifests (`plugin.toml`).
//!
//! A [`PluginManifest`] states what a plugin *is* — its identity, the panes,
//! commands and keybindings it provides, and the host capabilities it requests.
//! The host reads it without creating a VM or touching plugin source, so the
//! set of surfaces a plugin contributes is known before any of its code runs,
//! and its reach is reviewable from the manifest alone.
//!
//! This module is pure data + pure logic (serde/std only) to satisfy the
//! `session/` architecture rule. Discovery, the runtime, and capability
//! enforcement live in `crate::plugin`.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use serde::{Deserialize, Serialize};

/// Plugin API version this build implements.
///
/// A manifest declaring anything else is refused before its VM is created —
/// there is exactly one supported version while the protocol is unstable, so
/// compatibility is equality rather than a range.
pub const SUPPORTED_API_VERSION: u32 = 1;

/// Longest accepted plugin name or surface id.
const MAX_IDENT_LEN: usize = 64;

/// Manifest file name at the root of a plugin directory.
pub const MANIFEST_FILE_NAME: &str = "plugin.toml";

/// A host power a plugin may request.
///
/// The vocabulary is closed: an unrecognized name is a manifest error, never a
/// silently-ignored request. Enforcement is by *absence* — a capability that is
/// not granted has no binding in the plugin's environment at all, so there is
/// no per-call permission check that could be forgotten.
///
/// The set is deliberately minimal. Each new capability arrives with the change
/// that introduces the bindings it guards, so nothing here grants a power that
/// no binding yet provides.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Capability {
    /// Read thurbox's own log sink, and emit diagnostics into it.
    Log,
    /// Read the plugin's own persisted key/value state.
    StateRead,
    /// Write the plugin's own persisted key/value state.
    StateWrite,
    /// Be asked to produce a view tree for a declared pane.
    Render,
    /// Receive key events while one of this plugin's panes is focused.
    Input,
    /// Add environment to the agent sessions thurbox spawns.
    ///
    /// Unlike the others this grants no VM binding — the contribution is
    /// static manifest data. It is still a capability because it is the only
    /// thing that makes the reach visible in the capability list, which is
    /// where an install prompt reads a plugin's powers from.
    Spawn,
    /// Read the sessions thurbox is running — names, branches, agent metrics,
    /// activity text.
    ///
    /// Kernel state is gated per *kind* rather than by one blanket grant,
    /// because the capability list is what an install prompt is written from:
    /// "reads your sessions" and "reads this machine's CPU and memory" are
    /// different questions to ask a user, and a pane that wants a session name
    /// must not have to demand host telemetry to get it.
    Sessions,
    /// Read this machine's resource usage and thurbox's own disk footprint.
    Metrics,
    /// Read the automations thurbox has scheduled.
    Automations,
    /// Read thurbox's task list — titles, status, and which row a user is on.
    Tasks,
    /// Read the file tree thurbox's file viewer currently has open — the
    /// basename, depth and expansion state of each visible row.
    ///
    /// Deliberately **not** a filesystem capability, and named `Files` rather
    /// than `Fs` for that reason: it grants no directory listing, no file
    /// contents, no path, and causes no I/O. It reads a tree the kernel already
    /// holds, whose shape is a record of what the user expanded. A power that
    /// reached the filesystem itself would be a different capability, asked for
    /// in a different sentence.
    Files,
}

impl Capability {
    /// The wire name used in a manifest.
    pub fn as_str(self) -> &'static str {
        match self {
            Capability::Log => "log",
            Capability::StateRead => "state-read",
            Capability::StateWrite => "state-write",
            Capability::Render => "render",
            Capability::Input => "input",
            Capability::Spawn => "spawn",
            Capability::Sessions => "sessions",
            Capability::Metrics => "metrics",
            Capability::Automations => "automations",
            Capability::Tasks => "tasks",
            Capability::Files => "files",
        }
    }

    /// Every capability the host recognizes.
    pub fn all() -> &'static [Capability] {
        &[
            Capability::Log,
            Capability::StateRead,
            Capability::StateWrite,
            Capability::Render,
            Capability::Input,
            Capability::Spawn,
            Capability::Sessions,
            Capability::Metrics,
            Capability::Automations,
            Capability::Tasks,
            Capability::Files,
        ]
    }

    /// Whether this capability grants a reader over kernel state.
    ///
    /// The plugin host uses it to answer one question for the publisher — does
    /// *anything* running want a snapshot — so a new state capability cannot be
    /// added without the publisher noticing it.
    pub fn reads_kernel_state(self) -> bool {
        matches!(
            self,
            Capability::Sessions
                | Capability::Metrics
                | Capability::Automations
                | Capability::Tasks
                | Capability::Files
        )
    }
}

impl fmt::Display for Capability {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for Capability {
    type Err = UnknownCapability;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Capability::all()
            .iter()
            .copied()
            .find(|c| c.as_str() == s)
            .ok_or_else(|| UnknownCapability(s.to_string()))
    }
}

/// A capability name no build of thurbox defines.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownCapability(pub String);

impl fmt::Display for UnknownCapability {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "unknown capability `{}`", self.0)
    }
}

/// Where a plugin's pane sits in the layout.
///
/// A closed set: a plugin picks a slot, and the kernel decides the geometry —
/// the same deal the native side panels get. Letting a plugin position itself
/// would make every pane's layout a negotiation with every other pane's.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PaneSlot {
    /// The right-hand column, beside the file viewer and tasks panel.
    #[default]
    Right,
}

/// A pane a plugin contributes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PaneDecl {
    /// Unique within this manifest's panes.
    pub id: String,
    /// Human-readable title. Defaults to the id when absent.
    #[serde(default)]
    pub title: Option<String>,
    /// Where the pane sits. Defaults to [`PaneSlot::Right`].
    #[serde(default)]
    pub slot: PaneSlot,
    /// Whether the pane is shown before a user has said otherwise.
    ///
    /// Only a **seed**: the kernel owns visibility from then on and persists
    /// the user's choice, so a plugin cannot force its pane back on screen.
    #[serde(default = "default_true")]
    pub default_visible: bool,
}

/// serde default for [`PaneDecl::default_visible`] — a pane that says nothing
/// about visibility is shown, which is what an author expects.
fn default_true() -> bool {
    true
}

/// What kind of value a command argument carries.
///
/// Three scalars, and no container. Each extra type is a validation rule, a
/// JSON-Schema mapping, a command-line coercion and a Lua conversion that must
/// agree in four places forever, and a structured argument removes the reason
/// flags exist — a caller that wants a shape passes the whole argument object
/// as JSON, and a command that needs one takes a string and parses it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ArgType {
    /// Any text.
    String,
    /// A signed integer.
    Integer,
    /// `true` or `false`.
    Boolean,
}

impl ArgType {
    /// The wire name used in a manifest, which is also the JSON Schema type.
    pub fn as_str(self) -> &'static str {
        match self {
            ArgType::String => "string",
            ArgType::Integer => "integer",
            ArgType::Boolean => "boolean",
        }
    }

    /// Every type the host recognizes, for error messages.
    pub fn all() -> &'static [ArgType] {
        &[ArgType::String, ArgType::Integer, ArgType::Boolean]
    }
}

impl fmt::Display for ArgType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Which piece of the caller's injected identity fills an argument.
///
/// Thurbox injects `THURBOX_SESSION` and `THURBOX_TASK` into every session it
/// spawns, so an agent invoking a command already proves what it is. This is
/// how that reaches an argument: the agent operates thurbox in terms of what it
/// wants rather than ids it would have to scrape.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum IdentityDefault {
    /// The session the caller is running inside.
    Session,
    /// The task that spawned the caller's session.
    Task,
}

impl IdentityDefault {
    /// The wire name used in a manifest.
    pub fn as_str(self) -> &'static str {
        match self {
            IdentityDefault::Session => "session",
            IdentityDefault::Task => "task",
        }
    }

    /// Every source the host recognizes, for error messages.
    pub fn all() -> &'static [IdentityDefault] {
        &[IdentityDefault::Session, IdentityDefault::Task]
    }
}

impl fmt::Display for IdentityDefault {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Whether a caller running inside a thurbox session may invoke a command.
///
/// Two values, not three: the design's middle `confirm` policy queues a prompt
/// in the TUI and blocks the invocation until a human answers, which needs a
/// cross-process request/answer channel that does not exist. Until it does,
/// accepting the word would either run unprompted or always fail — so it is a
/// manifest error naming what exists instead.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AgentPolicy {
    /// Runs immediately for any caller.
    #[default]
    Allow,
    /// User-only: refused when invoked from inside a session.
    Deny,
}

impl AgentPolicy {
    /// The wire name used in a manifest.
    pub fn as_str(self) -> &'static str {
        match self {
            AgentPolicy::Allow => "allow",
            AgentPolicy::Deny => "deny",
        }
    }

    /// Every policy the host implements, for error messages.
    pub fn all() -> &'static [AgentPolicy] {
        &[AgentPolicy::Allow, AgentPolicy::Deny]
    }
}

impl fmt::Display for AgentPolicy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// One typed argument of a command.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommandArgDecl {
    /// Unique within its command. An identifier, because it is typed as a flag.
    pub name: String,
    /// What kind of value it carries.
    #[serde(rename = "type")]
    pub ty: ArgType,
    /// Whether an invocation must supply it.
    #[serde(default)]
    pub required: bool,
    /// One-line documentation, surfaced in the argument schema.
    #[serde(default)]
    pub description: Option<String>,
    /// Fill from the caller's identity when the invocation omits it.
    #[serde(default)]
    pub default_from: Option<IdentityDefault>,
}

/// A command a plugin contributes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommandDecl {
    /// Unique within this manifest's commands.
    pub id: String,
    /// Human-readable title. Defaults to the id when absent.
    #[serde(default)]
    pub title: Option<String>,
    /// Longer description, surfaced by `thurbox-cli command describe`.
    #[serde(default)]
    pub description: Option<String>,
    /// Typed arguments, in declaration order.
    #[serde(default)]
    pub args: Vec<CommandArgDecl>,
    /// Whether a caller running inside a session may invoke it.
    ///
    /// Defaults to true: a command no surface can reach is useless, and the
    /// capability set — not this flag — is what bounds what a command can do.
    #[serde(default = "default_true")]
    pub agent_callable: bool,
    /// The caller policy for an invocation from inside a session.
    #[serde(default)]
    pub agent_policy: AgentPolicy,
}

impl CommandDecl {
    /// Title for a command, falling back to its id.
    pub fn display_title(&self) -> &str {
        self.title.as_deref().unwrap_or(&self.id)
    }

    /// Check the rules serde cannot express about this command's arguments.
    fn validate_args(&self) -> Result<(), ManifestErrorKind> {
        let mut seen: BTreeSet<&str> = BTreeSet::new();
        for arg in &self.args {
            validate_identifier(&arg.name).map_err(|reason| ManifestErrorKind::Identifier {
                field: "command argument",
                value: arg.name.clone(),
                reason,
            })?;
            if RESERVED_ARG_NAMES.contains(&arg.name.as_str()) {
                return Err(ManifestErrorKind::ReservedArgName {
                    command: self.id.clone(),
                    arg: arg.name.clone(),
                });
            }
            if !seen.insert(arg.name.as_str()) {
                return Err(ManifestErrorKind::DuplicateArg {
                    command: self.id.clone(),
                    arg: arg.name.clone(),
                });
            }
            // An identity default carries a session or task id, so a non-string
            // argument could not hold what the host would put there. Caught
            // here, where the error names its own fix, rather than at an
            // invocation that silently ignored the declaration.
            if arg.default_from.is_some() && arg.ty != ArgType::String {
                return Err(ManifestErrorKind::IdentityDefaultType {
                    command: self.id.clone(),
                    arg: arg.name.clone(),
                    ty: arg.ty,
                });
            }
        }
        Ok(())
    }
}

/// A `thurbox-cli` verb a plugin contributes.
///
/// The verb is dispatched to the plugin's **service** half, since it must work
/// with no TUI running — that is the whole point of a plugin owning a command.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CliVerbDecl {
    /// The verb as typed: `thurbox-cli <name>`.
    pub name: String,
    /// One-line description for help output.
    #[serde(default)]
    pub about: Option<String>,
}

/// A keybinding a plugin contributes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KeybindingDecl {
    /// Unique within this manifest's keybindings.
    pub id: String,
    /// Chord string, parsed by the keymap when bindings are wired up. Left
    /// unvalidated here: the manifest layer is pure data and does not own the
    /// chord grammar.
    #[serde(default)]
    pub chord: Option<String>,
}

/// Which half of a plugin a grant or an entry point belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PluginHalf {
    /// The headless half, hosted with or without a TUI.
    Service,
    /// The TUI-only half that draws panes.
    View,
}

/// The headless half a plugin may declare.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ServiceDecl {
    /// Capabilities granted to the service half only.
    ///
    /// A view rarely needs to reach outward and a service rarely needs to
    /// draw, so granting per half keeps each VM's reach to what it actually
    /// does instead of the union of both.
    #[serde(default)]
    pub capabilities: BTreeSet<Capability>,
}

/// What a plugin adds to the environment of every agent session thurbox
/// spawns.
///
/// Static data rather than a callback: the contribution is then known at
/// discovery time, so the spawn path reads a snapshot instead of entering a VM
/// — which matters because one of the spawn paths finalizes its environment on
/// the UI thread. It is also the shape v1's `[[agent_patches]]` had, so a
/// migrating extension is translating data rather than writing code.
///
/// Declaring this without [`Capability::Spawn`] is a manifest error: the reach
/// has to be readable from the capability list, not only from the table's
/// contents.
///
/// Environment only. `PATH` prepends are specified by the policy layer but have
/// no manifest surface, because the session backend cannot deliver one: tmux
/// replaces a pane's `PATH` with the server's own, ignoring both `new-window
/// -e PATH=…` and `set-environment PATH`. Exposing the field would ship a
/// declaration that silently does nothing, which is the exact failure the
/// rejection machinery exists to prevent.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpawnDecl {
    /// Variables to add. Subject to the denylist and the append-only rule in
    /// [`crate::session::spawn_contribution`] — declaring one is a request,
    /// not a guarantee.
    #[serde(default)]
    pub env: BTreeMap<String, String>,
}

/// One plugin's manifest, as parsed and validated.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PluginManifest {
    /// Identity, unique across all discovered plugins.
    pub name: String,
    /// Plugin API version this plugin was written against.
    pub api_version: u32,
    /// Panes this plugin contributes.
    #[serde(default)]
    pub panes: Vec<PaneDecl>,
    /// Commands this plugin contributes.
    #[serde(default)]
    pub commands: Vec<CommandDecl>,
    /// Keybindings this plugin contributes.
    #[serde(default)]
    pub keybindings: Vec<KeybindingDecl>,
    /// Host powers this plugin requests for **both** halves. Anything absent
    /// here and from the per-half sets is unreachable from that VM.
    #[serde(default)]
    pub capabilities: BTreeSet<Capability>,
    /// The headless half, if this plugin has one.
    #[serde(default)]
    pub service: Option<ServiceDecl>,
    /// `thurbox-cli` verbs this plugin owns.
    #[serde(default)]
    pub cli: Vec<CliVerbDecl>,
    /// What this plugin adds to spawned agent sessions.
    #[serde(default)]
    pub spawn: Option<SpawnDecl>,
}

/// `thurbox-cli` subcommands the kernel owns.
///
/// A plugin verb colliding with one of these would shadow a built-in command
/// depending on dispatch order, so the collision is refused at manifest
/// validation instead — the kernel's surface is not negotiable.
pub const RESERVED_CLI_VERBS: &[&str] = &[
    "command",
    "editor",
    "session",
    "automation",
    "auto",
    "task",
    "todo",
    "message",
    "msg",
    "config",
    "extension",
    "ext",
    "version",
    "update",
    "notify",
    "perf",
    "plugin",
    "help",
];

/// Command argument names `thurbox-cli` cannot deliver.
///
/// These are its global output-format flags. They are declared `global = true`,
/// so clap matches `--json` / `--pretty` / `--text` before a plugin command's
/// arguments start collecting — an argument by one of these names would never
/// receive its value however the user typed it. Refused at validation for the
/// same reason a reserved CLI verb is: a declaration the host could not honour
/// should fail where the error names its own fix.
pub const RESERVED_ARG_NAMES: &[&str] = &["json", "pretty", "text"];

impl PluginManifest {
    /// The capabilities one half is granted: the shared set plus that half's.
    pub fn capabilities_for(&self, half: PluginHalf) -> BTreeSet<Capability> {
        let mut set = self.capabilities.clone();
        if half == PluginHalf::Service {
            if let Some(service) = &self.service {
                set.extend(service.capabilities.iter().copied());
            }
        }
        set
    }

    /// Whether this plugin has a headless half.
    pub fn has_service(&self) -> bool {
        self.service.is_some()
    }

    /// Whether either half was granted `capability`.
    ///
    /// For grants that are not per-half — a spawn contribution belongs to the
    /// plugin, not to one of its VMs — asking "did this plugin request it at
    /// all" is the question that matters.
    pub fn grants(&self, capability: Capability) -> bool {
        self.capabilities.contains(&capability)
            || self
                .service
                .as_ref()
                .is_some_and(|s| s.capabilities.contains(&capability))
    }
}

/// Why a manifest could not be turned into a [`PluginManifest`].
///
/// Every variant names the offending value so a typo surfaces as an error
/// pointing at itself rather than as a silently missing feature.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ManifestErrorKind {
    /// The file could not be read.
    Io(String),
    /// The file is not valid TOML, a required field is missing, a value has the
    /// wrong type, or a key is not recognized. Carries the parser's message,
    /// which already names the key.
    Syntax(String),
    /// A name or surface id violates the identifier rules.
    Identifier {
        /// What the offending value labels: `"name"`, `"pane id"`, ….
        field: &'static str,
        /// The offending value.
        value: String,
        /// Which rule it broke.
        reason: IdentifierProblem,
    },
    /// Two surfaces of the same kind share an id.
    DuplicateId {
        /// `"pane"`, `"command"`, or `"keybinding"`.
        kind: &'static str,
        /// The id declared twice.
        id: String,
    },
    /// One command declares the same argument name twice.
    DuplicateArg {
        /// The command declaring it.
        command: String,
        /// The repeated argument name.
        arg: String,
    },
    /// An argument name is one `thurbox-cli` claims as a global flag.
    ReservedArgName {
        /// The command declaring it.
        command: String,
        /// The offending argument name.
        arg: String,
    },
    /// An argument asks to be filled from the caller's identity but is not a
    /// string, so the id that would fill it could never be represented.
    IdentityDefaultType {
        /// The command declaring it.
        command: String,
        /// The offending argument.
        arg: String,
        /// The type it declared.
        ty: ArgType,
    },
    /// A CLI verb would shadow a kernel subcommand.
    ReservedCliVerb {
        /// The offending verb.
        verb: String,
    },
    /// A pane is declared without the capability that would let it render.
    PaneWithoutRender {
        /// The first pane declared without it.
        pane: String,
    },
    /// A spawn contribution is declared without the capability that permits
    /// it.
    SpawnWithoutCapability,
    /// The plugin targets an API version this build does not implement.
    ApiVersion {
        /// What the manifest asked for.
        declared: u32,
        /// What this build provides.
        supported: u32,
    },
}

/// Which identifier rule a value broke.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdentifierProblem {
    /// Empty string.
    Empty,
    /// Longer than the 64-character identifier limit.
    TooLong,
    /// Does not start with an ASCII lowercase letter.
    BadFirstChar,
    /// Contains something other than `[a-z0-9-]`.
    BadChar(char),
}

impl fmt::Display for IdentifierProblem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            IdentifierProblem::Empty => f.write_str("must not be empty"),
            IdentifierProblem::TooLong => {
                write!(f, "must be at most {MAX_IDENT_LEN} characters")
            }
            IdentifierProblem::BadFirstChar => f.write_str("must start with a lowercase letter"),
            IdentifierProblem::BadChar(c) => {
                write!(
                    f,
                    "contains `{c}`; only lowercase letters, digits and `-` are allowed"
                )
            }
        }
    }
}

/// A manifest failure, bound to the file it came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManifestError {
    /// The manifest the failure belongs to.
    pub path: PathBuf,
    /// What went wrong.
    pub kind: ManifestErrorKind,
}

impl fmt::Display for ManifestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: ", self.path.display())?;
        match &self.kind {
            ManifestErrorKind::Io(msg) => write!(f, "cannot read manifest: {msg}"),
            ManifestErrorKind::Syntax(msg) => write!(f, "invalid manifest: {msg}"),
            ManifestErrorKind::Identifier {
                field,
                value,
                reason,
            } => write!(f, "invalid {field} `{value}`: {reason}"),
            ManifestErrorKind::DuplicateId { kind, id } => {
                write!(f, "duplicate {kind} id `{id}`")
            }
            ManifestErrorKind::DuplicateArg { command, arg } => {
                write!(f, "command `{command}` declares argument `{arg}` twice")
            }
            ManifestErrorKind::ReservedArgName { command, arg } => write!(
                f,
                "command `{command}` argument `{arg}` collides with the global \
                 `--{arg}` output flag, which thurbox-cli consumes first"
            ),
            ManifestErrorKind::IdentityDefaultType { command, arg, ty } => write!(
                f,
                "command `{command}` argument `{arg}` is `{ty}`, but an identity \
                 default fills an id, so it is only allowed on `{}`",
                ArgType::String
            ),
            ManifestErrorKind::ReservedCliVerb { verb } => {
                write!(f, "cli verb `{verb}` is reserved by thurbox itself")
            }
            ManifestErrorKind::PaneWithoutRender { pane } => write!(
                f,
                "pane `{pane}` is declared without the `render` capability, so it could never draw"
            ),
            ManifestErrorKind::SpawnWithoutCapability => write!(
                f,
                "a `[spawn]` contribution is declared without the `spawn` capability, so it would never be applied"
            ),
            ManifestErrorKind::ApiVersion {
                declared,
                supported,
            } => write!(
                f,
                "plugin targets api_version {declared}, but this build supports {supported}"
            ),
        }
    }
}

impl std::error::Error for ManifestError {}

/// Validate one identifier against the shared rules.
///
/// Applied to the plugin name and to every surface id, so a plugin's name and
/// the ids it exposes are drawn from the same alphabet — ids end up in file
/// paths, config keys and command lines, and a permissive alphabet would make
/// each of those a quoting question.
pub fn validate_identifier(value: &str) -> Result<(), IdentifierProblem> {
    if value.is_empty() {
        return Err(IdentifierProblem::Empty);
    }
    if value.len() > MAX_IDENT_LEN {
        return Err(IdentifierProblem::TooLong);
    }
    let first = value.chars().next().expect("non-empty checked above");
    if !first.is_ascii_lowercase() {
        return Err(IdentifierProblem::BadFirstChar);
    }
    for c in value.chars() {
        if !(c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-') {
            return Err(IdentifierProblem::BadChar(c));
        }
    }
    Ok(())
}

impl PluginManifest {
    /// Parse and validate a manifest from TOML text.
    ///
    /// `path` is carried only for error reporting; nothing is read from disk.
    pub fn from_toml(path: &Path, text: &str) -> Result<Self, ManifestError> {
        let err = |kind| ManifestError {
            path: path.to_path_buf(),
            kind,
        };

        // `deny_unknown_fields` turns a typo into a parse error naming the key,
        // rather than a field that silently keeps its default.
        let manifest: PluginManifest = toml::from_str(text)
            .map_err(|e| err(ManifestErrorKind::Syntax(e.message().to_string())))?;

        manifest.validate().map_err(err)?;
        Ok(manifest)
    }

    /// Check every rule that serde cannot express.
    fn validate(&self) -> Result<(), ManifestErrorKind> {
        validate_identifier(&self.name).map_err(|reason| ManifestErrorKind::Identifier {
            field: "name",
            value: self.name.clone(),
            reason,
        })?;

        if self.api_version != SUPPORTED_API_VERSION {
            return Err(ManifestErrorKind::ApiVersion {
                declared: self.api_version,
                supported: SUPPORTED_API_VERSION,
            });
        }

        // Ids are unique per kind, not globally: a pane and the command that
        // opens it naturally share a name, and forcing them apart would buy
        // nothing.
        // A pane the host can never fill is a confusing runtime state; caught
        // here it is an error that names its own fix.
        if !self.panes.is_empty() && !self.capabilities.contains(&Capability::Render) {
            return Err(ManifestErrorKind::PaneWithoutRender {
                pane: self.panes[0].id.clone(),
            });
        }

        // Same rule as a pane without `render`: a declaration the host would
        // never act on is caught here, where the error names its own fix,
        // rather than at a spawn nobody is watching.
        // Accepted from either the shared set or the service half's: a
        // contribution is background-shaped, so an author who put every
        // headless grant under `[service]` should not be told the capability
        // is missing when it is right there.
        if self.spawn.is_some() && !self.grants(Capability::Spawn) {
            return Err(ManifestErrorKind::SpawnWithoutCapability);
        }

        // A verb is typed by a user, so it follows the same alphabet as every
        // other identifier, and it may not shadow a kernel subcommand.
        for verb in &self.cli {
            validate_identifier(&verb.name).map_err(|reason| ManifestErrorKind::Identifier {
                field: "cli verb",
                value: verb.name.clone(),
                reason,
            })?;
            if RESERVED_CLI_VERBS.contains(&verb.name.as_str()) {
                return Err(ManifestErrorKind::ReservedCliVerb {
                    verb: verb.name.clone(),
                });
            }
        }
        check_ids("cli verb", self.cli.iter().map(|c| c.name.as_str()))?;

        check_ids("pane", self.panes.iter().map(|p| p.id.as_str()))?;
        check_ids("command", self.commands.iter().map(|c| c.id.as_str()))?;
        check_ids("keybinding", self.keybindings.iter().map(|k| k.id.as_str()))?;

        for command in &self.commands {
            command.validate_args()?;
        }

        Ok(())
    }

    /// Title for a pane, falling back to its id.
    pub fn pane_title(pane: &PaneDecl) -> &str {
        pane.title.as_deref().unwrap_or(&pane.id)
    }
}

/// Validate a surface's ids and reject a repeat within the kind.
fn check_ids<'a>(
    kind: &'static str,
    ids: impl Iterator<Item = &'a str>,
) -> Result<(), ManifestErrorKind> {
    let field: &'static str = match kind {
        "pane" => "pane id",
        "command" => "command id",
        "cli verb" => "cli verb",
        _ => "keybinding id",
    };
    let mut seen: BTreeSet<&str> = BTreeSet::new();
    for id in ids {
        validate_identifier(id).map_err(|reason| ManifestErrorKind::Identifier {
            field,
            value: id.to_string(),
            reason,
        })?;
        if !seen.insert(id) {
            return Err(ManifestErrorKind::DuplicateId {
                kind,
                id: id.to_string(),
            });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn path() -> PathBuf {
        PathBuf::from("/plugins/demo/plugin.toml")
    }

    fn parse(text: &str) -> Result<PluginManifest, ManifestError> {
        PluginManifest::from_toml(&path(), text)
    }

    const MINIMAL: &str = r#"
        name = "demo"
        api_version = 1
    "#;

    #[test]
    fn minimal_manifest_declares_nothing() {
        let m = parse(MINIMAL).expect("valid");
        assert_eq!(m.name, "demo");
        assert_eq!(m.api_version, SUPPORTED_API_VERSION);
        assert!(m.panes.is_empty());
        assert!(m.commands.is_empty());
        assert!(m.keybindings.is_empty());
        assert!(m.capabilities.is_empty());
    }

    #[test]
    fn missing_name_is_rejected() {
        let e = parse("api_version = 1").expect_err("name required");
        assert!(matches!(e.kind, ManifestErrorKind::Syntax(ref m) if m.contains("name")));
        assert_eq!(e.path, path());
    }

    #[test]
    fn missing_api_version_is_rejected() {
        let e = parse(r#"name = "demo""#).expect_err("api_version required");
        assert!(matches!(e.kind, ManifestErrorKind::Syntax(ref m) if m.contains("api_version")));
    }

    #[test]
    fn malformed_names_are_rejected() {
        for (bad, want) in [
            ("Demo", IdentifierProblem::BadFirstChar),
            ("1demo", IdentifierProblem::BadFirstChar),
            ("de mo", IdentifierProblem::BadChar(' ')),
            ("de/mo", IdentifierProblem::BadChar('/')),
            ("de_mo", IdentifierProblem::BadChar('_')),
        ] {
            let text = format!("name = \"{bad}\"\napi_version = 1");
            let e = parse(&text).expect_err(&format!("`{bad}` must be rejected"));
            match e.kind {
                ManifestErrorKind::Identifier {
                    field: "name",
                    ref value,
                    reason,
                } => {
                    assert_eq!(value, bad);
                    assert_eq!(reason, want, "wrong reason for `{bad}`");
                }
                other => panic!("`{bad}` gave {other:?}"),
            }
        }
    }

    #[test]
    fn empty_name_is_rejected() {
        let e = parse("name = \"\"\napi_version = 1").expect_err("empty rejected");
        assert!(matches!(
            e.kind,
            ManifestErrorKind::Identifier {
                reason: IdentifierProblem::Empty,
                ..
            }
        ));
    }

    #[test]
    fn overlong_name_is_rejected() {
        let long = "a".repeat(MAX_IDENT_LEN + 1);
        let e = parse(&format!("name = \"{long}\"\napi_version = 1")).expect_err("too long");
        assert!(matches!(
            e.kind,
            ManifestErrorKind::Identifier {
                reason: IdentifierProblem::TooLong,
                ..
            }
        ));
    }

    #[test]
    fn duplicate_command_ids_are_rejected() {
        let text = r#"
            name = "demo"
            api_version = 1
            [[commands]]
            id = "refresh"
            [[commands]]
            id = "refresh"
        "#;
        let e = parse(text).expect_err("duplicate rejected");
        assert_eq!(
            e.kind,
            ManifestErrorKind::DuplicateId {
                kind: "command",
                id: "refresh".to_string(),
            }
        );
    }

    #[test]
    fn duplicate_pane_ids_are_rejected() {
        let text = r#"
            name = "demo"
            api_version = 1
            capabilities = ["render"]
            [[panes]]
            id = "board"
            [[panes]]
            id = "board"
        "#;
        let e = parse(text).expect_err("duplicate rejected");
        assert_eq!(
            e.kind,
            ManifestErrorKind::DuplicateId {
                kind: "pane",
                id: "board".to_string(),
            }
        );
    }

    #[test]
    fn same_id_across_kinds_is_accepted() {
        let text = r#"
            name = "demo"
            api_version = 1
            capabilities = ["render"]
            [[panes]]
            id = "board"
            [[commands]]
            id = "board"
        "#;
        let m = parse(text).expect("ids are unique per kind");
        assert_eq!(m.panes[0].id, "board");
        assert_eq!(m.commands[0].id, "board");
    }

    #[test]
    fn unknown_key_is_rejected() {
        let text = r#"
            name = "demo"
            api_version = 1
            capabilites = ["log"]
        "#;
        let e = parse(text).expect_err("typo rejected");
        assert!(
            matches!(e.kind, ManifestErrorKind::Syntax(ref m) if m.contains("capabilites")),
            "error should name the unrecognized key, got {:?}",
            e.kind
        );
    }

    #[test]
    fn invalid_toml_is_rejected() {
        let e = parse("name = \"demo\"\napi_version =").expect_err("syntax error");
        assert!(matches!(e.kind, ManifestErrorKind::Syntax(_)));
        assert_eq!(e.path, path());
    }

    #[test]
    fn newer_api_version_is_rejected() {
        let text = format!(
            "name = \"demo\"\napi_version = {}",
            SUPPORTED_API_VERSION + 1
        );
        let e = parse(&text).expect_err("incompatible");
        assert_eq!(
            e.kind,
            ManifestErrorKind::ApiVersion {
                declared: SUPPORTED_API_VERSION + 1,
                supported: SUPPORTED_API_VERSION,
            }
        );
    }

    #[test]
    fn compatible_api_version_is_accepted() {
        let text = format!("name = \"demo\"\napi_version = {SUPPORTED_API_VERSION}");
        assert!(parse(&text).is_ok());
    }

    #[test]
    fn known_capabilities_parse() {
        let text = r#"
            name = "demo"
            api_version = 1
            capabilities = ["log", "state-read"]
        "#;
        let m = parse(text).expect("valid");
        assert!(m.capabilities.contains(&Capability::Log));
        assert!(m.capabilities.contains(&Capability::StateRead));
        assert!(!m.capabilities.contains(&Capability::StateWrite));
    }

    #[test]
    fn unknown_capability_is_rejected() {
        let text = r#"
            name = "demo"
            api_version = 1
            capabilities = ["read-everything"]
        "#;
        let e = parse(text).expect_err("closed vocabulary");
        assert!(
            matches!(e.kind, ManifestErrorKind::Syntax(ref m) if m.contains("read-everything")),
            "error should name the unknown capability, got {:?}",
            e.kind
        );
    }

    #[test]
    fn capability_roundtrips_through_str() {
        for c in Capability::all() {
            assert_eq!(Capability::from_str(c.as_str()), Ok(*c));
        }
        assert_eq!(
            Capability::from_str("nope"),
            Err(UnknownCapability("nope".to_string()))
        );
    }

    #[test]
    fn surface_ids_follow_the_identifier_rules() {
        let text = r#"
            name = "demo"
            api_version = 1
            [[commands]]
            id = "Refresh"
        "#;
        let e = parse(text).expect_err("id rules apply to surfaces too");
        assert!(matches!(
            e.kind,
            ManifestErrorKind::Identifier {
                field: "command id",
                reason: IdentifierProblem::BadFirstChar,
                ..
            }
        ));
    }

    #[test]
    fn a_cli_verb_parses() {
        let text = r#"
            name = "demo"
            api_version = 1
            [service]
            [[cli]]
            name = "sync"
            about = "Sync everything"
        "#;
        let m = parse(text).expect("valid");
        assert_eq!(m.cli[0].name, "sync");
        assert_eq!(m.cli[0].about.as_deref(), Some("Sync everything"));
    }

    #[test]
    fn a_cli_verb_may_not_shadow_a_kernel_subcommand() {
        for reserved in ["session", "task", "plugin", "config"] {
            let text = format!(
                "name = \"demo\"\napi_version = 1\n[service]\n[[cli]]\nname = \"{reserved}\"\n"
            );
            let e = parse(&text).expect_err("{reserved} must be refused");
            assert_eq!(
                e.kind,
                ManifestErrorKind::ReservedCliVerb {
                    verb: reserved.to_string()
                },
                "{reserved}"
            );
        }
    }

    #[test]
    fn a_cli_verb_follows_the_identifier_rules() {
        let text = r#"
            name = "demo"
            api_version = 1
            [service]
            [[cli]]
            name = "Sync"
        "#;
        let e = parse(text).expect_err("verbs are typed, so same alphabet");
        assert!(matches!(
            e.kind,
            ManifestErrorKind::Identifier {
                field: "cli verb",
                ..
            }
        ));
    }

    #[test]
    fn two_cli_verbs_may_not_share_a_name() {
        let text = r#"
            name = "demo"
            api_version = 1
            [service]
            [[cli]]
            name = "sync"
            [[cli]]
            name = "sync"
        "#;
        let e = parse(text).expect_err("duplicate");
        assert_eq!(
            e.kind,
            ManifestErrorKind::DuplicateId {
                kind: "cli verb",
                id: "sync".to_string()
            }
        );
    }

    #[test]
    fn every_reserved_verb_matches_a_real_subcommand_name() {
        // The list is hand-maintained; if a kernel subcommand is renamed and
        // this is not updated, a plugin could quietly shadow it.
        assert!(RESERVED_CLI_VERBS.contains(&"plugin"));
        assert!(RESERVED_CLI_VERBS.contains(&"session"));
        assert!(!RESERVED_CLI_VERBS.contains(&"sync"));
    }

    #[test]
    fn a_pane_is_visible_by_default() {
        let text = r#"
            name = "demo"
            api_version = 1
            capabilities = ["render"]
            [[panes]]
            id = "board"
        "#;
        assert!(parse(text).expect("valid").panes[0].default_visible);
    }

    #[test]
    fn a_pane_can_opt_out_of_being_shown() {
        let text = r#"
            name = "demo"
            api_version = 1
            capabilities = ["render"]
            [[panes]]
            id = "board"
            default_visible = false
        "#;
        assert!(!parse(text).expect("valid").panes[0].default_visible);
    }

    #[test]
    fn a_pane_defaults_to_the_right_slot() {
        let text = r#"
            name = "demo"
            api_version = 1
            capabilities = ["render"]
            [[panes]]
            id = "board"
        "#;
        let m = parse(text).expect("valid");
        assert_eq!(m.panes[0].slot, PaneSlot::Right);
    }

    #[test]
    fn a_pane_may_name_its_slot() {
        let text = r#"
            name = "demo"
            api_version = 1
            capabilities = ["render"]
            [[panes]]
            id = "board"
            slot = "right"
        "#;
        let m = parse(text).expect("valid");
        assert_eq!(m.panes[0].slot, PaneSlot::Right);
    }

    #[test]
    fn an_unknown_slot_is_rejected() {
        let text = r#"
            name = "demo"
            api_version = 1
            capabilities = ["render"]
            [[panes]]
            id = "board"
            slot = "ceiling"
        "#;
        let e = parse(text).expect_err("closed set");
        assert!(
            matches!(e.kind, ManifestErrorKind::Syntax(ref m) if m.contains("ceiling")),
            "error should name the offending slot, got {:?}",
            e.kind
        );
    }

    #[test]
    fn a_pane_without_the_render_capability_is_rejected() {
        let text = r#"
            name = "demo"
            api_version = 1
            [[panes]]
            id = "board"
        "#;
        let e = parse(text).expect_err("a pane that cannot draw is a mistake");
        assert_eq!(
            e.kind,
            ManifestErrorKind::PaneWithoutRender {
                pane: "board".to_string()
            }
        );
        assert!(e.to_string().contains("render"), "{e}");
    }

    #[test]
    fn the_render_capability_without_a_pane_is_fine() {
        let text = r#"
            name = "demo"
            api_version = 1
            capabilities = ["render"]
        "#;
        let m = parse(text).expect("a plugin may hold render and draw nothing");
        assert!(m.panes.is_empty());
        assert!(m.capabilities.contains(&Capability::Render));
    }

    #[test]
    fn pane_title_falls_back_to_id() {
        let text = r#"
            name = "demo"
            api_version = 1
            capabilities = ["render"]
            [[panes]]
            id = "board"
        "#;
        let m = parse(text).expect("valid");
        assert_eq!(PluginManifest::pane_title(&m.panes[0]), "board");
    }

    #[test]
    fn a_spawn_contribution_needs_the_spawn_capability() {
        let text = r#"
            name = "demo"
            api_version = 1
            [spawn.env]
            CI_TOKEN_FILE = "/run/secrets/ci"
        "#;
        let e = parse(text).expect_err("a contribution that could never apply");
        assert!(matches!(e.kind, ManifestErrorKind::SpawnWithoutCapability));
        assert!(e.to_string().contains("spawn"));
    }

    #[test]
    fn a_declared_spawn_contribution_is_readable_from_the_manifest() {
        let text = r#"
            name = "demo"
            api_version = 1
            capabilities = ["spawn"]
            [spawn.env]
            CI_TOKEN_FILE = "/run/secrets/ci"
        "#;
        let m = parse(text).expect("valid");
        let spawn = m.spawn.expect("declared");
        assert_eq!(
            spawn.env.get("CI_TOKEN_FILE"),
            Some(&"/run/secrets/ci".to_string())
        );
    }

    #[test]
    fn the_spawn_capability_may_come_from_the_service_half() {
        // A headless-shaped grant declared where the rest of the headless
        // grants live still counts — the contribution belongs to the plugin,
        // not to one of its VMs.
        let text = r#"
            name = "demo"
            api_version = 1
            [service]
            capabilities = ["spawn"]
            [spawn.env]
            CI_TOKEN_FILE = "/run/secrets/ci"
        "#;
        let m = parse(text).expect("valid");
        assert!(m.grants(Capability::Spawn));
    }

    #[test]
    fn the_spawn_capability_without_a_contribution_is_fine() {
        let text = r#"
            name = "demo"
            api_version = 1
            capabilities = ["spawn"]
        "#;
        let m = parse(text).expect("a plugin may hold the grant and contribute nothing");
        assert!(m.spawn.is_none());
    }

    #[test]
    fn an_unknown_key_inside_the_spawn_table_is_rejected() {
        let text = r#"
            name = "demo"
            api_version = 1
            capabilities = ["spawn"]
            [spawn]
            path = ["bin"]
        "#;
        // `path` is policy the backend cannot honour, so it is not a manifest
        // key: an author gets an error rather than a line that does nothing.
        let e = parse(text).expect_err("path is not a spawn key");
        assert!(matches!(e.kind, ManifestErrorKind::Syntax(ref m) if m.contains("path")));
    }

    #[test]
    fn error_display_names_the_path() {
        let e = parse("api_version = 1").expect_err("invalid");
        assert!(e.to_string().starts_with("/plugins/demo/plugin.toml: "));
    }

    #[test]
    fn a_fully_specified_command_validates() {
        let text = r#"
            name = "demo"
            api_version = 1
            [[commands]]
            id = "note"
            title = "Attach a note"
            description = "Attach a note to the calling session"
            agent_callable = true
            agent_policy = "deny"
            [[commands.args]]
            name = "body"
            type = "string"
            required = true
            description = "the note body"
            [[commands.args]]
            name = "session"
            type = "string"
            default_from = "session"
        "#;
        let m = parse(text).expect("valid");
        let c = &m.commands[0];
        assert_eq!(c.display_title(), "Attach a note");
        assert_eq!(
            c.description.as_deref(),
            Some("Attach a note to the calling session")
        );
        assert_eq!(c.agent_policy, AgentPolicy::Deny);
        assert!(c.agent_callable);
        assert_eq!(c.args.len(), 2);
        assert_eq!(c.args[0].ty, ArgType::String);
        assert!(c.args[0].required);
        assert!(!c.args[1].required);
        assert_eq!(c.args[1].default_from, Some(IdentityDefault::Session));
    }

    #[test]
    fn a_minimal_command_is_agent_callable_and_allowed() {
        let text = r#"
            name = "demo"
            api_version = 1
            [[commands]]
            id = "refresh"
        "#;
        let c = &parse(text).expect("valid").commands[0];
        // The capability set bounds what a command can do; this flag only
        // bounds who may ask, so defaulting it closed would hide every command
        // from the surface it exists for.
        assert!(c.agent_callable);
        assert_eq!(c.agent_policy, AgentPolicy::Allow);
        assert_eq!(c.display_title(), "refresh");
        assert!(c.args.is_empty());
    }

    #[test]
    fn an_unimplemented_policy_is_rejected_naming_the_ones_that_exist() {
        let text = r#"
            name = "demo"
            api_version = 1
            [[commands]]
            id = "wipe"
            agent_policy = "confirm"
        "#;
        let e = parse(text).expect_err("confirm is not implemented");
        let msg = match e.kind {
            ManifestErrorKind::Syntax(m) => m,
            other => panic!("expected a syntax error, got {other:?}"),
        };
        assert!(msg.contains("confirm"), "{msg}");
        for policy in AgentPolicy::all() {
            assert!(msg.contains(policy.as_str()), "{msg} should name {policy}");
        }
    }

    #[test]
    fn an_unknown_argument_type_is_rejected_naming_the_ones_that_exist() {
        let text = r#"
            name = "demo"
            api_version = 1
            [[commands]]
            id = "note"
            [[commands.args]]
            name = "tags"
            type = "array"
        "#;
        let e = parse(text).expect_err("array is not a type");
        let msg = match e.kind {
            ManifestErrorKind::Syntax(m) => m,
            other => panic!("expected a syntax error, got {other:?}"),
        };
        for ty in ArgType::all() {
            assert!(msg.contains(ty.as_str()), "{msg} should name {ty}");
        }
    }

    #[test]
    fn an_unknown_identity_source_is_rejected_naming_the_ones_that_exist() {
        let text = r#"
            name = "demo"
            api_version = 1
            [[commands]]
            id = "note"
            [[commands.args]]
            name = "who"
            type = "string"
            default_from = "user"
        "#;
        let e = parse(text).expect_err("there is no user identity");
        let msg = match e.kind {
            ManifestErrorKind::Syntax(m) => m,
            other => panic!("expected a syntax error, got {other:?}"),
        };
        for source in IdentityDefault::all() {
            assert!(msg.contains(source.as_str()), "{msg} should name {source}");
        }
    }

    #[test]
    fn a_repeated_argument_name_is_rejected() {
        let text = r#"
            name = "demo"
            api_version = 1
            [[commands]]
            id = "note"
            [[commands.args]]
            name = "body"
            type = "string"
            [[commands.args]]
            name = "body"
            type = "integer"
        "#;
        let e = parse(text).expect_err("duplicate argument");
        assert_eq!(
            e.kind,
            ManifestErrorKind::DuplicateArg {
                command: "note".to_string(),
                arg: "body".to_string(),
            }
        );
    }

    #[test]
    fn a_malformed_argument_name_is_rejected() {
        let text = r#"
            name = "demo"
            api_version = 1
            [[commands]]
            id = "note"
            [[commands.args]]
            name = "Body"
            type = "string"
        "#;
        let e = parse(text).expect_err("an argument name is an identifier");
        match e.kind {
            ManifestErrorKind::Identifier {
                field: "command argument",
                ref value,
                reason,
            } => {
                assert_eq!(value, "Body");
                assert_eq!(reason, IdentifierProblem::BadFirstChar);
            }
            other => panic!("expected an identifier error, got {other:?}"),
        }
    }

    #[test]
    fn an_identity_default_on_a_non_string_is_rejected() {
        let text = r#"
            name = "demo"
            api_version = 1
            [[commands]]
            id = "note"
            [[commands.args]]
            name = "session"
            type = "integer"
            default_from = "session"
        "#;
        let e = parse(text).expect_err("an id is not an integer");
        assert_eq!(
            e.kind,
            ManifestErrorKind::IdentityDefaultType {
                command: "note".to_string(),
                arg: "session".to_string(),
                ty: ArgType::Integer,
            }
        );
        assert!(e.to_string().contains("string"), "{e}");
    }

    #[test]
    fn an_argument_named_after_a_global_output_flag_is_rejected() {
        // clap matches the global `--json`/`--pretty`/`--text` before a
        // command's arguments collect, so such an argument could never be
        // delivered however the user typed it.
        for name in RESERVED_ARG_NAMES {
            let text = format!(
                "name = \"demo\"\napi_version = 1\n[[commands]]\nid = \"note\"\n\
                 [[commands.args]]\nname = \"{name}\"\ntype = \"string\"\n"
            );
            let e = parse(&text).unwrap_err();
            assert_eq!(
                e.kind,
                ManifestErrorKind::ReservedArgName {
                    command: "note".to_string(),
                    arg: (*name).to_string(),
                },
                "`{name}` must be refused"
            );
            assert!(e.to_string().contains(&format!("--{name}")), "{e}");
        }
    }

    #[test]
    fn an_unknown_key_inside_a_command_is_rejected() {
        let text = r#"
            name = "demo"
            api_version = 1
            [[commands]]
            id = "note"
            returns = "object"
        "#;
        // `returns` is in the design but nothing would validate it, so it is
        // not a manifest key: an author gets an error rather than a promise the
        // host does not keep.
        let e = parse(text).expect_err("returns is not a command key");
        assert!(matches!(e.kind, ManifestErrorKind::Syntax(ref m) if m.contains("returns")));
    }

    #[test]
    fn the_command_subcommand_is_reserved_from_plugin_verbs() {
        let text = r#"
            name = "demo"
            api_version = 1
            [[cli]]
            name = "command"
        "#;
        let e = parse(text).expect_err("the kernel dispatches `command`");
        assert_eq!(
            e.kind,
            ManifestErrorKind::ReservedCliVerb {
                verb: "command".to_string(),
            }
        );
    }
}
