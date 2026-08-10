//! The v2 teardown inventory, enforced as a test (allowlist model).
//!
//! `docs/v2/MIGRATION.md` §4 lists what the final v2 phase deletes and, next to
//! it, the v1 capabilities that "must not be lost". This file is that inventory
//! in executable form: [`REPLACEMENTS`] records whether each capability's v2 home
//! exists yet, and [`UNITS`] records what may not be deleted until it does.
//!
//! Why a test rather than a document. A *half*-deleted pane is loud — the
//! compiler catches the dangling callers, and this gate never even runs. A
//! *cleanly* deleted one is silent, and so is a deleted hooks installer: the
//! binary compiles, sessions launch, and status reporting stops for every agent.
//! The quiet cases are exactly the ones a table can catch. A verdict written in
//! markdown is also a fact about a build that expires without telling anyone, so
//! every verdict here is re-derived from the source and must agree with what is
//! recorded ([`recorded_verdicts_match_the_tree`]).
//!
//! Probes read the source tree the way a human auditor would, which keeps the
//! gate free of the `plugins` Cargo feature — it runs, and means the same thing,
//! in both configurations.
//!
//! Mirrors `tests/architecture_rules.rs`: a table plus a rule that fails when
//! the tree and the table disagree. `docs/PHASE6-TEARDOWN-READINESS.md` is the
//! prose companion — the same verdicts with their evidence, and the worklist that
//! unblocks them.

use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

/// One v1 capability the teardown must not lose, and the v2 home MIGRATION §4
/// promises it.
#[derive(Clone, Copy)]
struct Replacement {
    /// Stable id a [`TeardownUnit`] requires.
    id: &'static str,
    /// The v1 behaviour, in MIGRATION's words.
    v1_capability: &'static str,
    /// Where v2 says it lives instead.
    v2_home: &'static str,
    /// The recorded verdict: does that home exist in this tree?
    ready: bool,
    /// Re-derives the verdict from the source (given the repo root and this
    /// row's id), so `ready` cannot go stale.
    probe: fn(&Path, &str) -> bool,
}

/// A thing the teardown deletes as one piece, and what must exist first.
struct TeardownUnit {
    name: &'static str,
    /// Files and directories the unit comprises. Each must still exist while the
    /// unit is blocked.
    paths: &'static [&'static str],
    /// `(file, needle)` pairs the unit also comprises — for state that is not a
    /// path of its own, such as a metadata key the schema cleanup would drop.
    markers: &'static [(&'static str, &'static str)],
    /// [`Replacement::id`]s that must all be ready before this unit may go.
    requires: &'static [&'static str],
}

/// Every capability MIGRATION §4 says must survive, plus one row per native pane
/// awaiting the plugin that replaces it.
///
/// The pane rows are the bundled-plugin half of the same question: a native pane
/// may only be deleted once a plugin renders it, and today none does — which is
/// the machine-checked form of "the phase that produces the replacements has not
/// run".
const REPLACEMENTS: &[Replacement] = &[
    Replacement {
        id: "hooks-in-kernel",
        v1_capability: "agent status hooks (working/blocked/done)",
        v2_home: "the kernel session layer, not the extension installer",
        ready: false,
        probe: |root, _| {
            // Absorbed when the built-in wiring no longer goes through the
            // installer that the same teardown deletes.
            !source(root, "src/session_ops/builtin_hooks.rs").contains("install_extension(")
        },
    },
    Replacement {
        id: "agent-registration",
        v1_capability: "registering agents in agents.toml",
        v2_home: "plugin manifest `[[agents]]`",
        ready: false,
        probe: |root, _| manifest_field(root, "pub struct PluginManifest", "pub agents:"),
    },
    Replacement {
        id: "resource-seeding",
        v1_capability: "seeding sessions and automations",
        v2_home: "plugin manifest `[[automations]]` plus kernel-table host APIs",
        ready: false,
        probe: |root, _| manifest_field(root, "pub struct PluginManifest", "pub automations:"),
    },
    Replacement {
        id: "agent-config-files",
        v1_capability: "placing files in an agent's own config dir",
        v2_home: "a plugin `fs` capability",
        ready: false,
        // SECURITY §3 enforces filesystem denial by the *absence* of a binding,
        // so this row flips only on a deliberate capability decision.
        probe: |root, _| manifest_field(root, "pub enum Capability", "Fs,"),
    },
    Replacement {
        id: "spawn-arg-contribution",
        v1_capability: "patching agent args at spawn",
        v2_home: "spawn contributions carrying arguments, not only env",
        ready: false,
        probe: |root, _| manifest_field(root, "pub struct SpawnDecl", "pub args"),
    },
    Replacement {
        id: "self-heal",
        v1_capability: "self-heal of declared resources on startup and tick",
        v2_home: "idempotent by construction — discovery re-walks the manifests",
        ready: true,
        probe: |root, _| source(root, "src/plugin/discovery.rs").contains("pub fn discover("),
    },
    Replacement {
        id: "plugin-update",
        v1_capability: "version, staleness, and auto-update of an extension",
        v2_home: "`thurbox-cli plugin update`, pinned to the release tag",
        ready: false,
        probe: |root, _| {
            let actions = block(root, "src/cli/plugins.rs", "pub enum Action");
            actions.contains("Update") || actions.contains("Install")
        },
    },
    // One row per pane MIGRATION §2 schedules for the bundled-plugin phase.
    pane("info-panel-plugin", "the info panel"),
    pane("tasks-plugin", "the tasks pane"),
    pane("automations-plugin", "the automations pane"),
    pane("file-viewer-plugin", "the file viewer"),
    pane("global-search-plugin", "global search"),
    pane("code-review-plugin", "the code review view"),
    pane("session-list-plugin", "the session list"),
];

/// A pane's replacement row. Ready once a bundled plugin exists whose directory
/// is named after the pane — the id minus its `-plugin` suffix, in either
/// spelling, since the eventual plugin's name is not this gate's to fix.
const fn pane(id: &'static str, v1_capability: &'static str) -> Replacement {
    Replacement {
        id,
        v1_capability,
        v2_home: "a bundled plugin under src/plugin/bundled/",
        ready: false,
        probe: |root, id| {
            let stem = match id.strip_suffix("-plugin") {
                Some(s) => s,
                None => id,
            };
            [stem.to_string(), stem.replace('-', "_")]
                .iter()
                .any(|dir| {
                    root.join("src/plugin/bundled")
                        .join(dir)
                        .join("plugin.toml")
                        .exists()
                })
        },
    }
}

/// What the teardown deletes, and what each deletion waits on.
const UNITS: &[TeardownUnit] = &[
    // ADR-V8 deletes this as one piece and none of the five files is
    // independently useful, so the unit requires the whole capability set: the
    // installer is what delivers every one of them. A narrower mapping is
    // defensible — `json_merge.rs` serves only the hooks and config-dir rows —
    // but that is a judgement for whoever does the deletion to argue here, with
    // its reasons attached, rather than one this gate makes on their behalf.
    TeardownUnit {
        name: "the v1 extension system",
        paths: &[
            "src/session_ops/extensions.rs",
            "src/agent/extension_config.rs",
            "src/session/extension_def.rs",
            "src/cli/extensions.rs",
            "src/agent/json_merge.rs",
            "extensions",
            // The embedded hook payloads: `builtin_hooks` `include_str!`s these,
            // so they are a build dependency of agent status reporting and not
            // merely example data.
            "extensions/hooks/extension.toml",
            "extensions/hooks/claude.json",
        ],
        markers: &[
            // The two metadata keys the teardown's schema cleanup drops. Quoted,
            // so the needle is the stored key itself and not an accessor whose
            // name merely contains it (`get_active_extensions`).
            ("src/storage/settings.rs", "\"active_extensions\""),
            ("src/storage/settings.rs", "\"builtin_hooks_optout\""),
        ],
        requires: &[
            "hooks-in-kernel",
            "agent-registration",
            "resource-seeding",
            "agent-config-files",
            "spawn-arg-contribution",
            "self-heal",
            "plugin-update",
        ],
    },
    native_pane(
        "the info panel",
        &["src/ui/info_panel.rs"],
        &["info-panel-plugin"],
    ),
    native_pane(
        "the tasks pane",
        &["src/ui/tasks_panel.rs"],
        &["tasks-plugin"],
    ),
    native_pane(
        "the automations pane",
        &["src/ui/automations_panel.rs"],
        &["automations-plugin"],
    ),
    native_pane(
        "the file viewer",
        &["src/ui/file_viewer.rs"],
        &["file-viewer-plugin"],
    ),
    native_pane(
        "global search",
        &["src/ui/global_search.rs"],
        &["global-search-plugin"],
    ),
    native_pane(
        "the code review view",
        &["src/ui/code_review.rs"],
        &["code-review-plugin"],
    ),
    native_pane(
        "the session list",
        &["src/ui/project_list.rs"],
        &["session-list-plugin"],
    ),
];

/// A native pane's unit: its renderer may not go until its plugin exists.
const fn native_pane(
    name: &'static str,
    paths: &'static [&'static str],
    requires: &'static [&'static str],
) -> TeardownUnit {
    TeardownUnit {
        name,
        paths,
        markers: &[],
        requires,
    }
}

/// The ids whose v2 home does not exist yet — the reason deletion is unsafe.
/// Pure over the table so both answers are testable.
fn blockers(replacements: &[Replacement]) -> Vec<&'static str> {
    replacements
        .iter()
        .filter(|r| !r.ready)
        .map(|r| r.id)
        .collect()
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Read a source file, panicking if it is unreadable — a probe that silently
/// read nothing would report a replacement as absent for the wrong reason.
fn source(root: &Path, rel: &str) -> String {
    let path = root.join(rel);
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

/// The body of the `struct`/`enum` whose declaration starts with `header`.
///
/// Scoping a probe to one item stops an unrelated mention elsewhere in the file
/// from flipping a verdict. rustfmt closes a top-level item with a `}` in column
/// zero, which is the terminator used here.
fn block(root: &Path, rel: &str, header: &str) -> String {
    let text = source(root, rel);
    let start = text
        .find(header)
        .unwrap_or_else(|| panic!("{rel} no longer declares `{header}`"));
    let rest = &text[start..];
    let end = rest
        .find("\n}\n")
        .unwrap_or_else(|| panic!("{rel}: `{header}` has no top-level close"));
    rest[..end].to_string()
}

/// Whether the plugin manifest's `header` item declares `needle`.
fn manifest_field(root: &Path, header: &str, needle: &str) -> bool {
    block(root, "src/session/plugin_manifest.rs", header).contains(needle)
}

fn replacement(id: &str) -> &'static Replacement {
    REPLACEMENTS
        .iter()
        .find(|r| r.id == id)
        .unwrap_or_else(|| panic!("unit requires unknown replacement id `{id}`"))
}

/// Nothing on the deletion list may disappear while its unit is blocked, and the
/// failure names the blockers rather than only the rule.
#[test]
fn every_listed_path_survives_until_its_unit_is_ready() {
    let root = repo_root();
    let mut failures = String::new();

    for unit in UNITS {
        let unmet: Vec<&str> = unit
            .requires
            .iter()
            .filter(|id| !replacement(id).ready)
            .copied()
            .collect();
        if unmet.is_empty() {
            continue;
        }
        let mut missing = Vec::new();
        for rel in unit.paths {
            if !root.join(rel).exists() {
                missing.push((*rel).to_string());
            }
        }
        for (rel, needle) in unit.markers {
            if !root.join(rel).exists() || !source(&root, rel).contains(needle) {
                missing.push(format!("{needle} (in {rel})"));
            }
        }
        if missing.is_empty() {
            continue;
        }
        let _ = write!(
            failures,
            "\n{}: removed {} while these replacements do not exist yet: {}",
            unit.name,
            missing.join(", "),
            unmet.join(", "),
        );
    }

    assert!(
        failures.is_empty(),
        "v2 teardown deleted behaviour that has no replacement.\n\
         Each line names what went and why it may not go yet. If a replacement \
         now exists, update its row in tests/teardown_gate.rs and the worklist in \
         docs/PHASE6-TEARDOWN-READINESS.md in the same change.{failures}"
    );
}

/// A recorded verdict is only worth having if it cannot drift from the tree.
#[test]
fn recorded_verdicts_match_the_tree() {
    let root = repo_root();
    let mut failures = String::new();

    for r in REPLACEMENTS {
        let probed = (r.probe)(&root, r.id);
        if probed == r.ready {
            continue;
        }
        let _ = write!(
            failures,
            "\n{}: recorded {}, but the tree says {} — {} ({})",
            r.id,
            if r.ready { "ready" } else { "blocked" },
            if probed { "ready" } else { "blocked" },
            r.v1_capability,
            r.v2_home,
        );
    }

    assert!(
        failures.is_empty(),
        "the teardown inventory disagrees with the source tree.\n\
         A row that became ready unblocks part of the teardown: re-verdict it \
         here and revisit what it unblocks.{failures}"
    );
}

/// Whether deletion is permitted follows from the verdicts alone, so both
/// answers are checkable — today's, and a table where everything landed.
#[test]
fn readiness_is_derived_from_the_verdicts() {
    let blocked = blockers(REPLACEMENTS);
    assert!(
        !blocked.is_empty(),
        "every replacement is recorded ready — the teardown is unblocked, so \
         retire this gate deliberately rather than leaving it passing vacuously"
    );
    // The headline blocker: status reporting is still delivered by the installer
    // the teardown deletes.
    assert!(blocked.contains(&"hooks-in-kernel"));
    // No pane has a plugin to become the default, so no native pane may go.
    for r in REPLACEMENTS
        .iter()
        .filter(|r| r.v2_home.contains("bundled"))
    {
        assert!(blocked.contains(&r.id), "{} is recorded ready", r.id);
    }
    // A row recorded ready is not a blocker.
    assert!(!blocked.contains(&"self-heal"));

    // The other direction: a fully landed table permits deletion.
    let all_ready: Vec<Replacement> = REPLACEMENTS
        .iter()
        .map(|r| Replacement { ready: true, ..*r })
        .collect();
    assert!(blockers(&all_ready).is_empty());
}

/// A unit with no requirements would be an unguarded deletion, and a requirement
/// naming nothing would be a guard that never fires.
#[test]
fn inventory_is_well_formed() {
    for unit in UNITS {
        assert!(
            !unit.requires.is_empty(),
            "{} lists no requirements — an unguarded deletion",
            unit.name
        );
        assert!(
            !unit.paths.is_empty() || !unit.markers.is_empty(),
            "{} names nothing to protect",
            unit.name
        );
        for id in unit.requires {
            // Panics with the offending id when it resolves to no row.
            let _ = replacement(id);
        }
    }
    for r in REPLACEMENTS {
        assert!(
            UNITS.iter().any(|u| u.requires.contains(&r.id)),
            "replacement `{}` gates nothing — either a unit should require it, \
             or the row is dead",
            r.id
        );
    }
}
