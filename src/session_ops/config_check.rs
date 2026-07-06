//! Config-file discovery + strict validation, shared by `thurbox-cli config`
//! and the TUI (Settings "Config files" section + `Config ⚠` footer badge).
//!
//! This is the single source of truth for *where* each config file lives and
//! *whether* it parses. The pure result type is [`crate::session::ConfigFile`]
//! (so `ui` can render it); the logic here does the disk reads + parsing.
//!
//! The `agent` loaders are reached via fully-qualified paths (no `use
//! crate::agent`), like `cli`/`session_ops` elsewhere — see
//! `tests/architecture_rules.rs`.

use crate::session::ConfigFile;

/// Validate every config file plus resolve the database path, in the order the
/// TUI/CLI render them. The database is a **path-only** entry (`validated =
/// false`): it has no parseable schema, so it never shows a validity mark or
/// counts as a "problem".
pub fn check_all() -> Vec<ConfigFile> {
    vec![
        check_toml::<crate::session::AgentRegistry>(
            "agents.toml",
            crate::agent::agent_config::agents_config_path(),
        ),
        check_toml::<crate::session::HostRegistry>(
            "hosts.toml",
            crate::agent::host_config::hosts_config_path(),
        ),
        check_toml::<crate::session::settings::Settings>(
            "settings.toml",
            crate::agent::settings_config::settings_config_path(),
        ),
        check_toml::<crate::session::theme_config::ThemesFile>(
            "themes.toml",
            crate::agent::themes_config::themes_config_path(),
        ),
        check_keybindings(),
        check_database(),
    ]
}

/// Validate one TOML config file against `T`. Absent files are valid
/// (defaults/seeding apply). Unknown fields don't fail the *load* at startup,
/// but they do fail *validation* — they are typos or stale keys either way.
fn check_toml<T: serde::de::DeserializeOwned>(
    label: &'static str,
    path: Option<std::path::PathBuf>,
) -> ConfigFile {
    let Some(path) = path else {
        return ConfigFile {
            label,
            path: None,
            exists: false,
            valid: false,
            validated: true,
            problems: vec!["could not resolve path".into()],
        };
    };
    if !path.exists() {
        return ConfigFile {
            label,
            path: Some(path),
            exists: false,
            valid: true,
            validated: true,
            problems: Vec::new(),
        };
    }
    let problems = match std::fs::read_to_string(&path) {
        Ok(contents) => {
            match crate::agent::agent_config::parse_toml_reporting_unknown::<T>(&contents, label) {
                Ok((_, warnings)) => warnings,
                Err(e) => vec![e.to_string()],
            }
        }
        Err(e) => vec![format!("read failed: {e}")],
    };
    ConfigFile {
        label,
        path: Some(path),
        exists: true,
        valid: problems.is_empty(),
        validated: true,
        problems,
    }
}

/// Validate `keybindings.json` — parses via the same loader the app uses, so a
/// chord conflict / skipped entry surfaces exactly like at startup.
fn check_keybindings() -> ConfigFile {
    let path = crate::paths::keybindings_file();
    let (exists, valid, problems) = match crate::storage::keybindings::load_keybindings_json() {
        Ok(Some(json)) => match crate::session::KeyBindings::from_json_with_warnings(&json) {
            Ok((_, warnings)) => (true, warnings.is_empty(), warnings),
            Err(e) => (true, false, vec![e]),
        },
        Ok(None) => (false, true, Vec::new()),
        Err(e) => (true, false, vec![e]),
    };
    ConfigFile {
        label: "keybindings.json",
        path,
        exists,
        valid,
        validated: true,
        problems,
    }
}

/// The SQLite database — a path-only entry: it's created on demand and has no
/// schema to validate here, so it renders as a discoverable path with no mark.
fn check_database() -> ConfigFile {
    let path = crate::paths::database_file();
    let exists = path.as_ref().is_some_and(|p| p.exists());
    ConfigFile {
        label: "database",
        path,
        exists,
        valid: true,
        validated: false,
        problems: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paths::TestPathGuard;

    #[test]
    fn check_all_passes_on_a_fresh_environment() {
        let tmp = tempfile::tempdir().unwrap();
        let _g = TestPathGuard::new(tmp.path());
        let files = check_all();
        assert!(
            !crate::session::any_problem(&files),
            "fresh env should have no problems: {files:?}"
        );
        // Every declared file is present, in order.
        let labels: Vec<_> = files.iter().map(|f| f.label).collect();
        assert_eq!(
            labels,
            vec![
                "agents.toml",
                "hosts.toml",
                "settings.toml",
                "themes.toml",
                "keybindings.json",
                "database",
            ]
        );
    }

    #[test]
    fn check_all_flags_a_malformed_toml() {
        let tmp = tempfile::tempdir().unwrap();
        let _g = TestPathGuard::new(tmp.path());
        let path = crate::agent::agent_config::agents_config_path().unwrap();
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, "not toml {{{").unwrap();

        let files = check_all();
        assert!(crate::session::any_problem(&files));
        let agents = files.iter().find(|f| f.label == "agents.toml").unwrap();
        assert!(agents.is_problem());
        assert!(!agents.problems.is_empty());
    }

    #[test]
    fn check_all_flags_an_unknown_settings_key() {
        let tmp = tempfile::tempdir().unwrap();
        let _g = TestPathGuard::new(tmp.path());
        let path = crate::agent::settings_config::settings_config_path().unwrap();
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, "[features]\nbogus = true\n").unwrap();

        let files = check_all();
        let settings = files.iter().find(|f| f.label == "settings.toml").unwrap();
        assert!(settings.is_problem(), "unknown key must fail validation");
    }

    #[test]
    fn database_is_path_only_never_a_problem() {
        let tmp = tempfile::tempdir().unwrap();
        let _g = TestPathGuard::new(tmp.path());
        let files = check_all();
        let db = files.iter().find(|f| f.label == "database").unwrap();
        assert!(!db.validated);
        assert_eq!(db.mark(), None);
        assert!(!db.is_problem());
    }
}
