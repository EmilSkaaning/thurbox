//! `thurbox-cli command` — the agent-facing surface of the plugin command
//! registry.
//!
//! Three verbs, and the split between them is the point. `list` and `describe`
//! read the registry, which is built from manifests, so discovery starts no
//! plugin and answers for one that has never run. `run` is the only verb that
//! executes anything, and what it executes depends on the command's handler: a
//! generated pane-visibility command is the kernel's to perform, and everything
//! else goes to the owning plugin's **service** half — the same rule a
//! plugin-owned CLI verb follows, because a command an agent should be able to
//! drive has to work with the TUI closed.
//!
//! Identity is not re-invented here. Thurbox already injects `THURBOX_SESSION`
//! and `THURBOX_TASK` into every session it spawns, which is what lets
//! `message send` stamp provenance with no ids passed; the same two variables
//! fill an argument that declares an identity default, and their presence is
//! what makes a caller "inside a session" for the policy gate.

use clap::Subcommand;
use serde_json::{json, Value};

use super::output::{table, CommandOutput};
use crate::session::plugin_command::{
    ArgError, BoundArgs, CallerIdentity, CommandRegistry, CommandSpec, Handler, VisibilityOp,
};
use crate::session::plugin_manifest::RESERVED_ARG_NAMES;

/// Arguments failed validation.
const E_ARGS: &str = "E_ARGS";
/// No such command in the registry.
const E_UNKNOWN_COMMAND: &str = "E_UNKNOWN_COMMAND";
/// The owning plugin cannot host the command.
const E_PLUGIN_UNAVAILABLE: &str = "E_PLUGIN_UNAVAILABLE";
/// The caller is not permitted to invoke this command.
const E_DENIED: &str = "E_DENIED";

/// `command` subcommands.
#[derive(Subcommand, Debug)]
pub enum Action {
    /// List every command plugins contribute.
    List {
        /// Only this plugin's commands.
        #[arg(long)]
        plugin: Option<String>,
    },
    /// Print one command's arguments, schema, and caller policy.
    Describe {
        /// Fully-qualified command id, e.g. `hello.hello.toggle`.
        id: String,
    },
    /// Invoke a command.
    ///
    /// Arguments are either `--name value` flags or one JSON object:
    ///
    ///   thurbox-cli command run notes.add --body "hi" --pinned
    ///   thurbox-cli command run notes.add '{"body":"hi","pinned":true}'
    ///
    /// The JSON form is positional rather than a flag because `--json` is
    /// already the global output format — which is also why a global output flag
    /// has to precede the command rather than trail its arguments.
    Run {
        /// Fully-qualified command id.
        id: String,
        /// `--name value` flags, or a single JSON object.
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
}

/// Run a `command` invocation.
///
/// Discovery is walked once and used twice: the registry needs the manifests,
/// and dispatching a plugin command needs the directory the registry does not
/// carry. Re-walking for the second would also make the two disagree if a
/// plugin were installed between them.
pub fn run(action: Action) -> Result<CommandOutput, String> {
    let outcome = crate::plugin::discovery::discover();
    let registry = crate::plugin::commands::registry_for(outcome.loadable.iter());
    match action {
        Action::List { plugin } => list(&registry, plugin.as_deref()),
        Action::Describe { id } => Ok(describe(&registry, &id)),
        Action::Run { id, args } => Ok(invoke(&registry, &outcome, &id, &args, &caller_identity())),
    }
}

/// The caller's identity, from the environment thurbox injects at spawn.
///
/// The raw ids, deliberately not resolved against the database: what a session
/// id *means* is the command's business, and a row that has since been deleted
/// is the handler's error to report rather than a different error from the CLI.
fn caller_identity() -> CallerIdentity {
    let var = |key: &str| {
        std::env::var(key)
            .ok()
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty())
    };
    CallerIdentity {
        session: var("THURBOX_SESSION"),
        task: var("THURBOX_TASK"),
    }
}

/// A structured failure: a stable code a caller can branch on, plus a non-zero
/// exit.
fn failure(code: &'static str, id: &str, message: String) -> CommandOutput {
    let json = json!({
        "error": code,
        "message": message,
        "command": id,
    });
    let human = format!("{code}: {message}");
    CommandOutput::failed(json, human, message)
}

/// One command as JSON.
fn command_json(spec: &CommandSpec) -> Value {
    let mut value = json!({
        "id": spec.id,
        "plugin": spec.plugin,
        "title": spec.title,
        "handler": handler_name(&spec.handler),
        "args": spec.args_schema(),
        "agent_callable": spec.agent_callable,
        "agent_policy": spec.agent_policy.as_str(),
    });
    if let Some(description) = &spec.description {
        value["description"] = json!(description);
    }
    value
}

/// Which side runs a command, for output.
fn handler_name(handler: &Handler) -> &'static str {
    match handler {
        Handler::Plugin { .. } => "plugin",
        Handler::PaneVisibility { .. } => "kernel",
    }
}

/// `command list [--plugin <name>]`.
fn list(registry: &CommandRegistry, plugin: Option<&str>) -> Result<CommandOutput, String> {
    let specs: Vec<&CommandSpec> = match plugin {
        Some(name) => {
            // An unknown plugin is an error rather than an empty list: "not
            // installed" and "installed but contributes nothing" are different
            // answers, and conflating them hides a typo.
            if !registry.plugins().contains(&name) {
                return Err(format!("no plugin named `{name}` contributes commands"));
            }
            registry.for_plugin(name)
        }
        None => registry.all().collect(),
    };

    let json = json!({
        "commands": specs.iter().map(|s| command_json(s)).collect::<Vec<_>>(),
    });

    if specs.is_empty() {
        return Ok(CommandOutput::new(json, "No plugin commands."));
    }

    let rows: Vec<Vec<String>> = specs
        .iter()
        .map(|s| {
            vec![
                s.id.clone(),
                s.plugin.clone(),
                s.title.clone(),
                if s.agent_reachable() { "yes" } else { "no" }.to_string(),
            ]
        })
        .collect();
    Ok(CommandOutput::new(
        json,
        table(&["ID", "PLUGIN", "TITLE", "AGENT"], &rows),
    ))
}

/// `command describe <id>`.
fn describe(registry: &CommandRegistry, id: &str) -> CommandOutput {
    let Some(spec) = registry.get(id) else {
        return unknown(registry, id);
    };

    let mut human = format!(
        "{}\n  plugin:          {}\n  title:           {}\n  handled by:      {}\n  \
         agent callable:  {}\n  agent policy:    {}",
        spec.id,
        spec.plugin,
        spec.title,
        handler_name(&spec.handler),
        spec.agent_callable,
        spec.agent_policy,
    );
    if let Some(description) = &spec.description {
        human.push_str(&format!("\n  description:     {description}"));
    }
    if spec.args.is_empty() {
        human.push_str("\n  arguments:       none");
    } else {
        human.push_str("\n  arguments:");
        for arg in &spec.args {
            let mut line = format!("\n    --{} <{}>", arg.name, arg.ty);
            if arg.required {
                line.push_str(" (required)");
            }
            if let Some(source) = arg.default_from {
                line.push_str(&format!(" (defaults to the calling {source})"));
            }
            if let Some(text) = &arg.description {
                line.push_str(&format!(" — {text}"));
            }
            human.push_str(&line);
        }
    }
    CommandOutput::new(command_json(spec), human)
}

/// The unknown-command failure, with a hint at the nearest same-plugin ids.
fn unknown(registry: &CommandRegistry, id: &str) -> CommandOutput {
    let mut message = format!("no command `{id}` is registered");
    // A wrong id is usually a wrong *suffix*, so listing that plugin's commands
    // answers the question the failure raises.
    if let Some((plugin, _)) = id.split_once('.') {
        let siblings: Vec<&str> = registry
            .for_plugin(plugin)
            .iter()
            .map(|s| s.id.as_str())
            .collect();
        if !siblings.is_empty() {
            message.push_str(&format!(" ({} has: {})", plugin, siblings.join(", ")));
        }
    }
    failure(E_UNKNOWN_COMMAND, id, message)
}

/// `command run <id> [args…]`.
fn invoke(
    registry: &CommandRegistry,
    outcome: &crate::plugin::DiscoveryOutcome,
    id: &str,
    args: &[String],
    identity: &CallerIdentity,
) -> CommandOutput {
    let Some(spec) = registry.get(id) else {
        return unknown(registry, id);
    };

    if let Some(reason) = spec.denial(identity) {
        return failure(E_DENIED, id, reason);
    }

    let bound = match bind(spec, args, identity) {
        Ok(bound) => bound,
        Err(e) => return failure(E_ARGS, id, arg_error_message(&e)),
    };

    match &spec.handler {
        Handler::PaneVisibility {
            pane,
            op,
            default_visible,
        } => match set_pane_visibility(&spec.plugin, pane, *op, *default_visible) {
            Ok(value) => success(value),
            Err(e) => failure(E_PLUGIN_UNAVAILABLE, id, e),
        },
        Handler::Plugin { entry } => match run_plugin_command(outcome, &spec.plugin, entry, &bound)
        {
            Ok(value) => success(value),
            Err(e) => failure(E_PLUGIN_UNAVAILABLE, id, e),
        },
    }
}

/// A command's result, rendered for both audiences.
fn success(result: Value) -> CommandOutput {
    let human = match &result {
        Value::Null => "ok".to_string(),
        Value::String(s) => s.clone(),
        other => serde_json::to_string_pretty(other).unwrap_or_else(|_| other.to_string()),
    };
    CommandOutput::new(result, human)
}

/// An argument failure, with a hint for the one mistake the CLI's own shape
/// invites.
///
/// `--json`/`--pretty`/`--text` are global output flags, and clap stops matching
/// them once the trailing argument list starts collecting — so a user who typed
/// one *after* a command argument gets "unknown argument: json", which is true
/// and unhelpful. A command cannot declare an argument by those names
/// ([`RESERVED_ARG_NAMES`]), so seeing one here is always the misplacement.
fn arg_error_message(error: &ArgError) -> String {
    if let ArgError::Unknown { name } = error {
        if RESERVED_ARG_NAMES.contains(&name.as_str()) {
            return format!(
                "`--{name}` is a global output flag: put it before the command, \
                 as `thurbox-cli --{name} command run …`"
            );
        }
    }
    error.to_string()
}

/// Bind either the JSON form or the flag form.
///
/// A single token starting with `{` is the JSON object form; anything else is
/// flags. A flag must start with `--`, so the two forms cannot be confused.
fn bind(
    spec: &CommandSpec,
    args: &[String],
    identity: &CallerIdentity,
) -> Result<BoundArgs, ArgError> {
    if let [single] = args {
        if single.trim_start().starts_with('{') {
            let value: Value =
                serde_json::from_str(single).map_err(|e| ArgError::MalformedJson {
                    cause: e.to_string(),
                })?;
            return spec.bind_json(&value, identity);
        }
    }
    let raw = spec.parse_flags(args)?;
    spec.bind_flags(&raw, identity)
}

/// Perform a generated visibility command against the persisted choice.
///
/// The kernel's own store, not a message to a running TUI: visibility is kernel
/// state (a suspended plugin cannot show its own pane), it must work headlessly,
/// and a live TUI already re-reads it on its external-change poll. Writing only
/// when the stored value differs keeps an idle `show` from bumping every other
/// process's `data_version`.
fn set_pane_visibility(
    plugin: &str,
    pane: &str,
    op: VisibilityOp,
    default_visible: bool,
) -> Result<Value, String> {
    let path = crate::paths::database_file().ok_or("cannot resolve the database path")?;
    let db = crate::storage::Database::open(&path).map_err(|e| e.to_string())?;

    let stored = db
        .get_plugin_pane_visible(plugin, pane)
        .map_err(|e| e.to_string())?;
    // A pane the user has never chosen for flips its manifest seed, which is
    // what makes `toggle` well-defined on a first run.
    let next = op.apply(stored.unwrap_or(default_visible));
    let changed = stored != Some(next);
    if changed {
        db.set_plugin_pane_visible(plugin, pane, next)
            .map_err(|e| e.to_string())?;
    }

    Ok(json!({
        "plugin": plugin,
        "pane": pane,
        "visible": next,
        "changed": changed,
    }))
}

/// Start the owning plugin's service, run the command, and stop it.
fn run_plugin_command(
    outcome: &crate::plugin::DiscoveryOutcome,
    plugin: &str,
    entry: &str,
    args: &BoundArgs,
) -> Result<Value, String> {
    let discovered = outcome
        .loadable
        .iter()
        .find(|p| p.name() == plugin)
        .ok_or_else(|| format!("plugin `{plugin}` is no longer installed"))?;

    if !discovered.manifest.has_service() {
        return Err(format!(
            "plugin `{plugin}` declares the command `{entry}` but has no service half to run it"
        ));
    }

    let path = crate::paths::database_file().ok_or("cannot resolve the database path")?;
    let lock_db = crate::storage::Database::open(&path).map_err(|e| e.to_string())?;
    let lock = crate::storage::plugins::DbServiceLock::new(lock_db, "cli");
    let store: crate::session::plugin_store::PluginStoreFactory = std::sync::Arc::new(|| {
        crate::storage::plugins::DbPluginStore::open()
            .map(|s| Box::new(s) as Box<dyn crate::session::plugin_store::PluginStore>)
    });

    let mut host = crate::plugin::ServiceHost::new(crate::plugin::ExecutionBounds::default());
    host.start(discovered, &lock, Some(store))
        .map_err(|e| e.to_string())?;
    let result = host.run_command(plugin, entry, args);
    host.stop_all(&lock);
    result.map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::Path;

    fn write_plugin(root: &Path, name: &str, manifest_extra: &str) {
        let dir = root.join(name);
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("plugin.toml"),
            format!("name = \"{name}\"\napi_version = 1\n{manifest_extra}"),
        )
        .unwrap();
        fs::write(dir.join("init.luau"), "return {}").unwrap();
    }

    /// Discovery plus its registry over an explicit root, bypassing the real
    /// config directory the way the production entry point cannot.
    fn discovered_over(root: &Path) -> (crate::plugin::DiscoveryOutcome, CommandRegistry) {
        let outcome = crate::plugin::discovery::discover_in(&[], Some(root));
        let registry = crate::plugin::commands::registry_for(outcome.loadable.iter());
        (outcome, registry)
    }

    /// Just the registry, for the verbs that never dispatch.
    fn registry_over(root: &Path) -> CommandRegistry {
        discovered_over(root).1
    }

    const NOTES: &str = "capabilities = [\"render\"]\n\
         [[panes]]\nid = \"board\"\ntitle = \"Board\"\n\
         [[commands]]\nid = \"add\"\ntitle = \"Add a note\"\n\
         description = \"Append to the board\"\n\
         [[commands.args]]\nname = \"body\"\ntype = \"string\"\nrequired = true\n\
         [[commands.args]]\nname = \"pinned\"\ntype = \"boolean\"\n\
         [[commands.args]]\nname = \"session\"\ntype = \"string\"\ndefault_from = \"session\"\n";

    fn notes_registry() -> (tempfile::TempDir, CommandRegistry) {
        let tmp = tempfile::tempdir().unwrap();
        write_plugin(tmp.path(), "notes", NOTES);
        let registry = registry_over(tmp.path());
        (tmp, registry)
    }

    /// Everything `invoke` needs over a root holding one `notes` plugin.
    fn notes_discovered() -> (
        tempfile::TempDir,
        crate::plugin::DiscoveryOutcome,
        CommandRegistry,
    ) {
        let tmp = tempfile::tempdir().unwrap();
        write_plugin(tmp.path(), "notes", NOTES);
        let (outcome, registry) = discovered_over(tmp.path());
        (tmp, outcome, registry)
    }

    fn inside(session: &str) -> CallerIdentity {
        CallerIdentity {
            session: Some(session.to_string()),
            task: None,
        }
    }

    #[test]
    fn list_reports_declared_and_generated_commands() {
        let (_tmp, registry) = notes_registry();
        let out = list(&registry, None).expect("lists");
        let ids: Vec<&str> = out.json["commands"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| c["id"].as_str().unwrap())
            .collect();
        assert_eq!(
            ids,
            vec![
                "notes.add",
                "notes.board.hide",
                "notes.board.show",
                "notes.board.toggle"
            ]
        );
        assert!(out.human.contains("Add a note"), "{}", out.human);
    }

    #[test]
    fn list_reports_the_handler_and_the_schema() {
        let (_tmp, registry) = notes_registry();
        let out = list(&registry, None).expect("lists");
        let commands = out.json["commands"].as_array().unwrap();
        let add = commands.iter().find(|c| c["id"] == "notes.add").unwrap();
        assert_eq!(add["handler"], "plugin");
        assert_eq!(add["args"]["properties"]["body"]["type"], "string");
        assert_eq!(add["args"]["required"], json!(["body"]));
        let toggle = commands
            .iter()
            .find(|c| c["id"] == "notes.board.toggle")
            .unwrap();
        assert_eq!(toggle["handler"], "kernel");
    }

    #[test]
    fn list_with_no_plugins_succeeds() {
        let tmp = tempfile::tempdir().unwrap();
        let out = list(&registry_over(tmp.path()), None).expect("lists");
        assert!(out.json["commands"].as_array().unwrap().is_empty());
        assert!(out.failure.is_none());
        assert!(out.human.contains("No plugin commands"));
    }

    #[test]
    fn list_scoped_to_one_plugin() {
        let tmp = tempfile::tempdir().unwrap();
        write_plugin(tmp.path(), "notes", NOTES);
        write_plugin(tmp.path(), "other", "[[commands]]\nid = \"go\"\n");
        let registry = registry_over(tmp.path());

        let out = list(&registry, Some("other")).expect("lists");
        let ids: Vec<&str> = out.json["commands"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| c["id"].as_str().unwrap())
            .collect();
        assert_eq!(ids, vec!["other.go"]);
    }

    #[test]
    fn list_scoped_to_an_unknown_plugin_is_an_error() {
        let (_tmp, registry) = notes_registry();
        let err = list(&registry, Some("nope")).expect_err("unknown plugin");
        assert!(err.contains("nope"), "{err}");
    }

    #[test]
    fn describe_reports_the_schema_and_the_policy() {
        let (_tmp, registry) = notes_registry();
        let out = describe(&registry, "notes.add");
        assert!(out.failure.is_none());
        assert_eq!(out.json["args"]["properties"]["pinned"]["type"], "boolean");
        assert_eq!(out.json["agent_policy"], "allow");
        assert_eq!(out.json["description"], "Append to the board");
        assert!(
            out.human.contains("--body <string> (required)"),
            "{}",
            out.human
        );
        assert!(
            out.human.contains("defaults to the calling session"),
            "{}",
            out.human
        );
    }

    #[test]
    fn describe_says_when_a_command_takes_no_arguments() {
        let (_tmp, registry) = notes_registry();
        let out = describe(&registry, "notes.board.toggle");
        assert!(out.human.contains("arguments:       none"), "{}", out.human);
    }

    #[test]
    fn describing_an_unknown_command_carries_the_code_and_a_hint() {
        let (_tmp, registry) = notes_registry();
        let out = describe(&registry, "notes.ad");
        assert_eq!(out.json["error"], E_UNKNOWN_COMMAND);
        assert_eq!(out.json["command"], "notes.ad");
        assert!(
            out.failure.is_some(),
            "an unknown command must exit non-zero"
        );
        let message = out.json["message"].as_str().unwrap();
        assert!(message.contains("notes.add"), "{message}");
    }

    #[test]
    fn invoking_an_unknown_command_carries_the_code() {
        let (_tmp, outcome, registry) = notes_discovered();
        let out = invoke(
            &registry,
            &outcome,
            "nope.nope",
            &[],
            &CallerIdentity::default(),
        );
        assert_eq!(out.json["error"], E_UNKNOWN_COMMAND);
        assert!(out.failure.is_some());
    }

    #[test]
    fn bad_arguments_carry_the_args_code_and_name_the_argument() {
        let (_tmp, outcome, registry) = notes_discovered();
        // Missing the required `body`, and nothing is dispatched: a plugin
        // command with no service half would otherwise fail differently.
        let out = invoke(
            &registry,
            &outcome,
            "notes.add",
            &[],
            &CallerIdentity::default(),
        );
        assert_eq!(out.json["error"], E_ARGS);
        assert!(
            out.json["message"].as_str().unwrap().contains("body"),
            "{:?}",
            out.json["message"]
        );
    }

    #[test]
    fn an_undeclared_flag_is_an_args_failure() {
        let (_tmp, outcome, registry) = notes_discovered();
        let out = invoke(
            &registry,
            &outcome,
            "notes.add",
            &["--nope".to_string(), "x".to_string()],
            &CallerIdentity::default(),
        );
        assert_eq!(out.json["error"], E_ARGS);
        assert!(out.json["message"].as_str().unwrap().contains("nope"));
    }

    #[test]
    fn a_bare_flag_for_a_string_argument_is_an_args_failure() {
        let (_tmp, outcome, registry) = notes_discovered();
        let out = invoke(
            &registry,
            &outcome,
            "notes.add",
            &["--body".to_string()],
            &CallerIdentity::default(),
        );
        assert_eq!(out.json["error"], E_ARGS);
        assert!(out.json["message"].as_str().unwrap().contains("value"));
    }

    #[test]
    fn malformed_json_arguments_are_an_args_failure() {
        let (_tmp, outcome, registry) = notes_discovered();
        let out = invoke(
            &registry,
            &outcome,
            "notes.add",
            &["{\"body\":".to_string()],
            &CallerIdentity::default(),
        );
        assert_eq!(out.json["error"], E_ARGS);
        assert!(
            out.json["message"].as_str().unwrap().contains("JSON"),
            "{:?}",
            out.json["message"]
        );
    }

    #[test]
    fn a_misplaced_output_flag_says_where_it_belongs() {
        // Every other unknown argument is a typo; this one is a position
        // mistake, and "unknown argument: json" would send the user hunting for
        // a declaration that cannot exist.
        let (_tmp, outcome, registry) = notes_discovered();
        let out = invoke(
            &registry,
            &outcome,
            "notes.add",
            &["--body".to_string(), "hi".to_string(), "--json".to_string()],
            &CallerIdentity::default(),
        );
        assert_eq!(out.json["error"], E_ARGS);
        let message = out.json["message"].as_str().unwrap();
        assert!(message.contains("global output flag"), "{message}");
        assert!(message.contains("before the command"), "{message}");
    }

    #[test]
    fn a_denied_command_refuses_a_caller_inside_a_session() {
        let tmp = tempfile::tempdir().unwrap();
        write_plugin(
            tmp.path(),
            "danger",
            "[[commands]]\nid = \"wipe\"\nagent_policy = \"deny\"\n",
        );
        let (outcome, registry) = discovered_over(tmp.path());

        let out = invoke(&registry, &outcome, "danger.wipe", &[], &inside("s-1"));
        assert_eq!(out.json["error"], E_DENIED);
        assert!(out.failure.is_some());
    }

    #[test]
    fn a_command_that_is_not_agent_callable_refuses_a_session_caller() {
        let tmp = tempfile::tempdir().unwrap();
        write_plugin(
            tmp.path(),
            "userland",
            "[[commands]]\nid = \"install\"\nagent_callable = false\n",
        );
        let (outcome, registry) = discovered_over(tmp.path());
        assert_eq!(
            invoke(&registry, &outcome, "userland.install", &[], &inside("s-1")).json["error"],
            E_DENIED
        );
    }

    #[test]
    fn a_plugin_command_without_a_service_half_is_unavailable() {
        // Not a manifest error: the same manifest is correct the instant a
        // `service.luau` appears beside it.
        let (_tmp, outcome, registry) = notes_discovered();
        let out = invoke(
            &registry,
            &outcome,
            "notes.add",
            &["--body".to_string(), "hi".to_string()],
            &CallerIdentity::default(),
        );
        assert_eq!(out.json["error"], E_PLUGIN_UNAVAILABLE);
        let message = out.json["message"].as_str().unwrap();
        assert!(message.contains("service half"), "{message}");
    }

    #[test]
    fn the_flag_and_json_forms_bind_identically() {
        let (_tmp, registry) = notes_registry();
        let spec = registry.get("notes.add").expect("registered");
        let from_flags = bind(
            spec,
            &[
                "--body".to_string(),
                "hi".to_string(),
                "--pinned".to_string(),
            ],
            &CallerIdentity::default(),
        )
        .expect("binds");
        let from_json = bind(
            spec,
            &["{\"body\":\"hi\",\"pinned\":true}".to_string()],
            &CallerIdentity::default(),
        )
        .expect("binds");
        assert_eq!(from_flags, from_json);
    }

    #[test]
    fn an_agents_identity_fills_a_session_argument_with_no_id_passed() {
        let (_tmp, registry) = notes_registry();
        let spec = registry.get("notes.add").expect("registered");
        let bound = bind(
            spec,
            &["--body".to_string(), "hi".to_string()],
            &inside("sess-7"),
        )
        .expect("binds");
        assert_eq!(
            bound["session"],
            crate::session::plugin_command::ArgValue::String("sess-7".to_string())
        );
    }

    /// The stored visibility for one pane, read back through a fresh handle.
    fn stored(plugin: &str, pane: &str) -> Option<bool> {
        let path = crate::paths::database_file().expect("guarded path");
        crate::storage::Database::open(&path)
            .expect("open")
            .get_plugin_pane_visible(plugin, pane)
            .expect("read")
    }

    #[test]
    fn a_hide_command_stores_the_choice_headlessly() {
        let tmp = tempfile::tempdir().unwrap();
        let _guard = crate::paths::TestPathGuard::new(tmp.path());

        let value = set_pane_visibility("demo", "board", VisibilityOp::Hide, true).expect("stores");
        assert_eq!(value["visible"], false);
        assert_eq!(value["changed"], true);
        assert_eq!(stored("demo", "board"), Some(false));

        // Idempotent: a second hide is not a change, so it does not bump every
        // other process's `data_version`.
        let again = set_pane_visibility("demo", "board", VisibilityOp::Hide, true).expect("stores");
        assert_eq!(again["changed"], false);
        assert_eq!(stored("demo", "board"), Some(false));
    }

    #[test]
    fn a_show_command_stores_the_choice() {
        let tmp = tempfile::tempdir().unwrap();
        let _guard = crate::paths::TestPathGuard::new(tmp.path());

        set_pane_visibility("demo", "board", VisibilityOp::Show, false).expect("stores");
        assert_eq!(stored("demo", "board"), Some(true));
    }

    #[test]
    fn toggle_flips_the_manifest_seed_when_nothing_is_stored() {
        let tmp = tempfile::tempdir().unwrap();
        let _guard = crate::paths::TestPathGuard::new(tmp.path());

        // This is what makes `toggle` well-defined on a first run: the seed is
        // the value there is to flip.
        set_pane_visibility("demo", "seeded-on", VisibilityOp::Toggle, true).expect("stores");
        assert_eq!(stored("demo", "seeded-on"), Some(false));

        set_pane_visibility("demo", "seeded-off", VisibilityOp::Toggle, false).expect("stores");
        assert_eq!(stored("demo", "seeded-off"), Some(true));
    }

    #[test]
    fn toggling_twice_returns_to_where_it_began() {
        let tmp = tempfile::tempdir().unwrap();
        let _guard = crate::paths::TestPathGuard::new(tmp.path());

        set_pane_visibility("demo", "board", VisibilityOp::Toggle, true).expect("stores");
        set_pane_visibility("demo", "board", VisibilityOp::Toggle, true).expect("stores");
        assert_eq!(stored("demo", "board"), Some(true));
    }

    #[test]
    fn each_pane_is_stored_independently() {
        let tmp = tempfile::tempdir().unwrap();
        let _guard = crate::paths::TestPathGuard::new(tmp.path());

        set_pane_visibility("demo", "one", VisibilityOp::Hide, true).expect("stores");
        assert_eq!(stored("demo", "one"), Some(false));
        assert_eq!(stored("demo", "two"), None, "a sibling pane is untouched");
    }

    #[test]
    fn a_generated_command_invocation_reports_the_new_state() {
        let tmp = tempfile::tempdir().unwrap();
        let _guard = crate::paths::TestPathGuard::new(tmp.path());
        let plugins = tmp.path().join("plugins");
        write_plugin(&plugins, "notes", NOTES);
        let (outcome, registry) = discovered_over(&plugins);

        let out = invoke(
            &registry,
            &outcome,
            "notes.board.hide",
            &[],
            // An agent may drive it: hiding a pane is reversible, so there is
            // nothing here for a conservative default to protect.
            &inside("s-1"),
        );
        assert!(out.failure.is_none(), "{:?}", out.json);
        assert_eq!(out.json["visible"], false);
        assert_eq!(stored("notes", "board"), Some(false));
    }

    #[test]
    fn a_successful_result_renders_for_both_audiences() {
        let out = success(json!({ "id": "t-1" }));
        assert!(out.failure.is_none());
        assert_eq!(out.json["id"], "t-1");
        assert!(out.human.contains("t-1"));
        assert_eq!(success(Value::Null).human, "ok");
    }
}
