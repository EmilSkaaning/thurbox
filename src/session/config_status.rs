//! Pure data describing one config file's on-disk state + validity — the
//! discoverability/validation snapshot surfaced in the TUI (the Settings
//! panel's "Config files" section and the persistent `Config ⚠` footer badge).
//!
//! The *behavior* that fills these in (path resolution + strict parsing) lives
//! in [`crate::session_ops::config_check`]; this module is pure data so both
//! `ui` (which may reference `session` but not `session_ops`) and the CLI can
//! consume the same type without an architecture-rule violation.

use std::path::PathBuf;

/// A config file's semantic validity, independent of any display string — so
/// renderers switch on this rather than on the [`ConfigFile::mark`] text (which
/// would couple styling to the exact glyphs).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigValidity {
    /// A validated file that exists and parsed cleanly.
    Ok,
    /// A validated file that exists but failed to parse.
    Invalid,
    /// A validated file that isn't on disk (defaults/seeding apply).
    Absent,
    /// A path-only entry (the database) — never parsed, so no mark.
    Unchecked,
}

/// One config file's resolved path, existence, and validity.
///
/// `validated` distinguishes files we strict-parse (`agents.toml`,
/// `keybindings.json`, …) from path-only entries like the database, which have
/// no parseable schema — so an absent DB path isn't reported as a "problem".
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigFile {
    /// Display name, e.g. `agents.toml` / `keybindings.json` / `database`.
    pub label: &'static str,
    /// Resolved absolute path (`None` when the path couldn't be resolved at all
    /// — no home dir, etc.).
    pub path: Option<PathBuf>,
    /// Whether the file exists on disk.
    pub exists: bool,
    /// Whether it parsed cleanly (always `true` for absent validated files —
    /// defaults/seeding apply — and for path-only entries).
    pub valid: bool,
    /// Whether validity was actually checked (a strict parse). Path-only
    /// entries (the database) are `false` and never render a validity mark.
    pub validated: bool,
    /// Human-readable problems (parse errors, unknown keys, conflicts). Empty
    /// unless `valid` is `false`.
    pub problems: Vec<String>,
}

impl ConfigFile {
    /// The file's semantic validity — the single source of truth behind both
    /// [`Self::mark`] and [`Self::is_problem`], and what renderers should switch
    /// on to pick styling.
    pub fn validity(&self) -> ConfigValidity {
        if !self.validated {
            return ConfigValidity::Unchecked;
        }
        match (self.exists, self.valid) {
            (false, _) => ConfigValidity::Absent,
            (true, true) => ConfigValidity::Ok,
            (true, false) => ConfigValidity::Invalid,
        }
    }

    /// An at-a-glance validity mark for the row: `✓ ok` / `✗ invalid` /
    /// `· absent`. Returns `None` for path-only entries (no parse to report).
    pub fn mark(&self) -> Option<&'static str> {
        match self.validity() {
            ConfigValidity::Unchecked => None,
            ConfigValidity::Ok => Some("✓ ok"),
            ConfigValidity::Invalid => Some("✗ invalid"),
            ConfigValidity::Absent => Some("· absent"),
        }
    }

    /// Whether this file is a *problem* worth flagging (a validated file that
    /// exists but failed to parse). Absent files are fine (defaults apply).
    pub fn is_problem(&self) -> bool {
        matches!(self.validity(), ConfigValidity::Invalid)
    }

    /// The resolved path as a display string, or `—` when unresolved.
    pub fn path_display(&self) -> String {
        self.path
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "—".to_string())
    }
}

/// Whether any file in the snapshot is a problem (drives the footer badge).
pub fn any_problem(files: &[ConfigFile]) -> bool {
    files.iter().any(ConfigFile::is_problem)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(exists: bool, valid: bool, validated: bool) -> ConfigFile {
        ConfigFile {
            label: "x.toml",
            path: Some(PathBuf::from("/tmp/x.toml")),
            exists,
            valid,
            validated,
            problems: Vec::new(),
        }
    }

    #[test]
    fn validity_classifies_each_state() {
        assert_eq!(file(true, true, true).validity(), ConfigValidity::Ok);
        assert_eq!(file(true, false, true).validity(), ConfigValidity::Invalid);
        assert_eq!(file(false, true, true).validity(), ConfigValidity::Absent);
        // Path-only entries are never parsed, regardless of exists/valid.
        assert_eq!(
            file(true, true, false).validity(),
            ConfigValidity::Unchecked
        );
        assert_eq!(
            file(false, false, false).validity(),
            ConfigValidity::Unchecked
        );
    }

    #[test]
    fn mark_reflects_existence_and_validity() {
        assert_eq!(file(true, true, true).mark(), Some("✓ ok"));
        assert_eq!(file(true, false, true).mark(), Some("✗ invalid"));
        assert_eq!(file(false, true, true).mark(), Some("· absent"));
        // Path-only entries render no mark.
        assert_eq!(file(true, true, false).mark(), None);
    }

    #[test]
    fn is_problem_only_for_existing_invalid_validated_files() {
        assert!(file(true, false, true).is_problem());
        assert!(!file(false, false, true).is_problem()); // absent → fine
        assert!(!file(true, true, true).is_problem()); // valid → fine
        assert!(!file(true, false, false).is_problem()); // path-only → never
    }

    #[test]
    fn any_problem_detects_a_single_bad_file() {
        assert!(!any_problem(&[
            file(true, true, true),
            file(false, true, true)
        ]));
        assert!(any_problem(&[
            file(true, true, true),
            file(true, false, true)
        ]));
    }
}
