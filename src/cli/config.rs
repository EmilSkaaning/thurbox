//! Config introspection subcommands: `validate` and `show`.
//!
//! `validate` strictly parses every config file and fails (exit 1) when any
//! is invalid — usable as a dotfiles CI check. `show` prints the *effective*
//! resolved configuration and where each value came from.
//!
//! The agent module's loaders are reached via fully-qualified paths (no
//! `use crate::agent`) to keep the cli module free of an `agent` import —
//! see tests/architecture_rules.rs::cli_module_isolation.

use clap::Subcommand;
use serde_json::{json, Value};

use crate::cli::output::{self, CommandOutput};
use crate::storage::Database;

#[derive(Subcommand, Debug)]
pub enum Action {
    /// Parse every config file strictly; non-zero exit when any is invalid.
    Validate,
    /// Print the effective configuration and where each value came from.
    Show,
}

pub fn run(action: Action, db: &Database) -> Result<CommandOutput, String> {
    match action {
        Action::Validate => {
            let (report, failed) = validate();
            let human = render_validate(&report, &failed);
            if failed.is_empty() {
                Ok(CommandOutput::new(report, human))
            } else {
                // Exit non-zero so this is usable as a dotfiles CI gate.
                Ok(CommandOutput::failed(
                    report,
                    human,
                    format!("config invalid: {}", failed.join(", ")),
                ))
            }
        }
        Action::Show => {
            let report = show(db)?;
            let human = render_show(&report);
            Ok(CommandOutput::new(report, human))
        }
    }
}

/// The validated config files in stable order, as `(display label, JSON key)`
/// pairs. Shared by [`validate`] (which builds the report) and
/// [`render_validate`] (which prints it) so their ordering can't drift. The
/// database is path-only and lives in `show`'s paths block, not here.
const VALIDATED_FILES: [(&str, &str); 5] = [
    ("agents.toml", "agents_toml"),
    ("hosts.toml", "hosts_toml"),
    ("settings.toml", "settings_toml"),
    ("themes.toml", "themes_toml"),
    ("keybindings.json", "keybindings_json"),
];

/// One file's validation outcome as the JSON block `config validate`/`show`
/// consumers expect.
fn file_report(file: &crate::session::ConfigFile) -> Value {
    json!({
        "path": file.path.as_ref().map(|p| p.display().to_string()),
        "exists": file.exists,
        "valid": file.valid,
        "problems": file.problems,
    })
}

/// Validate every config file. Returns the full report plus the list of files
/// that failed (empty = all valid). Sources its data from the shared
/// [`crate::session_ops::config_check`] so the TUI badge/section and the CLI
/// never disagree about validity.
fn validate() -> (Value, Vec<String>) {
    let files = crate::session_ops::config_check::check_all();
    let get = |label: &str| files.iter().find(|f| f.label == label);

    let failed: Vec<String> = VALIDATED_FILES
        .iter()
        .filter(|(label, _)| get(label).is_some_and(|f| !f.valid))
        .map(|(label, _)| (*label).to_string())
        .collect();

    let mut report = json!({ "valid": failed.is_empty() });
    for (label, key) in VALIDATED_FILES {
        if let Some(f) = get(label) {
            report[key] = file_report(f);
        }
    }
    (report, failed)
}

/// Render `config validate` as a per-file status list.
fn render_validate(report: &Value, failed: &[String]) -> String {
    let mut lines = Vec::new();
    for (label, key) in VALIDATED_FILES {
        push_validate_file_lines(&mut lines, label, &report[key]);
    }
    if failed.is_empty() {
        lines.push("All config files valid.".to_string());
    } else {
        lines.push(format!("Invalid: {}", failed.join(", ")));
    }
    lines.join("\n")
}

/// Append one config file's status line (and any problem lines) to `lines`.
fn push_validate_file_lines(lines: &mut Vec<String>, label: &str, entry: &Value) {
    let exists = entry["exists"].as_bool().unwrap_or(false);
    let valid = entry["valid"].as_bool().unwrap_or(false);
    // Absent files are valid (defaults/seeding apply), so flag them apart.
    let (mark, status) = match (exists, valid) {
        (false, _) => ("·", "absent"),
        (true, true) => ("✓", "ok"),
        (true, false) => ("✗", "invalid"),
    };
    lines.push(format!("{mark} {label}  {status}"));
    if let Some(problems) = entry["problems"].as_array() {
        for p in problems {
            if let Some(p) = p.as_str() {
                lines.push(format!("    - {p}"));
            }
        }
    }
}

/// Render `config show` as grouped key/value blocks.
fn render_show(report: &Value) -> String {
    let mut sections: Vec<String> = Vec::new();

    if let Some(paths) = report["paths"].as_object() {
        let pairs: Vec<(&str, String)> = paths
            .iter()
            .map(|(k, v)| (k.as_str(), output::dash(v.as_str())))
            .collect();
        sections.push(format!("Paths\n{}", output::kv(&pairs)));
    }

    let agents = &report["agents"];
    let names = agents["names"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(Value::as_str)
                .collect::<Vec<_>>()
                .join(", ")
        })
        .unwrap_or_default();
    sections.push(format!(
        "Agents\n{}",
        output::kv(&[
            ("default", output::dash(agents["default"].as_str())),
            ("names", names),
        ])
    ));

    let editor = &report["editor"];
    sections.push(format!(
        "Editor\n{}",
        output::kv(&[
            ("command", output::dash(editor["command"].as_str())),
            ("source", output::dash(editor["source"].as_str())),
        ])
    ));

    sections.push(format!(
        "Theme\n{}",
        output::kv(&[("active", output::dash(report["theme"].as_str()))])
    ));

    sections.join("\n\n")
}

fn show(db: &Database) -> Result<Value, String> {
    let agents = crate::agent::agent_config::load_or_seed();
    // The *effective* host set: configured SSH/WSL hosts plus auto-discovered
    // WSL distros — matching what `--host` and the TUI picker actually offer
    // (`config validate` stays file-only).
    let hosts = crate::agent::host_config::load_all();
    let settings = crate::session::settings::global();
    let (custom_themes, _) = crate::agent::themes_config::load_or_seed_with_warnings();

    // Editor resolution mirrors the TUI's Ctrl+O chain: DB → $VISUAL → $EDITOR.
    let (editor, editor_source) = resolve_editor(db);
    let overridden_actions = overridden_action_names();

    // Path set from the shared checker, so `show` and `validate` (and the TUI's
    // "Config files" section) resolve every path the same way.
    let files = crate::session_ops::config_check::check_all();
    let path_of = |label: &str| {
        files
            .iter()
            .find(|f| f.label == label)
            .and_then(|f| f.path.as_ref())
            .map(|p| p.display().to_string())
    };

    Ok(json!({
        "paths": {
            "agents_toml": path_of("agents.toml"),
            "hosts_toml": path_of("hosts.toml"),
            "settings_toml": path_of("settings.toml"),
            "themes_toml": path_of("themes.toml"),
            "keybindings_json": path_of("keybindings.json"),
            "database": path_of("database"),
        },
        "agents": { "default": agents.default_name(), "names": agents.names() },
        "hosts": { "names": hosts.names() },
        "settings": settings,
        "keybindings": { "overridden_actions": overridden_actions },
        "editor": { "command": editor, "source": editor_source },
        "theme": db.get_active_theme().ok().flatten(),
        "custom_themes": custom_themes.iter().map(|t| t.name.as_str()).collect::<Vec<_>>(),
    }))
}

/// Resolve the effective editor command + its source, mirroring the TUI's
/// Ctrl+O chain: DB (editor_command) → $VISUAL → $EDITOR → unset.
fn resolve_editor(db: &Database) -> (Option<String>, &'static str) {
    let db_editor = db.get_editor_command().ok().flatten();
    if let Some(cmd) = db_editor.filter(|c| !c.is_empty()) {
        return (Some(cmd), "database (editor_command)");
    }
    if let Some(v) = nonempty_env("VISUAL") {
        return (Some(v), "$VISUAL");
    }
    if let Some(v) = nonempty_env("EDITOR") {
        return (Some(v), "$EDITOR");
    }
    (None, "unset")
}

/// A non-empty environment variable value, or `None` when unset/empty.
fn nonempty_env(key: &str) -> Option<String> {
    std::env::var(key).ok().filter(|v| !v.is_empty())
}

/// The sorted action names overridden in keybindings.json (empty when none).
fn overridden_action_names() -> Vec<String> {
    match crate::storage::keybindings::load_keybindings_json() {
        Ok(Some(jsonbody)) => {
            serde_json::from_str::<std::collections::HashMap<String, Vec<String>>>(&jsonbody)
                .map(|m| {
                    let mut keys: Vec<String> = m.into_keys().collect();
                    keys.sort();
                    keys
                })
                .unwrap_or_default()
        }
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paths::TestPathGuard;

    #[test]
    fn validate_passes_on_fresh_environment() {
        let tmp = tempfile::tempdir().unwrap();
        let _g = TestPathGuard::new(tmp.path());
        let (v, failed) = validate();
        assert!(failed.is_empty(), "got failures: {failed:?}");
        assert_eq!(v["valid"], json!(true));
    }

    #[test]
    fn validate_fails_with_exit_error_on_malformed_agents_toml() {
        let tmp = tempfile::tempdir().unwrap();
        let _g = TestPathGuard::new(tmp.path());
        let path = crate::agent::agent_config::agents_config_path().unwrap();
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, "not toml {{{").unwrap();

        let (_, failed) = validate();
        assert!(failed.iter().any(|f| f == "agents.toml"), "got: {failed:?}");
    }

    #[test]
    fn validate_reports_keybinding_conflicts() {
        let tmp = tempfile::tempdir().unwrap();
        let _g = TestPathGuard::new(tmp.path());
        crate::storage::keybindings::save_keybindings_json(
            r#"{ "QuitApp": ["ctrl+a"], "NewSession": ["ctrl+a"] }"#,
        )
        .unwrap();

        let (_, failed) = validate();
        assert!(
            failed.iter().any(|f| f == "keybindings.json"),
            "got: {failed:?}"
        );
    }

    /// `serde_ignored` reports nested unknown keys, so a typo inside
    /// `[features]` must fail strict validation just like a top-level one.
    #[test]
    fn validate_fails_on_unknown_feature_flag() {
        let tmp = tempfile::tempdir().unwrap();
        let _g = TestPathGuard::new(tmp.path());
        let path = crate::agent::settings_config::settings_config_path().unwrap();
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, "[features]\nbogus = true\n").unwrap();

        let (_, failed) = validate();
        assert!(
            failed.iter().any(|f| f == "settings.toml"),
            "got: {failed:?}"
        );
    }

    #[test]
    fn show_reports_effective_settings_and_editor_source() {
        let tmp = tempfile::tempdir().unwrap();
        let _g = TestPathGuard::new(tmp.path());
        let db = Database::open_in_memory().unwrap();
        db.set_editor_command("code --wait").unwrap();

        let v = show(&db).unwrap();
        assert_eq!(v["editor"]["command"], json!("code --wait"));
        assert_eq!(v["editor"]["source"], json!("database (editor_command)"));
        assert!(v["settings"]["scrollback_lines"].is_number());
        assert_eq!(v["settings"]["features"]["tasks"], json!(true));
        assert_eq!(v["agents"]["default"], json!("claude"));
    }
}
