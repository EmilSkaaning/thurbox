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

use std::collections::BTreeSet;
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
        ]
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

/// A command a plugin contributes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommandDecl {
    /// Unique within this manifest's commands.
    pub id: String,
    /// Human-readable title. Defaults to the id when absent.
    #[serde(default)]
    pub title: Option<String>,
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
    /// Host powers this plugin requests. Anything absent here is unreachable
    /// from its VM.
    #[serde(default)]
    pub capabilities: BTreeSet<Capability>,
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
    /// A pane is declared without the capability that would let it render.
    PaneWithoutRender {
        /// The first pane declared without it.
        pane: String,
    },
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
            ManifestErrorKind::PaneWithoutRender { pane } => write!(
                f,
                "pane `{pane}` is declared without the `render` capability, so it could never draw"
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

        check_ids("pane", self.panes.iter().map(|p| p.id.as_str()))?;
        check_ids("command", self.commands.iter().map(|c| c.id.as_str()))?;
        check_ids("keybinding", self.keybindings.iter().map(|k| k.id.as_str()))?;

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
    fn error_display_names_the_path() {
        let e = parse("api_version = 1").expect_err("invalid");
        assert!(e.to_string().starts_with("/plugins/demo/plugin.toml: "));
    }
}
