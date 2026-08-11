//! `thurbox-cli plugin` — report what the plugin host found.
//!
//! Read-only. `list` and `status` answer "what is installed and is it working",
//! `doctor` answers "what did discovery refuse and why". Nothing here installs,
//! enables, or removes a plugin.
//!
//! `list` and `status` **start** the plugins they report on. The failures users
//! actually hit — a compile error, an `init` that throws — are invisible until
//! something runs the plugin, so a listing that only discovered would answer a
//! different question than the one being asked. `doctor` reports plugins that
//! never reach a VM by definition, so it discovers only.

use clap::Subcommand;
use serde_json::{json, Value};

use super::output::{table, CommandOutput};

/// `plugin` subcommands.
#[derive(Subcommand, Debug)]
pub enum Action {
    /// List every discovered plugin with its state and capabilities.
    List,
    /// Explain one plugin (or all), including why a failed one did not start.
    Status {
        /// Plugin name; omit to report every plugin.
        name: Option<String>,
    },
    /// Report everything discovery rejected, and why.
    Doctor,
    /// Reload a plugin from its current source (or every plugin).
    Reload {
        /// Plugin name; omit to reload everything.
        name: Option<String>,
    },
}

/// Run a `plugin` command.
///
/// Takes no database — the plugin host's state lives in the filesystem and in
/// the VMs this process starts, not in SQLite.
pub fn run(action: Action) -> Result<CommandOutput, String> {
    let bounds = crate::plugin::ExecutionBounds::default();
    match action {
        Action::List => Ok(render_list(&started_report(bounds))),
        Action::Status { name } => status(&started_report(bounds), name.as_deref()),
        Action::Doctor => {
            // Discovery is walked once and used twice: the registry needs the
            // manifests, which the report does not carry.
            let outcome = crate::plugin::discovery::discover();
            let registry = crate::plugin::spawn::registry_for(outcome.loadable.iter());
            let keys = keybinding_report(
                user_keybindings(),
                crate::plugin::keymap::declarations_for(outcome.loadable.iter()),
            );
            let report = crate::plugin::PluginHost::from_discovery(outcome, bounds).report();
            Ok(render_doctor(&report, &registry, &keys))
        }
        Action::Reload { name } => reload(bounds, name.as_deref()),
    }
}

/// `plugin reload [name]`.
///
/// Reload is only meaningful against a started host — the point is to replace
/// running code — so this starts plugins first, like `list` and `status`.
fn reload(
    bounds: crate::plugin::ExecutionBounds,
    name: Option<&str>,
) -> Result<CommandOutput, String> {
    let mut host = crate::plugin::PluginHost::discover(bounds);
    host.start_all();

    let outcomes: Vec<(String, String)> = match name {
        Some(name) => {
            if host.status(name).is_none() {
                return Err(format!("no plugin named `{name}` was discovered"));
            }
            let state = host
                .reload(name)
                .map_err(|e| format!("cannot reload `{name}`: {e}"))?;
            vec![(name.to_string(), state.to_string())]
        }
        None => host
            .reload_all()
            .into_iter()
            .map(|(n, state)| (n, state.to_string()))
            .collect(),
    };

    let json = json!({
        "reloaded": outcomes
            .iter()
            .map(|(n, s)| json!({ "name": n, "state": s }))
            .collect::<Vec<_>>(),
    });
    let human = if outcomes.is_empty() {
        "No plugins to reload.".to_string()
    } else {
        outcomes
            .iter()
            .map(|(n, s)| format!("{n}: {s}"))
            .collect::<Vec<_>>()
            .join("\n")
    };
    Ok(CommandOutput::new(json, human))
}

/// Discover, start, and report — the full truth, including run-time failures.
fn started_report(bounds: crate::plugin::ExecutionBounds) -> crate::plugin::PluginReport {
    let mut host = crate::plugin::PluginHost::discover(bounds);
    host.start_all();
    host.report()
}

/// One plugin status as JSON.
fn status_json(status: &crate::plugin::PluginStatus) -> Value {
    let caps: Vec<String> = status.capabilities.iter().map(|c| c.to_string()).collect();
    let mut value = json!({
        "name": status.name,
        "source": status.source.to_string(),
        "state": state_name(&status.state),
        "dir": status.dir.display().to_string(),
        "capabilities": caps,
    });

    // Only a failure carries a transition and a cause, and a caller that
    // branches on `state` should not have to also handle null fields.
    if let crate::plugin::PluginState::Failed { transition, cause } = &status.state {
        value["failed_during"] = json!(transition.to_string());
        value["cause"] = json!(cause);
    }
    value
}

/// The bare state word, without a failure's trailing detail.
fn state_name(state: &crate::plugin::PluginState) -> &'static str {
    match state {
        crate::plugin::PluginState::Discovered => "discovered",
        crate::plugin::PluginState::Loaded => "loaded",
        crate::plugin::PluginState::Running => "running",
        crate::plugin::PluginState::Stopped => "stopped",
        crate::plugin::PluginState::Failed { .. } => "failed",
    }
}

/// `plugin list`.
fn render_list(report: &crate::plugin::PluginReport) -> CommandOutput {
    let json = json!({
        "plugins": report.plugins.iter().map(status_json).collect::<Vec<_>>(),
        "problems": report.problems,
    });

    if report.plugins.is_empty() {
        let human = if report.problems.is_empty() {
            "No plugins installed.".to_string()
        } else {
            format!(
                "No plugins loaded. {} rejected — run `thurbox-cli plugin doctor`.",
                report.problems.len()
            )
        };
        return CommandOutput::new(json, human);
    }

    let rows: Vec<Vec<String>> = report
        .plugins
        .iter()
        .map(|p| {
            let caps = if p.capabilities.is_empty() {
                "-".to_string()
            } else {
                p.capabilities
                    .iter()
                    .map(|c| c.to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            };
            vec![
                p.name.clone(),
                state_name(&p.state).to_string(),
                p.source.to_string(),
                caps,
            ]
        })
        .collect();

    let mut human = table(&["NAME", "STATE", "SOURCE", "CAPABILITIES"], &rows);
    if !report.problems.is_empty() {
        human.push_str(&format!(
            "\n\n{} plugin(s) rejected — run `thurbox-cli plugin doctor`.",
            report.problems.len()
        ));
    }
    CommandOutput::new(json, human)
}

/// `plugin status [name]`.
fn status(
    report: &crate::plugin::PluginReport,
    name: Option<&str>,
) -> Result<CommandOutput, String> {
    let Some(name) = name else {
        let json = json!({
            "plugins": report.plugins.iter().map(status_json).collect::<Vec<_>>(),
        });
        let human = if report.plugins.is_empty() {
            "No plugins installed.".to_string()
        } else {
            report
                .plugins
                .iter()
                .map(human_status)
                .collect::<Vec<_>>()
                .join("\n\n")
        };
        return Ok(CommandOutput::new(json, human));
    };

    // An unknown name is an error rather than an empty success: "no such
    // plugin" and "that plugin reported nothing" are different answers, and
    // conflating them hides a typo.
    let found = report
        .plugins
        .iter()
        .find(|p| p.name == name)
        .ok_or_else(|| format!("no plugin named `{name}` was discovered"))?;

    Ok(CommandOutput::new(status_json(found), human_status(found)))
}

/// One plugin rendered for a terminal.
fn human_status(status: &crate::plugin::PluginStatus) -> String {
    let caps = if status.capabilities.is_empty() {
        "none".to_string()
    } else {
        status
            .capabilities
            .iter()
            .map(|c| c.to_string())
            .collect::<Vec<_>>()
            .join(", ")
    };

    let mut out = format!(
        "{}\n  state:        {}\n  source:       {}\n  directory:    {}\n  capabilities: {caps}",
        status.name,
        state_name(&status.state),
        status.source,
        status.dir.display(),
    );

    if let crate::plugin::PluginState::Failed { transition, cause } = &status.state {
        out.push_str(&format!(
            "\n  failed during: {transition}\n  cause:         {cause}"
        ));
    }
    out
}

/// `plugin doctor`.
///
/// `spawn` re-derives every contribution verdict from the manifests rather
/// than reading a log of past spawns. That is what makes "why is my variable
/// not in the session" answerable without spawning one — and it is why the
/// section costs nothing in VM terms, keeping doctor's rule that it starts
/// nothing.
fn render_doctor(
    report: &crate::plugin::PluginReport,
    registry: &crate::session::spawn_contribution::Registry,
    keys: &[KeybindingReport],
) -> CommandOutput {
    let spawn = resolve_for_report(registry);
    let accepted: Vec<String> = spawn.env.keys().cloned().collect();
    let refused: Vec<String> = spawn.rejections.iter().map(|r| r.to_string()).collect();

    let json = json!({
        "problems": report.problems,
        "healthy": report.plugins.len(),
        "spawn": {
            "env": accepted,
            "refused": refused,
        },
        "keybindings": keys
            .iter()
            .map(|k| json!({
                "binding": k.binding,
                "title": k.title,
                "chord": k.chord,
                "unbound_because": k.unbound_because,
            }))
            .collect::<Vec<_>>(),
    });

    let mut human = if report.problems.is_empty() {
        format!(
            "No problems found ({} plugin(s) discovered).",
            report.plugins.len()
        )
    } else {
        let mut s = format!("{} problem(s):\n", report.problems.len());
        for problem in &report.problems {
            s.push_str(&format!("  - {problem}\n"));
        }
        s.trim_end().to_string()
    };

    if !keys.is_empty() {
        human.push_str("\n\nPane keybindings:");
        for key in keys {
            match (&key.chord, &key.unbound_because) {
                (Some(chord), _) => {
                    human.push_str(&format!("\n  {chord:<12} {} ({})", key.title, key.binding))
                }
                (None, Some(reason)) => human.push_str(&format!(
                    "\n  {:<12} {} ({}) — {reason}",
                    "unbound", key.title, key.binding
                )),
                (None, None) => human.push_str(&format!(
                    "\n  {:<12} {} ({}) — declares no chord",
                    "unbound", key.title, key.binding
                )),
            }
        }
    }

    if !registry.is_empty() {
        human.push_str("\n\nSpawn contributions:");
        for key in &accepted {
            human.push_str(&format!("\n  + {key}"));
        }
        for reason in &refused {
            human.push_str(&format!("\n  ! {reason}"));
        }
        if accepted.is_empty() && refused.is_empty() {
            human.push_str("\n  none");
        }
    }

    CommandOutput::new(json, human)
}

/// One declared pane binding and the chord it actually has.
///
/// Re-derived from the manifests plus the user's keybindings file, for the same
/// reason the spawn section is re-derived from the manifests: "why does my key do
/// nothing" has to be answerable with no TUI running and no VM started. A chord a
/// manifest declared can be missing for two reasons a user cannot otherwise tell
/// apart — something overlapping already held it, or the manifest never named one
/// — so both are printed.
struct KeybindingReport {
    binding: String,
    title: String,
    chord: Option<String>,
    unbound_because: Option<String>,
}

/// The store factory a plugin VM builds its own connection from.
///
/// Shared by every `thurbox-cli` path that hosts a plugin, so "each VM gets its
/// own connection, built on its own thread" is stated once.
pub(crate) fn store_factory() -> crate::session::plugin_store::PluginStoreFactory {
    std::sync::Arc::new(|| {
        crate::storage::plugins::DbPluginStore::open()
            .map(|s| Box::new(s) as Box<dyn crate::session::plugin_store::PluginStore>)
    })
}

/// The write factory, beside [`store_factory`] and for the same reason.
///
/// Handed to every hosted plugin: *which* of its bindings exist is decided by the
/// capability check, so a plugin that declared no write capability gets a factory
/// it has no binding to reach.
pub(crate) fn writer_factory() -> crate::session::plugin_mutations::KernelWriterFactory {
    std::sync::Arc::new(|| {
        crate::storage::plugins::DbKernelWriter::open()
            .map(|w| Box::new(w) as Box<dyn crate::session::plugin_mutations::KernelWriter>)
    })
}

/// The user's keymap, or the defaults when there is no override file.
fn user_keybindings() -> crate::session::KeyBindings {
    match crate::storage::keybindings::load_keybindings_json() {
        Ok(Some(json)) => crate::session::KeyBindings::from_json(&json).unwrap_or_default(),
        _ => crate::session::KeyBindings::default(),
    }
}

/// Register the declarations against a keymap, so the reported chord is the one
/// that would resolve — a user's rebind included.
///
/// Takes the keymap rather than loading it, so the report is testable without
/// depending on whatever is in the developer's own keybindings file.
fn keybinding_report(
    mut bindings: crate::session::KeyBindings,
    decls: Vec<crate::session::keybindings::PaneBindingDecl>,
) -> Vec<KeybindingReport> {
    let mut dropped: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    for drop in bindings.register_pane_bindings(decls) {
        dropped.insert(drop.id.qualified(), drop.to_string());
    }
    bindings
        .pane_bindings()
        .map(|(id, binding)| KeybindingReport {
            binding: id.qualified(),
            title: binding.title.clone(),
            chord: binding
                .chords
                .first()
                .map(crate::session::KeyChord::display),
            unbound_because: dropped.get(&id.qualified()).cloned(),
        })
        .collect()
}

/// Resolve the published contributions the way a spawn would.
fn resolve_for_report(
    registry: &crate::session::spawn_contribution::Registry,
) -> crate::session::spawn_contribution::ResolvedContributions {
    registry.resolve(&crate::session_ops::reserved_spawn_env_keys())
}

/// Run a plugin-owned `thurbox-cli <verb>`, if the first argument is one.
///
/// Returns the process exit code when a plugin claimed the verb, or `None` to
/// let clap parse normally. Matching is against **declared** verbs only, so an
/// ordinary typo still produces clap's unknown-subcommand error rather than a
/// confusing plugin failure.
pub fn dispatch_plugin_verb(mut args: std::env::Args) -> Option<i32> {
    let _program = args.next()?;
    let verb = args.next()?;
    if verb.starts_with('-') {
        return None;
    }
    let rest: Vec<String> = args.collect();

    let outcome = crate::plugin::discovery::discover();
    let owner = outcome
        .loadable
        .iter()
        .find(|p| p.manifest.cli.iter().any(|c| c.name == verb))?;

    Some(match run_plugin_verb(owner, &verb, &rest) {
        Ok(output) => {
            if !output.is_empty() {
                println!("{output}");
            }
            0
        }
        Err(e) => {
            eprintln!("error: {e}");
            1
        }
    })
}

/// Start the owning plugin's service, hand it the verb, and stop it.
///
/// Dispatched to the **service** half because a plugin verb must work with no
/// TUI running — which is the whole reason a plugin would own one.
fn run_plugin_verb(
    plugin: &crate::plugin::DiscoveredPlugin,
    verb: &str,
    args: &[String],
) -> Result<String, String> {
    if !plugin.manifest.has_service() {
        return Err(format!(
            "plugin `{}` declares the verb `{verb}` but has no service half to run it",
            plugin.name()
        ));
    }

    let path = crate::paths::database_file().ok_or("cannot resolve the database path")?;
    let lock_db = crate::storage::Database::open(&path).map_err(|e| e.to_string())?;
    let lock = crate::storage::plugins::DbServiceLock::new(lock_db, "cli");
    let store = store_factory();

    let mut host = crate::plugin::ServiceHost::new(crate::plugin::ExecutionBounds::default());
    host.start(plugin, &lock, Some(store), Some(writer_factory()))
        .map_err(|e| e.to_string())?;
    let result = host.run_verb(plugin.name(), verb, args);
    host.stop_all(&lock);
    result.map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::Path;
    use std::time::Duration;

    // Fully-qualified throughout, like the rest of `cli`: the architecture
    // rules allow `cli -> plugin` only via paths, so each place that starts a
    // VM says so at the call site.
    fn bounds() -> crate::plugin::ExecutionBounds {
        crate::plugin::ExecutionBounds {
            interrupt_budget: 5_000,
            memory_limit: 8 * 1024 * 1024,
            shutdown_timeout: Duration::from_millis(300),
        }
    }

    fn write_plugin(root: &Path, name: &str, caps: &str, entry: &str) {
        let dir = root.join(name);
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("plugin.toml"),
            format!("name = \"{name}\"\napi_version = 1\n{caps}"),
        )
        .unwrap();
        fs::write(dir.join("init.luau"), entry).unwrap();
    }

    /// Build a started report over an explicit root, bypassing the real
    /// config directory the way the production entry points cannot.
    fn started_over(root: &Path) -> crate::plugin::PluginReport {
        let mut host = crate::plugin::PluginHost::from_discovery(
            crate::plugin::discovery::discover_in(&[], Some(root)),
            bounds(),
        );
        host.start_all();
        host.report()
    }

    /// Run doctor over an explicit root, mirroring what `Action::Doctor` does.
    fn doctor_over(root: &Path) -> CommandOutput {
        let outcome = crate::plugin::discovery::discover_in(&[], Some(root));
        let registry = crate::plugin::spawn::registry_for(outcome.loadable.iter());
        // The real path registers against the *user's* keymap; a test must not
        // depend on what is in the developer's own keybindings file.
        let keys = keybinding_report(
            crate::session::KeyBindings::default(),
            crate::plugin::keymap::declarations_for(outcome.loadable.iter()),
        );
        let report = crate::plugin::PluginHost::from_discovery(outcome, bounds()).report();
        render_doctor(&report, &registry, &keys)
    }

    #[test]
    fn list_reports_healthy_and_failed_plugins() {
        let tmp = tempfile::tempdir().unwrap();
        write_plugin(
            tmp.path(),
            "good",
            "capabilities = [\"log\"]\n",
            "return {}",
        );
        write_plugin(tmp.path(), "broken", "", "return {");

        let out = render_list(&started_over(tmp.path()));
        let plugins = out.json["plugins"].as_array().unwrap();

        assert_eq!(plugins.len(), 2, "both plugins must appear");
        let by_name = |n: &str| {
            plugins
                .iter()
                .find(|p| p["name"] == n)
                .unwrap_or_else(|| panic!("{n} missing"))
        };
        assert_eq!(by_name("good")["state"], "running");
        assert_eq!(by_name("good")["capabilities"][0], "log");
        assert_eq!(by_name("broken")["state"], "failed");
        assert!(out.human.contains("good") && out.human.contains("broken"));
    }

    #[test]
    fn list_with_no_plugins_succeeds() {
        let tmp = tempfile::tempdir().unwrap();
        let out = render_list(&started_over(tmp.path()));

        assert!(out.json["plugins"].as_array().unwrap().is_empty());
        assert!(out.failure.is_none());
        assert!(out.human.contains("No plugins"));
    }

    #[test]
    fn status_names_the_failing_transition_and_cause() {
        let tmp = tempfile::tempdir().unwrap();
        write_plugin(
            tmp.path(),
            "angry",
            "",
            "return { init = function() error('kaboom') end }",
        );

        let out = status(&started_over(tmp.path()), Some("angry")).expect("known plugin");

        assert_eq!(out.json["state"], "failed");
        assert_eq!(out.json["failed_during"], "initialize");
        assert!(
            out.json["cause"].as_str().unwrap().contains("kaboom"),
            "{:?}",
            out.json["cause"]
        );
        assert!(out.human.contains("kaboom"));
    }

    #[test]
    fn status_for_an_unknown_plugin_is_an_error() {
        let tmp = tempfile::tempdir().unwrap();
        write_plugin(tmp.path(), "real", "", "return {}");

        let err = status(&started_over(tmp.path()), Some("imaginary"))
            .expect_err("unknown names must not succeed silently");
        assert!(err.contains("imaginary"), "{err}");
    }

    #[test]
    fn status_without_a_name_reports_every_plugin() {
        let tmp = tempfile::tempdir().unwrap();
        write_plugin(tmp.path(), "one", "", "return {}");
        write_plugin(tmp.path(), "two", "", "return {}");

        let out = status(&started_over(tmp.path()), None).expect("all");
        assert_eq!(out.json["plugins"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn a_healthy_status_carries_no_failure_fields() {
        let tmp = tempfile::tempdir().unwrap();
        write_plugin(tmp.path(), "fine", "", "return {}");

        let out = status(&started_over(tmp.path()), Some("fine")).expect("known");
        assert_eq!(out.json["state"], "running");
        assert!(out.json.get("failed_during").is_none());
        assert!(out.json.get("cause").is_none());
    }

    #[test]
    fn doctor_reports_an_invalid_manifest() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("bad");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("plugin.toml"), "name = \"Bad\"\napi_version = 1").unwrap();

        let out = doctor_over(tmp.path());
        let problems = out.json["problems"].as_array().unwrap();

        assert_eq!(problems.len(), 1);
        let text = problems[0].as_str().unwrap();
        assert!(text.contains("plugin.toml"), "{text}");
        assert!(text.contains("lowercase"), "{text}");
    }

    #[test]
    fn doctor_reports_each_pane_binding_and_why_one_is_unbound() {
        let tmp = tempfile::tempdir().unwrap();
        write_plugin(
            tmp.path(),
            "notes",
            "capabilities = [\"render\", \"input\"]\n[[panes]]\nid = \"board\"\n\
             [[keybindings]]\nid = \"next\"\npane = \"board\"\nchord = \"j\"\n\
             [[keybindings]]\nid = \"quit\"\npane = \"board\"\nchord = \"ctrl+q\"\n\
             [[keybindings]]\nid = \"spare\"\npane = \"board\"\n",
            "return { render = function() return { kind = \"divider\" } end }",
        );

        let out = doctor_over(tmp.path());
        let keys = out.json["keybindings"].as_array().expect("section present");
        assert_eq!(keys.len(), 3);

        let row = |binding: &str| {
            keys.iter()
                .find(|k| k["binding"] == format!("notes.board.{binding}"))
                .cloned()
                .expect(binding)
        };
        assert_eq!(row("next")["chord"], "j");
        // The kernel's quit chord is not stolen by a manifest, and the row says so.
        assert!(row("quit")["chord"].is_null());
        assert!(row("quit")["unbound_because"]
            .as_str()
            .unwrap()
            .contains("Quit"));
        // A binding that declared no chord is unbound for a different reason, and
        // the two must be distinguishable.
        assert!(row("spare")["chord"].is_null());
        assert!(row("spare")["unbound_because"].is_null());
        assert!(out.human.contains("Pane keybindings:"), "{}", out.human);
    }

    #[test]
    fn doctor_starts_nothing_to_report_a_keybinding() {
        let tmp = tempfile::tempdir().unwrap();
        write_plugin(
            tmp.path(),
            "notes",
            "capabilities = [\"render\", \"input\"]\n[[panes]]\nid = \"board\"\n\
             [[keybindings]]\nid = \"next\"\npane = \"board\"\nchord = \"j\"\n",
            // Would fail loudly if the plugin were ever run.
            "error('must not run')",
        );

        let out = doctor_over(tmp.path());
        assert_eq!(out.json["keybindings"][0]["chord"], "j");
    }

    #[test]
    fn doctor_reports_an_override() {
        let tmp = tempfile::tempdir().unwrap();
        let bundled_root = tmp.path().join("bundled");
        let user_root = tmp.path().join("user");
        fs::create_dir_all(&user_root).unwrap();
        write_plugin(&bundled_root, "tasks", "", "return {}");
        write_plugin(&user_root, "tasks", "", "return {}");

        let host = crate::plugin::PluginHost::from_discovery(
            crate::plugin::discovery::discover_in(&[bundled_root.join("tasks")], Some(&user_root)),
            bounds(),
        );
        let out = render_doctor(
            &host.report(),
            &crate::session::spawn_contribution::Registry::default(),
            &[],
        );

        let problems = out.json["problems"].as_array().unwrap();
        assert_eq!(problems.len(), 1);
        assert!(problems[0].as_str().unwrap().contains("overridden"));
    }

    #[test]
    fn doctor_reports_a_same_source_conflict() {
        let tmp = tempfile::tempdir().unwrap();
        write_plugin(tmp.path(), "copy-a", "", "return {}");
        write_plugin(tmp.path(), "copy-b", "", "return {}");
        // Both declare the same plugin name from different directories.
        for d in ["copy-a", "copy-b"] {
            fs::write(
                tmp.path().join(d).join("plugin.toml"),
                "name = \"dup\"\napi_version = 1\n",
            )
            .unwrap();
        }

        let out = doctor_over(tmp.path());
        let problems = out.json["problems"].as_array().unwrap();
        assert_eq!(problems.len(), 1);
        assert!(problems[0]
            .as_str()
            .unwrap()
            .contains("more than one plugin"));
    }

    #[test]
    fn doctor_is_clean_when_nothing_is_wrong() {
        let tmp = tempfile::tempdir().unwrap();
        write_plugin(tmp.path(), "fine", "", "return {}");

        let out = doctor_over(tmp.path());
        assert!(out.json["problems"].as_array().unwrap().is_empty());
        assert_eq!(out.json["healthy"], 1);
        assert!(out.human.contains("No problems"));
    }

    #[test]
    fn reload_reports_each_plugins_resulting_state() {
        let tmp = tempfile::tempdir().unwrap();
        write_plugin(tmp.path(), "one", "", "return {}");
        write_plugin(tmp.path(), "two", "", "return {}");

        let mut host = crate::plugin::PluginHost::from_discovery(
            crate::plugin::discovery::discover_in(&[], Some(tmp.path())),
            bounds(),
        );
        host.start_all();

        let outcomes = host.reload_all();
        assert_eq!(outcomes.len(), 2);
        for (_, state) in &outcomes {
            assert!(state.is_running(), "{state:?}");
        }
    }

    #[test]
    fn reload_surfaces_a_broken_edit() {
        let tmp = tempfile::tempdir().unwrap();
        write_plugin(tmp.path(), "fragile", "", "return {}");

        let mut host = crate::plugin::PluginHost::from_discovery(
            crate::plugin::discovery::discover_in(&[], Some(tmp.path())),
            bounds(),
        );
        host.start_all();

        fs::write(tmp.path().join("fragile").join("init.luau"), "return {").unwrap();
        let state = host.reload("fragile").expect("reload runs");
        assert!(state.is_failed(), "{state:?}");
    }

    #[test]
    fn doctor_lists_what_a_contribution_adds_and_what_is_refused() {
        let tmp = tempfile::tempdir().unwrap();
        write_plugin(
            tmp.path(),
            "ci",
            "capabilities = [\"spawn\"]\n[spawn.env]\nCI_TOKEN_FILE = \"/run/secrets/ci\"\nLD_PRELOAD = \"/tmp/pwn.so\"\n",
            "return {}",
        );

        let out = doctor_over(tmp.path());
        let env = out.json["spawn"]["env"].as_array().unwrap();
        assert_eq!(env, &[serde_json::json!("CI_TOKEN_FILE")]);

        let refused = out.json["spawn"]["refused"].as_array().unwrap();
        assert_eq!(refused.len(), 1);
        let reason = refused[0].as_str().unwrap();
        assert!(reason.contains("ci"), "{reason}");
        assert!(reason.contains("LD_PRELOAD"), "{reason}");
        assert!(out.human.contains("Spawn contributions"), "{}", out.human);
    }

    #[test]
    fn doctor_refuses_a_contribution_aimed_at_the_session_identity() {
        // The reserved set is derived from the kernel injection itself, so
        // this stays true if the injected set ever changes.
        let tmp = tempfile::tempdir().unwrap();
        write_plugin(
            tmp.path(),
            "impostor",
            "capabilities = [\"spawn\"]\n[spawn.env]\nTHURBOX_SESSION = \"someone-else\"\n",
            "return {}",
        );

        let out = doctor_over(tmp.path());
        assert!(out.json["spawn"]["env"].as_array().unwrap().is_empty());
        let reason = out.json["spawn"]["refused"][0].as_str().unwrap();
        assert!(reason.contains("THURBOX_SESSION"), "{reason}");
    }

    #[test]
    fn doctor_says_nothing_about_spawns_when_nothing_contributes() {
        let tmp = tempfile::tempdir().unwrap();
        write_plugin(tmp.path(), "quiet", "", "return {}");

        let out = doctor_over(tmp.path());
        assert!(out.json["spawn"]["env"].as_array().unwrap().is_empty());
        assert!(!out.human.contains("Spawn contributions"), "{}", out.human);
    }

    #[test]
    fn doctor_does_not_start_plugins() {
        let tmp = tempfile::tempdir().unwrap();
        // An entry chunk that would fail loudly if it were ever run.
        write_plugin(tmp.path(), "never-run", "", "error('must not run')");

        let out = doctor_over(tmp.path());
        assert!(
            out.json["problems"].as_array().unwrap().is_empty(),
            "doctor must not execute plugin code"
        );
        assert_eq!(out.json["healthy"], 1);
    }
}
