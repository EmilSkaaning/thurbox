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
    /// For a pane row, the native renderer module whose continued use in
    /// `src/app/view.rs` means the pane has not been handed over. `None` for the
    /// capability rows, which have no pane.
    native_module: Option<&'static str>,
    /// For a pane a bundled plugin reproduces, the stem of the integration test
    /// that is that pane's equality oracle — `bundled_tasks_panel` for
    /// `tests/bundled_tasks_panel.rs` and `tests/snapshots/bundled_tasks_panel__*`.
    ///
    /// Named rather than derived because it is not derivable: `tasks-plugin`'s
    /// oracle is `bundled_tasks_panel`, `session-list-plugin`'s is
    /// `bundled_session_list`, and guessing across the spellings would turn a
    /// renamed test into a "file not found" instead of the verdict this row wants.
    /// `None` for the capability rows and for a pane recorded structurally
    /// unportable, which has no reproduction to constrain.
    oracle: Option<&'static str>,
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
/// may only be deleted once a plugin renders it **in the build a user installs**,
/// and today none does — six panes have a plugin that reproduces them, and none is
/// what the interface draws. Since Stage B (ADR-40) the runtime behind them is in
/// the default feature set, so what remains blocking is per-pane rather than
/// shared. See [`pane`] for the four conditions and which mistake each rules out.
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
        native_module: None,
        oracle: None,
    },
    Replacement {
        id: "agent-registration",
        v1_capability: "registering agents in agents.toml",
        v2_home: "plugin manifest `[[agents]]`",
        ready: false,
        probe: |root, _| manifest_field(root, "pub struct PluginManifest", "pub agents:"),
        native_module: None,
        oracle: None,
    },
    Replacement {
        id: "resource-seeding",
        v1_capability: "seeding sessions and automations",
        v2_home: "plugin manifest `[[automations]]` plus kernel-table host APIs",
        ready: false,
        probe: |root, _| manifest_field(root, "pub struct PluginManifest", "pub automations:"),
        native_module: None,
        oracle: None,
    },
    Replacement {
        id: "agent-config-files",
        v1_capability: "placing files in an agent's own config dir",
        v2_home: "a plugin `fs` capability",
        ready: false,
        // SECURITY §3 enforces filesystem denial by the *absence* of a binding,
        // so this row flips only on a deliberate capability decision.
        probe: |root, _| manifest_field(root, "pub enum Capability", "Fs,"),
        native_module: None,
        oracle: None,
    },
    Replacement {
        id: "spawn-arg-contribution",
        v1_capability: "patching agent args at spawn",
        v2_home: "spawn contributions carrying arguments, not only env",
        ready: false,
        probe: |root, _| manifest_field(root, "pub struct SpawnDecl", "pub args"),
        native_module: None,
        oracle: None,
    },
    Replacement {
        id: "self-heal",
        v1_capability: "self-heal of declared resources on startup and tick",
        v2_home: "idempotent by construction — discovery re-walks the manifests",
        ready: true,
        probe: |root, _| source(root, "src/plugin/discovery.rs").contains("pub fn discover("),
        native_module: None,
        oracle: None,
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
        native_module: None,
        oracle: None,
    },
    // One row per pane MIGRATION §2 schedules for the bundled-plugin phase. The
    // third argument is the pane's native renderer module, which is how the probe
    // below asks whether the handover happened; the fourth is the oracle that has
    // to hold a recording before the handover may take that module away.
    pane(
        "info-panel-plugin",
        "the info panel",
        "info_panel",
        Some("bundled_info_panel"),
    ),
    pane(
        "tasks-plugin",
        "the tasks pane",
        "tasks_panel",
        Some("bundled_tasks_panel"),
    ),
    pane(
        "automations-plugin",
        "the automations pane",
        "automations_panel",
        Some("bundled_automations_panel"),
    ),
    pane(
        "file-viewer-plugin",
        "the file viewer",
        "file_viewer",
        Some("bundled_file_viewer"),
    ),
    // Global search is recorded structurally unportable and has no bundled plugin,
    // so it has no reproduction for an oracle to constrain: condition 1 already
    // holds its row blocked, and naming an oracle would be inventing one.
    pane(
        "global-search-plugin",
        "global search",
        "global_search",
        None,
    ),
    pane(
        "code-review-plugin",
        "the code review view",
        "code_review",
        Some("bundled_code_review"),
    ),
    pane(
        "session-list-plugin",
        "the session list",
        "project_list",
        Some("bundled_session_list"),
    ),
];

/// A pane's replacement row, ready only on **handover**.
///
/// Four conditions, and only the first is "does a plugin exist":
///
/// 1. a bundled plugin directory named after the pane — the id minus its
///    `-plugin` suffix, in either spelling, since the eventual plugin's name is
///    not this gate's to fix;
/// 2. that `src/app/view.rs` no longer names the pane's native renderer module.
///    Every one of the seven renderers is referenced from that one file, so the
///    rule is uniform across the panes rather than seven special cases;
/// 3. that the runtime which draws a bundled pane reaches the build a user
///    installs; and
/// 4. that the pane's equality oracle holds a **recorded** expectation, and not
///    only a comparison against the native builder this deletion takes away.
///
/// Condition 2 rules out the loud mistake. A bundled plugin that *reproduces* a
/// pane while the native one is still what the interface draws has replaced
/// nothing, and a gate that called that ready would permit deleting the renderer
/// every user is looking at. That is not hypothetical: the info panel's plugin
/// exists today and this row is still blocked, which
/// `a_reproduced_pane_is_not_a_replaced_one` asserts.
///
/// Condition 3 rules out the quiet one, which the first two together permit. A
/// bundled pane is a Luau program, so a build without the VM draws it as an empty
/// column. While the runtime was an optional dependency that
/// `release/workflow-invariants` forbade the release workflow from enabling,
/// handing a pane to it would have removed that pane from every install while
/// `--features plugins` stayed green — the only build able to draw the
/// replacement was one no installer produced. Stage B (ADR-40) put the runtime in
/// the default feature set, so the condition now **holds**; it stays checked
/// rather than retired, because it records a release decision a later change
/// could reverse, and every pane already handed over would empty silently.
/// `the_build_condition_holds_and_still_gates_a_handover` pins both halves.
///
/// Condition 3 is a fact about the build rather than about any one pane, so all
/// seven rows carry it. That was the real dependency structure while it blocked:
/// one release decision unblocked them together, and stating it once stopped it
/// reading as seven independent pane problems.
///
/// Condition 4 (ADR-48) guards the **evidence**, and it is the only one whose
/// violation is invisible in a running binary: the pane looks right and is no
/// longer constrained. Each pane's oracle compares its plugin's view tree against
/// the native builder — `ui::tasks_panel::tasks_tree` and the like — which is in
/// the module the same deletion removes. With the right-hand side gone the repair
/// that compiles is to drop the comparison, leaving a test that the plugin renders
/// without erroring: satisfied equally by a pane drawing one wrong row and by one
/// drawing twenty. A recording taken while the native builder is present is what
/// survives it, and it cannot be produced afterwards, so the ordering is the
/// condition. `a_pane_whose_oracle_is_differential_is_not_handed_over` pins it,
/// and `every_reproduced_pane_records_its_native_tree` fails *now* rather than at
/// deletion time.
const fn pane(
    id: &'static str,
    v1_capability: &'static str,
    native_module: &'static str,
    oracle: Option<&'static str>,
) -> Replacement {
    Replacement {
        id,
        v1_capability,
        v2_home: "a bundled plugin under src/plugin/bundled/, drawn instead of the native pane",
        ready: false,
        probe: |root, id| {
            pane_is_handed_over(
                bundled_plugin_exists(root, id),
                view_draws_native_pane(root, id),
                plugin_host_reaches_the_installed_build(root),
                pane_oracle_records_the_native_tree(root, id),
            )
        },
        native_module: Some(native_module),
        oracle,
    }
}

/// Handover, from its four conditions.
///
/// Pure over the conditions rather than over the tree, because the interesting
/// cases cannot be exhibited by the tree the test runs against: a native renderer
/// that is no longer drawn is one someone already deleted. Mirrors [`blockers`],
/// which is pure over the table for the same reason.
fn pane_is_handed_over(
    plugin_exists: bool,
    native_still_drawn: bool,
    host_in_installed_build: bool,
    oracle_records: bool,
) -> bool {
    plugin_exists && !native_still_drawn && host_in_installed_build && oracle_records
}

/// Whether `id`'s pane has a **recorded** expectation, rather than only a
/// comparison against the builder its handover deletes.
///
/// Three facts, all read as source text so the answer is the same in either build
/// configuration (every pane oracle is `#![cfg(feature = "plugins")]` and does not
/// compile under `--no-default-features`, where this gate still runs):
///
/// * the row names an oracle at all — a pane with no reproduction has nothing to
///   record, and answers `false` because it is also not handed over;
/// * that oracle uses the shared recorder and asserts a snapshot with it; and
/// * at least one recording is checked in, so a test that calls
///   `insta::assert_snapshot!` and has never been run does not read as recorded.
///
/// A missing oracle file answers `false` rather than panicking: the loud report of
/// a deleted or renamed oracle belongs to
/// [`every_reproduced_pane_records_its_native_tree`], which names the pane, not to
/// a probe whose failure would read as "read tests/…: No such file".
fn pane_oracle_records_the_native_tree(root: &Path, id: &str) -> bool {
    let Some(stem) = REPLACEMENTS
        .iter()
        .find(|r| r.id == id)
        .and_then(|r| r.oracle)
    else {
        return false;
    };
    let Ok(text) = fs::read_to_string(root.join("tests").join(format!("{stem}.rs"))) else {
        return false;
    };
    let wired = text.contains("view_tree_record") && text.contains("insta::assert_snapshot!");
    let prefix = format!("{stem}__");
    let recorded = fs::read_dir(root.join("tests/snapshots")).is_ok_and(|dir| {
        dir.filter_map(Result::ok).any(|entry| {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            name.starts_with(&prefix) && name.ends_with(".snap")
        })
    });
    wired && recorded
}

/// Whether the runtime that draws a bundled pane is part of the build a user
/// installs.
///
/// Read from `Cargo.toml`'s default feature list, not from
/// `cfg!(feature = "plugins")`. The `cfg!` answers "was *this test binary* built
/// with the feature", which under `cargo nextest run --all --no-default-features`
/// is `false` and under a plain run is `true` — neither is a fact about what an
/// installer produces. Reading the manifest keeps one verdict in both
/// configurations, which is the property the module note above depends on.
fn plugin_host_reaches_the_installed_build(root: &Path) -> bool {
    let manifest = source(root, "Cargo.toml");
    let start = manifest
        .find("\n[features]\n")
        .unwrap_or_else(|| panic!("Cargo.toml no longer declares `[features]`"));
    let table = &manifest[start + 1..];
    // Bounded at the next table, so the `plugins = [...]` definition sitting
    // below the default list cannot be read as membership of it.
    let table = match table[1..].find("\n[") {
        Some(i) => &table[..i + 1],
        None => table,
    };
    let default = table
        .lines()
        .find(|line| line.trim_start().starts_with("default"))
        .unwrap_or_else(|| panic!("Cargo.toml's `[features]` declares no `default`"));
    // Quoted, so a feature whose name merely contains `plugins` is not read as
    // the gate itself.
    default.contains("\"plugins\"")
}

/// Whether a bundled plugin directory named after `id`'s pane exists.
fn bundled_plugin_exists(root: &Path, id: &str) -> bool {
    let stem = id.strip_suffix("-plugin").unwrap_or(id);
    [stem.to_string(), stem.replace('-', "_")]
        .iter()
        .any(|dir| {
            root.join("src/plugin/bundled")
                .join(dir)
                .join("plugin.toml")
                .exists()
        })
}

/// Whether the interface still draws `id`'s pane natively.
///
/// Resolved through the row rather than passed in, so the probe stays a plain
/// `fn` pointer and the module name lives in exactly one place — the table.
fn view_draws_native_pane(root: &Path, id: &str) -> bool {
    let module = REPLACEMENTS
        .iter()
        .find(|r| r.id == id)
        .and_then(|r| r.native_module)
        .unwrap_or_else(|| panic!("pane row `{id}` names no native renderer module"));
    source(root, "src/app/view.rs").contains(module)
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

/// The half of the pane probe that is easy to get wrong, asserted directly: a
/// pane whose plugin exists but whose native renderer is still drawn is **not**
/// ready.
///
/// This is not a restatement of the probe. It pins the *reason* the info panel's
/// row is blocked — a reproduction is not a replacement — so that "simplifying"
/// the probe back to "does a plugin directory exist" fails here with the
/// argument attached, rather than quietly permitting the deletion of the pane
/// every user is looking at.
#[test]
fn a_reproduced_pane_is_not_a_replaced_one() {
    let root = repo_root();
    assert!(
        bundled_plugin_exists(&root, "info-panel-plugin"),
        "the info panel's bundled plugin should exist — if it was removed, this \
         test no longer proves anything and should be revisited with it"
    );
    assert!(
        view_draws_native_pane(&root, "info-panel-plugin"),
        "the native info panel should still be what the interface draws"
    );
    assert!(
        !replacement("info-panel-plugin").ready,
        "a pane reproduced by a plugin, while the native one is still drawn, is \
         not handed over: deleting src/ui/info_panel.rs would remove what users see"
    );
    // And the recorded verdict agrees with the probe, which is the general rule
    // `recorded_verdicts_match_the_tree` enforces for every row.
    assert!(!(replacement("info-panel-plugin").probe)(
        &root,
        "info-panel-plugin"
    ));
}

/// The other half of the pane probe that is easy to get wrong: a pane handed to a
/// runtime the user does not have is **not** handed over — and that condition is
/// satisfied today, which is a fact worth asserting rather than assuming.
///
/// This is the quiet case, and the two-condition probe permitted it. Delete a
/// pane's native renderer plus its call in `src/app/view.rs`, and conditions 1
/// and 2 both hold: the row would be recorded ready,
/// `recorded_verdicts_match_the_tree` would *require* that, and
/// `every_listed_path_survives_until_its_unit_is_ready` would stop protecting the
/// renderer — signing off the deletion of a pane no released binary can draw.
///
/// Stage B (ADR-40) put the runtime in the default feature set, so the tree now
/// satisfies the condition. The rule is kept because it is what a change removing
/// the runtime from `default` — or a release built with default features
/// suppressed — would violate, at which point every handed-over pane becomes an
/// empty column with nothing failing. So this asserts the condition *holds*, and
/// separately that it still decides a handover.
#[test]
fn the_build_condition_holds_and_still_gates_a_handover() {
    let root = repo_root();
    assert!(
        plugin_host_reaches_the_installed_build(&root),
        "the plugin runtime is expected to be in Cargo.toml's default features \
         since Stage B, so that a bundled pane is drawn by the binary users \
         install. If it left the default list, that is a release decision to \
         revisit — with every pane already handed over — rather than a probe to \
         update"
    );
    // The case the tree can no longer exhibit, which is why the rule is pure:
    // plugin present, native renderer no longer drawn, runtime absent from the
    // build.
    assert!(
        !pane_is_handed_over(true, false, false, true),
        "a pane whose replacement only runs behind a compile-time feature the \
         released binary does not enable has not been handed over — deleting its \
         renderer would remove the pane from every install"
    );
    // And it is a handover once the runtime ships, which is the state the tree is
    // in: the condition gated a release decision rather than forbidding handover.
    assert!(pane_is_handed_over(true, false, true, true));

    // Consequence today: with the build condition met, each pane row is blocked
    // by its own pane-level reason — the interface still draws all seven native
    // renderers — so a handover is now pane work rather than a release decision.
    for r in REPLACEMENTS
        .iter()
        .filter(|r| r.v2_home.contains("bundled"))
    {
        assert!(
            view_draws_native_pane(&root, r.id),
            "{}: expected the native pane to still be drawn",
            r.id
        );
        assert!(
            !(r.probe)(&root, r.id),
            "{}: the native renderer is still drawn, so the row must be blocked",
            r.id
        );
    }
}

/// The fourth condition, asserted on the case the tree cannot exhibit: a pane
/// whose plugin is drawn instead of the native one, in a build that ships the
/// runtime, is **not** handed over while its oracle holds no recording.
///
/// This is the quiet case the other three conditions permit, and it is quieter
/// than condition 3's — that one empties a column, which someone eventually
/// notices; this one leaves the pane looking perfect and stops constraining it.
/// The pane oracles compare a plugin's tree against the native builder the
/// deletion removes, so the deletion takes the right-hand side with it and the
/// repair that compiles is to drop the assertion. A recording is only *provable*
/// while that builder exists, which is why the gate refuses the handover rather
/// than asking for the recording afterwards.
#[test]
fn a_pane_whose_oracle_is_differential_is_not_handed_over() {
    assert!(
        !pane_is_handed_over(true, false, true, false),
        "a pane whose oracle only compares against the builder being deleted has \
         no evidence that survives the deletion: the handover would leave a test \
         that the plugin renders without erroring, which one wrong row satisfies"
    );
    // And the condition is not a veto on handover: with the recording present the
    // same three conditions hand the pane over.
    assert!(pane_is_handed_over(true, false, true, true));
}

/// Every pane a bundled plugin reproduces records its native tree — asserted
/// positively, so a missing recording fails **now** rather than at deletion time.
///
/// The conjunct in [`pane`] only fires once someone also stops drawing the native
/// pane, which is the change that can no longer add the recording. This is the
/// half that is actionable: a pane reproduced today with no recording is a pane
/// whose evidence has to be captured while its builder is here (ADR-42, ADR-48).
#[test]
fn every_reproduced_pane_records_its_native_tree() {
    let root = repo_root();
    let mut failures = String::new();

    for r in REPLACEMENTS
        .iter()
        .filter(|r| r.v2_home.contains("bundled") && bundled_plugin_exists(&root, r.id))
    {
        let Some(stem) = r.oracle else {
            let _ = write!(
                failures,
                "\n{}: a bundled plugin reproduces this pane but the row names no \
                 oracle, so nothing checks that its expectation is recorded",
                r.id,
            );
            continue;
        };
        if !root.join("tests").join(format!("{stem}.rs")).exists() {
            let _ = write!(
                failures,
                "\n{}: names oracle `{stem}`, but tests/{stem}.rs does not exist — \
                 either the oracle was renamed (update the row) or it was deleted \
                 (the pane has no equality proof at all)",
                r.id,
            );
            continue;
        }
        if !pane_oracle_records_the_native_tree(&root, r.id) {
            let _ = write!(
                failures,
                "\n{}: tests/{stem}.rs does not record its native tree — it must \
                 use tests/view_tree_record and assert an insta snapshot per case, \
                 with the recordings checked in",
                r.id,
            );
        }
    }

    assert!(
        failures.is_empty(),
        "a reproduced pane's oracle is still purely differential.\n\
         Its right-hand side is the native builder a handover deletes, and the \
         recording that outlives it can only be generated while that builder is \
         present — so it is owed by the port, not by the handover.{failures}"
    );
}

/// Every pane row must name a native renderer, or its probe would panic the first
/// time the pane's plugin appeared — which is exactly when the gate matters.
#[test]
fn every_pane_row_names_its_native_renderer() {
    let root = repo_root();
    for r in REPLACEMENTS
        .iter()
        .filter(|r| r.v2_home.contains("bundled"))
    {
        let module = r
            .native_module
            .unwrap_or_else(|| panic!("{} names no native renderer module", r.id));
        assert!(
            source(&root, "src/app/view.rs").contains(module),
            "{}: src/app/view.rs no longer mentions `{module}` — either the pane \
             was handed over (re-verdict the row) or the module was renamed",
            r.id
        );
    }
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
