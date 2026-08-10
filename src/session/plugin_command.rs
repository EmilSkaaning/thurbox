//! The plugin command registry — pure data and pure logic.
//!
//! A command is the one entry point every surface shares: a keybinding, the
//! palette, `thurbox-cli`, and an agent inside a session all resolve to the same
//! typed thing ([ADR-V10]). This module owns what that thing *is* — its
//! plugin-qualified id, its argument schema, who may invoke it, and how a set of
//! raw strings or a JSON object becomes bound arguments.
//!
//! Two properties are worth stating because everything else follows from them:
//!
//! - **The registry is built from manifests, never from running plugins.** A
//!   plugin that has never started, faulted at `init`, or ships no service half
//!   still contributes its commands, so listing costs a filesystem walk rather
//!   than a VM per plugin. It is also what lets a suspended plugin keep its
//!   entry in the palette instead of vanishing from it.
//! - **Ids cannot collide.** Plugin names are unique (discovery guarantees it),
//!   manifest ids are unique per kind, and no declared identifier may contain a
//!   dot — so a declared command can never spell a kernel-generated
//!   `<pane>.<op>` id and no tiebreak rule is needed.
//!
//! The kernel generates one triple of commands per declared pane
//! (`toggle`/`show`/`hide`) with no plugin code at all. That is not a
//! convenience: pane visibility is kernel state precisely because a suspended
//! plugin cannot show its own pane, so the command that shows it cannot be the
//! plugin's either.
//!
//! [ADR-V10]: ../../docs/v2/ARCHITECTURE.md

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use serde_json::{json, Map, Value};

use crate::session::plugin_manifest::{
    AgentPolicy, ArgType, CommandArgDecl, IdentityDefault, PluginManifest,
};

/// Which visibility operation a generated pane command performs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VisibilityOp {
    /// Flip the pane's current visibility.
    Toggle,
    /// Put the pane on screen.
    Show,
    /// Take the pane off screen.
    Hide,
}

impl VisibilityOp {
    /// The id suffix, which is also the verb a user reads.
    pub fn as_str(self) -> &'static str {
        match self {
            VisibilityOp::Toggle => "toggle",
            VisibilityOp::Show => "show",
            VisibilityOp::Hide => "hide",
        }
    }

    /// Every operation the kernel generates, in the order it generates them.
    pub fn all() -> &'static [VisibilityOp] {
        &[VisibilityOp::Toggle, VisibilityOp::Show, VisibilityOp::Hide]
    }

    /// What this operation makes the visibility, given what it is now.
    pub fn apply(self, current: bool) -> bool {
        match self {
            VisibilityOp::Toggle => !current,
            VisibilityOp::Show => true,
            VisibilityOp::Hide => false,
        }
    }
}

impl fmt::Display for VisibilityOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Who runs a command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Handler {
    /// The plugin's service half, keyed by the manifest-local command id.
    Plugin {
        /// The key the service half's `commands` table must carry.
        entry: String,
    },
    /// The kernel, against the persisted per-pane visibility.
    PaneVisibility {
        /// The pane whose visibility this command changes.
        pane: String,
        /// What it does to it.
        op: VisibilityOp,
        /// The manifest's seed, used as the value to flip when the user has
        /// never stored a choice for this pane.
        default_visible: bool,
    },
}

/// One command as the registry holds it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandSpec {
    /// Fully-qualified: `<plugin>.<declared-id>`, or `<plugin>.<pane>.<op>`.
    pub id: String,
    /// The plugin that owns it.
    pub plugin: String,
    /// Human-readable title.
    pub title: String,
    /// Longer description, when the manifest carried one.
    pub description: Option<String>,
    /// Declared arguments, in declaration order.
    pub args: Vec<CommandArgDecl>,
    /// Who runs it.
    pub handler: Handler,
    /// Whether a caller inside a session may invoke it at all.
    pub agent_callable: bool,
    /// The policy applied to a caller inside a session.
    pub agent_policy: AgentPolicy,
}

/// The identity a caller brings, resolved from the environment thurbox injects
/// into every session it spawns.
///
/// This is deliberately the *only* identity mechanism: `THURBOX_SESSION` and
/// `THURBOX_TASK` already let `message send` stamp provenance with no ids
/// passed, and a second scheme would be a second thing to keep true.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CallerIdentity {
    /// The session the caller runs inside, if any.
    pub session: Option<String>,
    /// The task that spawned the caller's session, if any.
    pub task: Option<String>,
}

impl CallerIdentity {
    /// Whether this caller is running inside a thurbox session.
    ///
    /// The caller gate keys on this, and it is not an authentication claim: an
    /// agent can unset an environment variable. The gate stops an agent
    /// *stumbling* into a user-only command; containment is the capability set.
    pub fn is_inside_session(&self) -> bool {
        self.session.is_some()
    }

    /// The value an identity default would supply, if the caller has it.
    fn value_for(&self, source: IdentityDefault) -> Option<&str> {
        match source {
            IdentityDefault::Session => self.session.as_deref(),
            IdentityDefault::Task => self.task.as_deref(),
        }
    }
}

/// A bound argument value, matching its declared type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArgValue {
    /// A `string` argument.
    String(String),
    /// An `integer` argument.
    Integer(i64),
    /// A `boolean` argument.
    Boolean(bool),
}

impl ArgValue {
    /// This value as JSON.
    pub fn to_json(&self) -> Value {
        match self {
            ArgValue::String(s) => json!(s),
            ArgValue::Integer(i) => json!(i),
            ArgValue::Boolean(b) => json!(b),
        }
    }
}

/// Arguments that passed validation, keyed by declared name.
pub type BoundArgs = BTreeMap<String, ArgValue>;

/// Why an invocation's arguments were refused.
///
/// Every variant names the offending argument, because "bad arguments" without
/// a name is the error message a caller cannot act on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArgError {
    /// An argument the command does not declare.
    Unknown {
        /// The offending name.
        name: String,
    },
    /// A required argument no source supplied.
    Missing {
        /// The missing name.
        name: String,
    },
    /// A value that is not of the declared type.
    BadType {
        /// The argument.
        name: String,
        /// What it declares.
        expected: ArgType,
        /// What arrived.
        got: String,
    },
    /// A flag that needs a value and has none.
    MissingValue {
        /// The argument.
        name: String,
    },
    /// A command-line token that is not a `--flag`.
    NotAFlag {
        /// The offending token.
        token: String,
    },
    /// The JSON argument form was not an object.
    NotAnObject {
        /// What arrived instead.
        got: String,
    },
    /// The JSON argument form did not parse.
    MalformedJson {
        /// The parser's message.
        cause: String,
    },
}

impl fmt::Display for ArgError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ArgError::Unknown { name } => write!(f, "unknown argument: {name}"),
            ArgError::Missing { name } => write!(f, "missing required argument: {name}"),
            ArgError::BadType {
                name,
                expected,
                got,
            } => write!(f, "argument {name} expects {expected}, got {got}"),
            ArgError::MissingValue { name } => write!(f, "argument {name} needs a value"),
            ArgError::NotAFlag { token } => {
                write!(f, "expected `--<argument>`, got `{token}`")
            }
            ArgError::NotAnObject { got } => {
                write!(f, "arguments must be a JSON object, got {got}")
            }
            ArgError::MalformedJson { cause } => {
                write!(f, "arguments are not valid JSON: {cause}")
            }
        }
    }
}

impl std::error::Error for ArgError {}

impl CommandSpec {
    /// Look up a declared argument.
    fn arg(&self, name: &str) -> Option<&CommandArgDecl> {
        self.args.iter().find(|a| a.name == name)
    }

    /// The JSON Schema for this command's arguments.
    ///
    /// JSON Schema rather than a bespoke shape because it is what agent CLIs
    /// already consume as tool definitions: a command list is then a toolset
    /// with no translation layer, which is the point of making commands
    /// agent-callable at all.
    pub fn args_schema(&self) -> Value {
        let mut properties = Map::new();
        let mut required: Vec<Value> = Vec::new();
        for arg in &self.args {
            let mut property = json!({ "type": arg.ty.as_str() });
            if let Some(description) = &arg.description {
                property["description"] = json!(description);
            }
            properties.insert(arg.name.clone(), property);
            if arg.required {
                required.push(json!(arg.name));
            }
        }
        json!({
            "type": "object",
            "properties": Value::Object(properties),
            "required": required,
        })
    }

    /// Whether *any* caller inside a session could invoke this command.
    ///
    /// The listing's answer to "will this call reach the command", which the
    /// design requires a command list to report so an agent can tell in advance.
    /// [`Self::denial`] is the per-invocation gate; this is the same rule with
    /// the caller quantified away.
    pub fn agent_reachable(&self) -> bool {
        self.agent_callable && self.agent_policy == AgentPolicy::Allow
    }

    /// Why this caller may not invoke the command, if it may not.
    pub fn denial(&self, identity: &CallerIdentity) -> Option<String> {
        if !identity.is_inside_session() {
            return None;
        }
        if !self.agent_callable {
            return Some(format!(
                "`{}` is not callable from inside a session",
                self.id
            ));
        }
        match self.agent_policy {
            AgentPolicy::Allow => None,
            AgentPolicy::Deny => Some(format!(
                "`{}` is user-only: its policy denies callers inside a session",
                self.id
            )),
        }
    }

    /// Turn command-line tokens into raw name/value pairs.
    ///
    /// The bare-flag rule needs the declaration: `--flag` with no value is
    /// `true` for a `boolean` argument and a missing value for anything else.
    /// Deciding it in clap or in the CLI layer would mean `--body --other x`
    /// silently binding `body = "true"`.
    pub fn parse_flags(&self, tokens: &[String]) -> Result<BTreeMap<String, String>, ArgError> {
        let mut raw: BTreeMap<String, String> = BTreeMap::new();
        let mut i = 0;
        while i < tokens.len() {
            let token = &tokens[i];
            let Some(body) = token.strip_prefix("--") else {
                return Err(ArgError::NotAFlag {
                    token: token.clone(),
                });
            };
            let (name, inline) = match body.split_once('=') {
                Some((n, v)) => (n.to_string(), Some(v.to_string())),
                None => (body.to_string(), None),
            };
            // Resolved before the value, so a typo is "unknown argument" rather
            // than a confusing complaint about a missing value.
            let declared = self
                .arg(&name)
                .ok_or(ArgError::Unknown { name: name.clone() })?;

            let value = match inline {
                Some(v) => {
                    i += 1;
                    v
                }
                None => match tokens.get(i + 1) {
                    Some(next) if !next.starts_with("--") => {
                        i += 2;
                        next.clone()
                    }
                    _ if declared.ty == ArgType::Boolean => {
                        i += 1;
                        "true".to_string()
                    }
                    _ => {
                        return Err(ArgError::MissingValue { name });
                    }
                },
            };
            raw.insert(name, value);
        }
        Ok(raw)
    }

    /// Bind raw string values, coercing each to its declared type.
    pub fn bind_flags(
        &self,
        raw: &BTreeMap<String, String>,
        identity: &CallerIdentity,
    ) -> Result<BoundArgs, ArgError> {
        let mut bound = BoundArgs::new();
        for (name, text) in raw {
            let declared = self
                .arg(name)
                .ok_or(ArgError::Unknown { name: name.clone() })?;
            bound.insert(name.clone(), coerce_text(name, declared.ty, text)?);
        }
        self.finish(bound, identity)
    }

    /// Bind a JSON object of arguments.
    pub fn bind_json(
        &self,
        value: &Value,
        identity: &CallerIdentity,
    ) -> Result<BoundArgs, ArgError> {
        let object = value.as_object().ok_or_else(|| ArgError::NotAnObject {
            got: json_type_name(value).to_string(),
        })?;

        let mut bound = BoundArgs::new();
        for (name, raw) in object {
            let declared = self
                .arg(name)
                .ok_or(ArgError::Unknown { name: name.clone() })?;
            bound.insert(name.clone(), coerce_json(name, declared.ty, raw)?);
        }
        self.finish(bound, identity)
    }

    /// Apply identity defaults, then check the required set.
    ///
    /// The order is the contract: an explicitly supplied value always wins over
    /// what the environment would have filled, and a required argument whose
    /// only source is an identity default fails cleanly when the caller is
    /// outside a session rather than binding an empty string.
    fn finish(
        &self,
        mut bound: BoundArgs,
        identity: &CallerIdentity,
    ) -> Result<BoundArgs, ArgError> {
        for arg in &self.args {
            if bound.contains_key(&arg.name) {
                continue;
            }
            if let Some(source) = arg.default_from {
                if let Some(value) = identity.value_for(source) {
                    bound.insert(arg.name.clone(), ArgValue::String(value.to_string()));
                    continue;
                }
            }
            if arg.required {
                return Err(ArgError::Missing {
                    name: arg.name.clone(),
                });
            }
        }
        Ok(bound)
    }
}

/// Coerce a command-line string to a declared type.
fn coerce_text(name: &str, ty: ArgType, text: &str) -> Result<ArgValue, ArgError> {
    let bad = || ArgError::BadType {
        name: name.to_string(),
        expected: ty,
        got: format!("`{text}`"),
    };
    match ty {
        ArgType::String => Ok(ArgValue::String(text.to_string())),
        ArgType::Integer => text
            .parse::<i64>()
            .map(ArgValue::Integer)
            .map_err(|_| bad()),
        ArgType::Boolean => match text {
            "true" => Ok(ArgValue::Boolean(true)),
            "false" => Ok(ArgValue::Boolean(false)),
            _ => Err(bad()),
        },
    }
}

/// Coerce a JSON value to a declared type.
///
/// Strict: a string `"7"` is not an integer here, because the JSON form exists
/// for callers that already have types and silently accepting a mismatch would
/// make the schema advisory.
fn coerce_json(name: &str, ty: ArgType, value: &Value) -> Result<ArgValue, ArgError> {
    let bad = || ArgError::BadType {
        name: name.to_string(),
        expected: ty,
        got: json_type_name(value).to_string(),
    };
    match ty {
        ArgType::String => value
            .as_str()
            .map(|s| ArgValue::String(s.to_string()))
            .ok_or_else(bad),
        ArgType::Integer => value.as_i64().map(ArgValue::Integer).ok_or_else(bad),
        ArgType::Boolean => value.as_bool().map(ArgValue::Boolean).ok_or_else(bad),
    }
}

/// A JSON value's type, for an error message.
fn json_type_name(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

/// Every command a set of manifests contributes.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CommandRegistry {
    commands: BTreeMap<String, CommandSpec>,
}

impl CommandRegistry {
    /// Build the registry from manifests.
    ///
    /// No VM is created and no plugin code runs, which is what makes the
    /// registry answerable for a plugin that never started.
    pub fn from_manifests<'a>(manifests: impl IntoIterator<Item = &'a PluginManifest>) -> Self {
        let mut commands: BTreeMap<String, CommandSpec> = BTreeMap::new();
        for manifest in manifests {
            for decl in &manifest.commands {
                let id = format!("{}.{}", manifest.name, decl.id);
                commands.entry(id.clone()).or_insert_with(|| CommandSpec {
                    id,
                    plugin: manifest.name.clone(),
                    title: decl.display_title().to_string(),
                    description: decl.description.clone(),
                    args: decl.args.clone(),
                    handler: Handler::Plugin {
                        entry: decl.id.clone(),
                    },
                    agent_callable: decl.agent_callable,
                    agent_policy: decl.agent_policy,
                });
            }

            for pane in &manifest.panes {
                let pane_title = PluginManifest::pane_title(pane);
                for op in VisibilityOp::all().iter().copied() {
                    let id = format!("{}.{}.{}", manifest.name, pane.id, op);
                    commands.entry(id.clone()).or_insert_with(|| CommandSpec {
                        id,
                        plugin: manifest.name.clone(),
                        title: format!("{} the {pane_title} pane", verb_title(op)),
                        description: None,
                        // Visibility takes no arguments: the pane is named by
                        // the command id, and there is nothing else to say.
                        args: Vec::new(),
                        handler: Handler::PaneVisibility {
                            pane: pane.id.clone(),
                            op,
                            default_visible: pane.default_visible,
                        },
                        // Non-destructive and reversible, so there is nothing
                        // for a conservative default to protect.
                        agent_callable: true,
                        agent_policy: AgentPolicy::Allow,
                    });
                }
            }
        }
        Self { commands }
    }

    /// Look up one command by its fully-qualified id.
    pub fn get(&self, id: &str) -> Option<&CommandSpec> {
        self.commands.get(id)
    }

    /// Every command, in id order.
    pub fn all(&self) -> impl Iterator<Item = &CommandSpec> {
        self.commands.values()
    }

    /// One plugin's commands, in id order.
    pub fn for_plugin(&self, plugin: &str) -> Vec<&CommandSpec> {
        self.commands
            .values()
            .filter(|c| c.plugin == plugin)
            .collect()
    }

    /// Every plugin that contributed a command.
    pub fn plugins(&self) -> BTreeSet<&str> {
        self.commands.values().map(|c| c.plugin.as_str()).collect()
    }

    /// How many commands are registered.
    pub fn len(&self) -> usize {
        self.commands.len()
    }

    /// Whether nothing is registered.
    pub fn is_empty(&self) -> bool {
        self.commands.is_empty()
    }
}

/// The verb a generated command's title starts with.
fn verb_title(op: VisibilityOp) -> &'static str {
    match op {
        VisibilityOp::Toggle => "Toggle",
        VisibilityOp::Show => "Show",
        VisibilityOp::Hide => "Hide",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn manifest(body: &str) -> PluginManifest {
        PluginManifest::from_toml(Path::new("/p/plugin.toml"), body).expect("valid manifest")
    }

    fn registry(body: &str) -> CommandRegistry {
        CommandRegistry::from_manifests(std::iter::once(&manifest(body)))
    }

    const NOTE: &str = r#"
        name = "notes"
        api_version = 1
        [[commands]]
        id = "note"
        title = "Attach a note"
        [[commands.args]]
        name = "body"
        type = "string"
        required = true
        description = "the note body"
        [[commands.args]]
        name = "pinned"
        type = "boolean"
        [[commands.args]]
        name = "weight"
        type = "integer"
        [[commands.args]]
        name = "session"
        type = "string"
        default_from = "session"
    "#;

    fn note_spec() -> CommandSpec {
        registry(NOTE)
            .get("notes.note")
            .expect("registered")
            .clone()
    }

    fn inside(session: &str) -> CallerIdentity {
        CallerIdentity {
            session: Some(session.to_string()),
            task: None,
        }
    }

    #[test]
    fn a_command_is_namespaced_by_its_plugin() {
        let r = registry(NOTE);
        assert_eq!(r.len(), 1);
        let spec = r.get("notes.note").expect("plugin-qualified id");
        assert_eq!(spec.plugin, "notes");
        assert_eq!(spec.title, "Attach a note");
        assert_eq!(
            spec.handler,
            Handler::Plugin {
                entry: "note".to_string()
            }
        );
    }

    #[test]
    fn two_plugins_may_declare_the_same_local_id() {
        let one = manifest("name = \"a\"\napi_version = 1\n[[commands]]\nid = \"go\"\n");
        let two = manifest("name = \"b\"\napi_version = 1\n[[commands]]\nid = \"go\"\n");
        let r = CommandRegistry::from_manifests([&one, &two]);
        assert_eq!(r.len(), 2);
        assert!(r.get("a.go").is_some());
        assert!(r.get("b.go").is_some());
        assert_eq!(r.plugins(), ["a", "b"].into_iter().collect());
    }

    #[test]
    fn a_declared_id_cannot_collide_with_a_generated_one() {
        // The alphabet is the guarantee: a declared id may not contain a dot,
        // so `board` and `board.toggle` are different namespaces by
        // construction rather than by a tiebreak rule.
        let r = registry(
            "name = \"p\"\napi_version = 1\ncapabilities = [\"render\"]\n\
             [[commands]]\nid = \"board\"\n[[panes]]\nid = \"board\"\n",
        );
        let ids: Vec<&str> = r.all().map(|c| c.id.as_str()).collect();
        assert_eq!(
            ids,
            vec!["p.board", "p.board.hide", "p.board.show", "p.board.toggle"]
        );
    }

    #[test]
    fn every_pane_gets_three_kernel_handled_commands() {
        let r = registry(
            "name = \"p\"\napi_version = 1\ncapabilities = [\"render\"]\n\
             [[panes]]\nid = \"board\"\ntitle = \"Board\"\n",
        );
        assert_eq!(r.len(), 3);
        for op in VisibilityOp::all() {
            let spec = r
                .get(&format!("p.board.{op}"))
                .unwrap_or_else(|| panic!("p.board.{op} must exist"));
            assert_eq!(
                spec.handler,
                Handler::PaneVisibility {
                    pane: "board".to_string(),
                    op: *op,
                    default_visible: true,
                }
            );
            assert!(spec.args.is_empty(), "visibility takes no arguments");
            assert!(spec.title.contains("Board"), "{}", spec.title);
        }
    }

    #[test]
    fn two_panes_are_addressable_independently() {
        let r = registry(
            "name = \"p\"\napi_version = 1\ncapabilities = [\"render\"]\n\
             [[panes]]\nid = \"one\"\n[[panes]]\nid = \"two\"\ndefault_visible = false\n",
        );
        assert_eq!(r.len(), 6);
        let one = r.get("p.one.show").expect("first pane");
        let two = r.get("p.two.show").expect("second pane");
        assert_eq!(
            one.handler,
            Handler::PaneVisibility {
                pane: "one".to_string(),
                op: VisibilityOp::Show,
                default_visible: true,
            }
        );
        assert_eq!(
            two.handler,
            Handler::PaneVisibility {
                pane: "two".to_string(),
                op: VisibilityOp::Show,
                default_visible: false,
            }
        );
    }

    #[test]
    fn a_plugin_declaring_nothing_contributes_nothing() {
        let r = registry("name = \"quiet\"\napi_version = 1\n");
        assert!(r.is_empty());
        assert!(r.for_plugin("quiet").is_empty());
    }

    #[test]
    fn visibility_ops_compose_as_expected() {
        assert!(VisibilityOp::Toggle.apply(false));
        assert!(!VisibilityOp::Toggle.apply(true));
        assert!(VisibilityOp::Show.apply(false));
        assert!(!VisibilityOp::Hide.apply(true));
    }

    #[test]
    fn the_schema_describes_properties_and_the_required_set() {
        let schema = note_spec().args_schema();
        assert_eq!(schema["type"], "object");
        assert_eq!(schema["properties"]["body"]["type"], "string");
        assert_eq!(schema["properties"]["body"]["description"], "the note body");
        assert_eq!(schema["properties"]["pinned"]["type"], "boolean");
        assert_eq!(schema["properties"]["weight"]["type"], "integer");
        assert_eq!(schema["required"], json!(["body"]));
    }

    #[test]
    fn the_schema_of_an_argument_less_command_is_an_empty_object() {
        let r = registry(
            "name = \"p\"\napi_version = 1\ncapabilities = [\"render\"]\n[[panes]]\nid = \"b\"\n",
        );
        let schema = r.get("p.b.toggle").expect("generated").args_schema();
        assert_eq!(
            schema,
            json!({ "type": "object", "properties": {}, "required": [] })
        );
    }

    #[test]
    fn well_typed_flags_bind() {
        let spec = note_spec();
        let raw = spec
            .parse_flags(&[
                "--body".to_string(),
                "hello".to_string(),
                "--weight=3".to_string(),
                "--pinned".to_string(),
            ])
            .expect("parses");
        let bound = spec
            .bind_flags(&raw, &CallerIdentity::default())
            .expect("binds");
        assert_eq!(bound["body"], ArgValue::String("hello".to_string()));
        assert_eq!(bound["weight"], ArgValue::Integer(3));
        assert_eq!(bound["pinned"], ArgValue::Boolean(true));
        // Optional and unsupplied: absent rather than defaulted to something.
        assert!(!bound.contains_key("session"));
    }

    #[test]
    fn a_bare_flag_is_true_only_for_a_boolean() {
        let spec = note_spec();
        assert_eq!(
            spec.parse_flags(&["--pinned".to_string()])
                .expect("boolean"),
            BTreeMap::from([("pinned".to_string(), "true".to_string())])
        );
        assert_eq!(
            spec.parse_flags(&["--body".to_string()])
                .expect_err("needs a value"),
            ArgError::MissingValue {
                name: "body".to_string()
            }
        );
        // The failure mode this rule exists for: without it `body` would bind
        // to "true" and `weight` would go missing.
        assert_eq!(
            spec.parse_flags(&[
                "--body".to_string(),
                "--weight".to_string(),
                "1".to_string()
            ])
            .expect_err("a following flag is not a value"),
            ArgError::MissingValue {
                name: "body".to_string()
            }
        );
    }

    #[test]
    fn a_negative_number_is_a_value_not_a_flag() {
        let spec = note_spec();
        let raw = spec
            .parse_flags(&["--weight".to_string(), "-4".to_string()])
            .expect("parses");
        assert_eq!(raw["weight"], "-4");
    }

    #[test]
    fn a_positional_token_is_refused() {
        let spec = note_spec();
        assert_eq!(
            spec.parse_flags(&["body".to_string()])
                .expect_err("not a flag"),
            ArgError::NotAFlag {
                token: "body".to_string()
            }
        );
    }

    #[test]
    fn an_undeclared_flag_is_named() {
        let spec = note_spec();
        assert_eq!(
            spec.parse_flags(&["--titel".to_string(), "x".to_string()])
                .expect_err("typo"),
            ArgError::Unknown {
                name: "titel".to_string()
            }
        );
    }

    #[test]
    fn a_missing_required_argument_is_named() {
        let spec = note_spec();
        let err = spec
            .bind_flags(&BTreeMap::new(), &CallerIdentity::default())
            .expect_err("body is required");
        assert_eq!(
            err,
            ArgError::Missing {
                name: "body".to_string()
            }
        );
        assert!(err.to_string().contains("body"));
    }

    #[test]
    fn a_value_of_the_wrong_type_is_refused() {
        let spec = note_spec();
        let raw = BTreeMap::from([
            ("body".to_string(), "x".to_string()),
            ("weight".to_string(), "heavy".to_string()),
        ]);
        match spec.bind_flags(&raw, &CallerIdentity::default()) {
            Err(ArgError::BadType {
                name,
                expected,
                got,
            }) => {
                assert_eq!(name, "weight");
                assert_eq!(expected, ArgType::Integer);
                assert!(got.contains("heavy"), "{got}");
            }
            other => panic!("expected a type error, got {other:?}"),
        }
    }

    #[test]
    fn a_boolean_flag_only_accepts_the_two_words() {
        let spec = note_spec();
        let raw = BTreeMap::from([
            ("body".to_string(), "x".to_string()),
            ("pinned".to_string(), "yes".to_string()),
        ]);
        assert!(matches!(
            spec.bind_flags(&raw, &CallerIdentity::default()),
            Err(ArgError::BadType { .. })
        ));
    }

    #[test]
    fn json_and_flags_bind_to_the_same_thing() {
        let spec = note_spec();
        let raw = spec
            .parse_flags(&[
                "--body".to_string(),
                "hi".to_string(),
                "--weight".to_string(),
                "2".to_string(),
                "--pinned".to_string(),
                "false".to_string(),
            ])
            .expect("parses");
        let from_flags = spec
            .bind_flags(&raw, &CallerIdentity::default())
            .expect("binds");
        let from_json = spec
            .bind_json(
                &json!({ "body": "hi", "weight": 2, "pinned": false }),
                &CallerIdentity::default(),
            )
            .expect("binds");
        assert_eq!(from_flags, from_json);
    }

    #[test]
    fn json_arguments_must_be_an_object() {
        let spec = note_spec();
        assert_eq!(
            spec.bind_json(&json!(["body"]), &CallerIdentity::default())
                .expect_err("not an object"),
            ArgError::NotAnObject {
                got: "array".to_string()
            }
        );
    }

    #[test]
    fn json_types_are_not_coerced() {
        let spec = note_spec();
        // A schema that accepted "7" for an integer would be advisory rather
        // than a contract, and the JSON form exists for callers that have types.
        match spec.bind_json(
            &json!({ "body": "x", "weight": "7" }),
            &CallerIdentity::default(),
        ) {
            Err(ArgError::BadType { name, got, .. }) => {
                assert_eq!(name, "weight");
                assert_eq!(got, "string");
            }
            other => panic!("expected a type error, got {other:?}"),
        }
    }

    #[test]
    fn an_undeclared_json_key_is_named() {
        let spec = note_spec();
        assert_eq!(
            spec.bind_json(
                &json!({ "body": "x", "extra": 1 }),
                &CallerIdentity::default()
            )
            .expect_err("unknown key"),
            ArgError::Unknown {
                name: "extra".to_string()
            }
        );
    }

    #[test]
    fn an_identity_default_is_filled_from_the_caller() {
        let spec = note_spec();
        let raw = BTreeMap::from([("body".to_string(), "x".to_string())]);
        let bound = spec.bind_flags(&raw, &inside("sess-1")).expect("binds");
        assert_eq!(bound["session"], ArgValue::String("sess-1".to_string()));
    }

    #[test]
    fn an_explicit_value_beats_an_identity_default() {
        let spec = note_spec();
        let raw = BTreeMap::from([
            ("body".to_string(), "x".to_string()),
            ("session".to_string(), "other".to_string()),
        ]);
        let bound = spec.bind_flags(&raw, &inside("sess-1")).expect("binds");
        assert_eq!(bound["session"], ArgValue::String("other".to_string()));
    }

    #[test]
    fn a_required_identity_default_with_no_identity_fails_cleanly() {
        // Binding an empty string would hand the command a session id that
        // exists nowhere, which is worse than refusing the call.
        let spec = registry(
            "name = \"p\"\napi_version = 1\n[[commands]]\nid = \"c\"\n\
             [[commands.args]]\nname = \"session\"\ntype = \"string\"\n\
             required = true\ndefault_from = \"session\"\n",
        )
        .get("p.c")
        .expect("registered")
        .clone();

        assert_eq!(
            spec.bind_flags(&BTreeMap::new(), &CallerIdentity::default())
                .expect_err("no identity"),
            ArgError::Missing {
                name: "session".to_string()
            }
        );
        assert!(spec.bind_flags(&BTreeMap::new(), &inside("s")).is_ok());
    }

    #[test]
    fn a_task_identity_default_is_filled_separately() {
        let spec = registry(
            "name = \"p\"\napi_version = 1\n[[commands]]\nid = \"c\"\n\
             [[commands.args]]\nname = \"task\"\ntype = \"string\"\ndefault_from = \"task\"\n",
        )
        .get("p.c")
        .expect("registered")
        .clone();

        let identity = CallerIdentity {
            session: Some("s".to_string()),
            task: Some("t-9".to_string()),
        };
        let bound = spec.bind_flags(&BTreeMap::new(), &identity).expect("binds");
        assert_eq!(bound["task"], ArgValue::String("t-9".to_string()));
        // A session without a task leaves an optional task argument unbound.
        let bound = spec
            .bind_flags(&BTreeMap::new(), &inside("s"))
            .expect("binds");
        assert!(!bound.contains_key("task"));
    }

    #[test]
    fn a_caller_outside_a_session_is_never_denied() {
        let spec = registry(
            "name = \"p\"\napi_version = 1\n[[commands]]\nid = \"c\"\nagent_policy = \"deny\"\n",
        )
        .get("p.c")
        .expect("registered")
        .clone();
        assert_eq!(spec.denial(&CallerIdentity::default()), None);
    }

    #[test]
    fn a_denying_policy_refuses_a_caller_inside_a_session() {
        let spec = registry(
            "name = \"p\"\napi_version = 1\n[[commands]]\nid = \"c\"\nagent_policy = \"deny\"\n",
        )
        .get("p.c")
        .expect("registered")
        .clone();
        let denial = spec.denial(&inside("s")).expect("denied");
        assert!(denial.contains("p.c"), "{denial}");
    }

    #[test]
    fn a_command_that_is_not_agent_callable_refuses_a_session_caller() {
        let spec = registry(
            "name = \"p\"\napi_version = 1\n[[commands]]\nid = \"c\"\nagent_callable = false\n",
        )
        .get("p.c")
        .expect("registered")
        .clone();
        assert!(spec.denial(&inside("s")).is_some());
        assert_eq!(spec.denial(&CallerIdentity::default()), None);
    }

    #[test]
    fn an_allowing_policy_admits_a_session_caller() {
        assert_eq!(note_spec().denial(&inside("s")), None);
    }

    #[test]
    fn reachability_is_the_denial_rule_with_the_caller_quantified_away() {
        assert!(note_spec().agent_reachable());
        for extra in ["agent_policy = \"deny\"", "agent_callable = false"] {
            let spec = registry(&format!(
                "name = \"p\"\napi_version = 1\n[[commands]]\nid = \"c\"\n{extra}\n"
            ))
            .get("p.c")
            .expect("registered")
            .clone();
            assert!(!spec.agent_reachable(), "{extra}");
            assert!(spec.denial(&inside("s")).is_some(), "{extra}");
        }
    }

    #[test]
    fn arg_values_render_as_json() {
        assert_eq!(ArgValue::String("a".into()).to_json(), json!("a"));
        assert_eq!(ArgValue::Integer(-2).to_json(), json!(-2));
        assert_eq!(ArgValue::Boolean(true).to_json(), json!(true));
    }
}
