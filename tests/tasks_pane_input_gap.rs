//! The tasks pane's **input** verdict, enforced as a test.
//!
//! `docs/PHASE4-PANE-READINESS.md` §15 records that the tasks pane's *rendering*
//! is reproduced by a bundled plugin — including, since this change, its
//! scrolling — while its **keys** cannot be. This file is that half of the
//! verdict in executable form: [`BLOCKERS`] records one host power the pane's
//! keys need and do not have, per row, and re-derives each from the source.
//!
//! Why a test rather than only the document, in `tests/global_search_pane_gap.rs`'s
//! words: a verdict written in markdown is a fact about a build that expires
//! without telling anyone. "The tasks pane's keys cannot be ported" stops being
//! true the moment someone adds a view write for an unrelated reason, and nothing
//! would say so.
//!
//! ## Which route this gate measures, since ADR-51 there are two
//!
//! Every row below is about a pane whose **keys are the plugin's**: `input`, a
//! binding per chord (ADR-34), and a capability per effect. That was the only route
//! when this verdict was written, and each row is still a true statement about it —
//! nothing here has been granted.
//!
//! ADR-51 added a second: a pane may declare that it **is** thurbox's tasks pane,
//! and the kernel then resolves `KeyContext::Tasks` and performs those actions
//! itself while the pane holds focus, as `InputFocus::TaskList`. On that route the
//! host powers this table asks for are not needed at all — the kernel moves its own
//! cursor, opens its own editor, and opens its own picker — so a blocked row here
//! says "this pane's keys cannot be ported **to a plugin**", never "this pane cannot
//! be handed over". The one row untouched by the distinction is
//! `no-ellipsizing-clip`, which is about drawing.
//!
//! **The finding this gate exists to keep true** is the second row below, because
//! it is the one nobody predicted. The port was expected to fail on the pane's two
//! separate surfaces — the central-pane editor and the trigger-time action picker
//! — and it does. But it fails earlier, on `j`: a cursor is view state, and the
//! two keys that need no new host power (cycling a status, deleting a task, both
//! already expressible as record writes) still cannot name the row they would act
//! on, because a plugin holds the keys only while the *kernel* publishes no cursor.
//!
//! **How that row was later narrowed, without weakening it.** The automations pane's
//! port (ADR-41) showed that a plugin declaring `input` can keep a cursor **of its
//! own** across renders — a VM persists between calls — and act on the row *it*
//! drew, so "a plugin cannot name the row the user is looking at" is true of the
//! kernel's cursor and not of panes in general. The tasks verdict stands as written:
//! this pane's plugin declares no input and holds no cursor, so the kernel's is the
//! only one it could name, and its other three rows are untouched by the
//! distinction. What changed is the *scope* of the sentence, which the row's `stands`
//! now says, so nobody reads it as a wall the host has since walked through.
//!
//! Three things this gate is deliberately not:
//!
//! - it is **not** the teardown gate, which answers whether
//!   `src/ui/tasks_panel.rs` may be deleted — already no, for ADR-37's reason, and
//!   no either way. One table answering two questions produces failures that do
//!   not say which question moved;
//! - it is **not** a claim that the pane's rendering is inexpressible. The plugin
//!   builds the native pane's view tree and paints its frame when the pane
//!   scrolls (`tests/bundled_tasks_panel.rs`); one vocabulary row is left, and it
//!   is marked as such;
//! - it is **not** a copy of the global-search gate's verdict. That surface is not
//!   a pane at all; this one is a pane whose *keys* have nowhere to land.
//!
//! Its probes read the source the way a human auditor would, so this gate runs,
//! and means the same thing, with or without the `plugins` Cargo feature. The
//! helpers below duplicate that gate's, because an integration test cannot import
//! another one — the alternative is a shared crate for two readers, which is more
//! machinery than the duplication costs.

use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

/// Whether the pane model withholds a power **by design**, or merely has not
/// spelled it yet.
///
/// The same distinction the global-search gate draws, and it matters here for a
/// sharper reason: this pane's one remaining *vocabulary* row is about drawing,
/// and closing it would improve the reproduction while changing nothing about the
/// keys. So portability is a conjunction the structural rows dominate, which
/// [`the_verdict_is_derived_from_the_blockers`] asserts in both directions.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Gap {
    /// A power a plugin pane is not given, on purpose. Reversing it changes what
    /// a plugin *is*, so no further node or token closes it.
    Structural,
    /// Something today's catalogue cannot say and could.
    Vocabulary,
}

/// One host power the tasks pane needs and does not have.
#[derive(Clone, Copy)]
struct Blocker {
    /// Stable id, named in a failure.
    id: &'static str,
    /// The pane behaviour that needs it, named by its key where it has one.
    needs: &'static str,
    /// Where the host stands today.
    stands: &'static str,
    gap: Gap,
    /// The recorded verdict: is this still missing?
    blocked: bool,
    /// Re-derives that verdict from the source. `true` means "still missing", so
    /// a probe and its row read the same way round.
    probe: fn(&Path) -> bool,
}

/// Every power the pane's ten `KeyContext::Tasks` actions would need, structural
/// rows first.
///
/// Each probe is scoped to the *declaration* it reads rather than to a whole file,
/// copied from `tests/teardown_gate.rs` for its reason: an unrelated mention
/// elsewhere in a file must not flip a verdict.
const BLOCKERS: &[Blocker] = &[
    Blocker {
        id: "no-cursor-write",
        needs: "moving the pane's cursor (`j`/`k`), scrolling the central preview \
                (`PageUp`/`PageDown`), and switching to a task's related session (`o`)",
        stands: "no binding writes *view* state — a plugin may change records it was granted \
                 (ADR-35), and nothing it holds moves a cursor, takes focus, or switches the \
                 active session",
        gap: Gap::Structural,
        blocked: true,
        probe: |root| !a_view_write_binding_exists(root),
    },
    Blocker {
        id: "input-and-cursor-are-disjoint",
        needs: "acting on the row the user is looking at **through the kernel's cursor** — which \
                is what the two keys that need no new host power, `Space` and `d`, would do in \
                this pane's plugin, since it declares no input and so keeps no cursor of its own",
        stands: "a plugin receives keys only while one of its own panes holds focus, and the \
                 published task row is marked as the cursor's only while the *native* pane holds \
                 focus or a search preview moves it, so the two are never true at once. Narrower \
                 than it first read: the automations port showed that a pane declaring `input` \
                 can hold a cursor **of its own** across renders and act on that row, so this \
                 row is about the kernel's cursor rather than about panes in general",
        gap: Gap::Structural,
        blocked: true,
        probe: |root| {
            // Both halves, because either alone would be the wrong claim. The
            // first says the appearance is gated on the tasks focus; the second
            // that a plugin pane is a *different* focus. A probe that read only
            // the first would report the wall closed as soon as the gate's
            // spelling changed.
            let gated_on_the_tasks_focus =
                method_body(root, "src/app/mod.rs", "fn build_tasks_snapshot")
                    .contains("matches!(self.focus, InputFocus::TaskList)");
            let a_plugin_pane_is_its_own_focus =
                variant_names(&block(root, "src/app/mod.rs", "pub enum InputFocus"))
                    .iter()
                    .any(|v| v == "PluginPane");
            gated_on_the_tasks_focus && a_plugin_pane_is_its_own_focus
        },
    },
    Blocker {
        id: "no-record-creation",
        needs: "creating a task (`n`) and authoring its title and description (`e`/`Enter`)",
        stands: "the write seam's operations are a closed list that changes existing records by \
                 id; nothing creates one, and nothing writes a record's text",
        gap: Gap::Structural,
        blocked: true,
        probe: |root| {
            // Signatures only, for `no-agent-reach`'s reason: the trait's prose
            // explains that a task's title is authored in thurbox's own editor,
            // and a probe reading the prose would find the word it forbids.
            let methods = writer_methods(root);
            let names_none_of = |needles: &[&str]| {
                !methods
                    .iter()
                    .any(|m| needles.iter().any(|n| m.contains(n)))
            };
            // A creation would be `create`/`insert`/`add`/`new`; a text write
            // would take a `title`, a `description` or a `body`.
            names_none_of(&["create", "insert", "add_", "new_"])
                && names_none_of(&["title", "description", "body"])
        },
    },
    Blocker {
        id: "no-central-seat",
        needs: "the central pane the task editor is drawn into (`n`, `e`/`Enter`)",
        stands: "**closed as a seat** (ADR-46). `PaneSlot::Center` names `RegionId::Center` and \
                 `App::render_central_pane` stands down for a pane that claims it, so a plugin \
                 may declare its list in one slot and its editor in the centre. What the editor \
                 still lacks is what to write when it is typed into — no operation authors a \
                 task's text (`no-record-creation`) — and the kernel's central chrome is not \
                 drawn over a plugin-owned centre",
        gap: Gap::Structural,
        blocked: false,
        probe: |root| {
            let seat_exists = method_body(root, "src/session/plugin_manifest.rs", "pub fn seat(")
                .contains("Some(RegionId::Center)");
            let native_stands_down = method_body(root, "src/app/view.rs", "fn render_central_pane")
                .contains("seat_taken(PaneSlot::Center)");
            !(seat_exists && native_stands_down)
        },
    },
    Blocker {
        id: "no-modal-surface",
        needs: "the trigger-time action picker (`r`), a modal that captures input ahead of every \
                keybinding while it is up",
        stands: "a manifest declares panes, commands, keybindings, a service, CLI verbs and spawn \
                 environment — nothing that owns the interface's input",
        gap: Gap::Structural,
        blocked: true,
        probe: |root| {
            let manifest = block(
                root,
                "src/session/plugin_manifest.rs",
                "pub struct PluginManifest",
            )
            .to_lowercase();
            !["modal", "overlay", "dialog", "picker"]
                .iter()
                .any(|n| manifest.contains(n))
        },
    },
    Blocker {
        id: "no-agent-reach",
        needs: "the picker's two outcomes (`r`): typing a prompt into a running session, or \
                spawning one for the task",
        stands:
            "the write seam names five operations over tasks and automations, and even running \
                 an automation is a *request* the kernel fulfils. `Capability::Spawn` adds \
                 environment to spawns thurbox already makes; it is not the power to start one",
        gap: Gap::Structural,
        blocked: true,
        probe: |root| {
            // Signatures only, not the whole body: the trait's prose says plainly
            // that a plugin thread never "spawns a process, or opens a session",
            // and a probe fooled by the sentence stating the rule would report the
            // rule broken.
            let reaches_a_session = writer_methods(root).iter().any(|m| {
                ["session", "prompt", "spawn", "send"]
                    .iter()
                    .any(|n| m.contains(n))
            });
            let bound = |needle: &str| {
                module_bindings(root)
                    .iter()
                    .any(|b| b.to_lowercase().contains(needle))
            };
            !reaches_a_session && !bound("prompt") && !bound("spawnsession") && !bound("sendkeys")
        },
    },
    Blocker {
        id: "no-ellipsizing-clip",
        needs: "fitting a title too wide for the column, with an ellipsis and room kept for the \
                trailing link marker",
        stands: "no node clips: a `line` clips at the pane edge without an ellipsis, and the \
                 renderer that paints a plugin's tree never fits a run to a width",
        gap: Gap::Vocabulary,
        blocked: true,
        probe: |root| {
            let kinds = view_node_kinds(root);
            let no_clipping_node = !kinds
                .iter()
                .any(|k| k == "Clip" || k == "Ellipsis" || k == "Truncate");
            // And the renderer does not fit a run itself — the kernel's helper is
            // `ui::truncate_ellipsis`, which the tree painter never calls.
            let renderer_fits_nothing =
                !source(root, "src/ui/plugin_pane.rs").contains("truncate_ellipsis");
            no_clipping_node && renderer_fits_nothing
        },
    },
];

/// Whether the pane's **key** surface could be ported at all: every blocker
/// closed.
///
/// Pure over the table so both answers are testable — today's, and a tree where
/// each row landed.
fn keys_are_portable(blockers: &[Blocker]) -> bool {
    blockers.iter().all(|b| !b.blocked)
}

/// The structural rows still blocking — the ones no widening of the catalogue
/// reaches.
fn structural_blockers(blockers: &[Blocker]) -> Vec<&'static str> {
    blockers
        .iter()
        .filter(|b| b.blocked && b.gap == Gap::Structural)
        .map(|b| b.id)
        .collect()
}

/// Whether any granted binding writes **view** state, as opposed to a record.
///
/// The distinction ADR-35 forced on the global-search gate, written in from the
/// start here: `setTaskStatus` is write-shaped and is *not* the write these keys
/// need. A **view verb** is a view write whatever it is applied to — there is no
/// record you `focus` or `jump` to. A **generic mutator** counts only when it
/// names a view noun, so `setTaskStatus` passes and a future `setActiveSession`
/// does not.
fn a_view_write_binding_exists(root: &Path) -> bool {
    const VIEW_VERBS: [&str; 6] = ["focus", "jump", "goto", "reveal", "scroll", "select"];
    const MUTATORS: [&str; 4] = ["set", "move", "open", "write"];
    const VIEW_NOUNS: [&str; 8] = [
        "Focus",
        "Cursor",
        "Selection",
        "Selected",
        "Row",
        "Pane",
        "Panel",
        "Active",
    ];
    // Matched as a *verb* in the host's camelCase names, so a reader that merely
    // begins with the same letters (`settings`) is not caught.
    let verb_applies = |binding: &str, verb: &str| -> Option<String> {
        binding
            .strip_prefix(verb)
            .filter(|rest| rest.is_empty() || rest.starts_with(char::is_uppercase))
            .map(str::to_string)
    };
    module_bindings(root).iter().any(|binding| {
        let view_verb = VIEW_VERBS
            .iter()
            .any(|verb| verb_applies(binding, verb).is_some());
        let view_write = MUTATORS.iter().any(|verb| {
            verb_applies(binding, verb)
                .is_some_and(|rest| VIEW_NOUNS.iter().any(|noun| rest.starts_with(noun)))
        });
        view_verb || view_write
    })
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Read a source file, panicking if it is unreadable — a probe that silently read
/// nothing would report a gap as closed for the wrong reason.
fn source(root: &Path, rel: &str) -> String {
    let path = root.join(rel);
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

/// The body of the top-level `struct`/`enum` whose declaration starts with
/// `header`.
///
/// rustfmt closes a top-level item with a `}` in column zero, which is the
/// terminator used here.
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

/// The body of a method — an item **inside** an `impl`, which [`block`] cannot
/// scope because its close is indented rather than in column zero.
///
/// Without this the probe would read the whole `impl App`, and a mention of the
/// gate's spelling anywhere in it would answer for the one function that matters.
fn method_body(root: &Path, rel: &str, header: &str) -> String {
    let text = source(root, rel);
    let start = text
        .find(header)
        .unwrap_or_else(|| panic!("{rel} no longer declares `{header}`"));
    let rest = &text[start..];
    let end = rest
        .find("\n    }\n")
        .unwrap_or_else(|| panic!("{rel}: `{header}` has no method-level close"));
    rest[..end].to_string()
}

/// The variant names declared in an item body, payloads and attributes ignored.
///
/// A variant is a line at exactly one level of indentation starting with an
/// uppercase letter, which is what rustfmt produces for every enum in the tree.
fn variant_names(body: &str) -> Vec<String> {
    body.lines()
        .filter_map(|line| {
            let unindented = line.strip_prefix("    ")?;
            if unindented.starts_with(' ') || !unindented.starts_with(char::is_uppercase) {
                return None;
            }
            let name: String = unindented
                .chars()
                .take_while(|c| c.is_alphanumeric() || *c == '_')
                .collect();
            (!name.is_empty()).then_some(name)
        })
        .collect()
}

/// The node kinds the view tree defines.
fn view_node_kinds(root: &Path) -> Vec<String> {
    variant_names(&block(
        root,
        "src/session/view_tree.rs",
        "pub enum ViewNode",
    ))
}

/// Every name inserted into a plugin's `@thurbox` module table.
///
/// The granted surface is exactly this list — enforcement in the host is by
/// absence, so what a plugin holds is what was `set` on the module.
fn module_bindings(root: &Path) -> Vec<String> {
    let text = source(root, "src/plugin/capabilities.rs");
    let mut names = Vec::new();
    for (at, matched) in text.match_indices("module.set(\"") {
        let tail = &text[at + matched.len()..];
        if let Some(name) = tail.split('"').next() {
            names.push(name.to_string());
        }
    }
    assert!(
        names.iter().any(|n| n == "ui"),
        "no module bindings found — the probe's anchor (`module.set(\"…\"`) changed shape, so \
         every verdict derived from it is meaningless"
    );
    names
}

/// The **signatures** of the write seam's methods, lowercased.
///
/// A signature rather than the whole trait body, because the trait's own
/// documentation names the powers it withholds ("spawns a process, or opens a
/// session"; a task's title being authored in thurbox's editor) — so a probe over
/// the prose would read the sentence stating a rule as the rule being broken. The
/// closed operation list is what decides these rows, and it is exactly these
/// lines.
fn writer_methods(root: &Path) -> Vec<String> {
    let body = block(
        root,
        "src/session/plugin_mutations.rs",
        "pub trait KernelWriter",
    );
    let methods: Vec<String> = body
        .lines()
        .map(str::trim)
        .filter(|line| line.starts_with("fn "))
        .map(|line| line.to_lowercase())
        .collect();
    assert!(
        methods.iter().any(|m| m.contains("set_task_status")),
        "no writer signatures found — the probe's anchor changed shape, so every verdict derived \
         from it is meaningless"
    );
    methods
}

/// A recorded verdict is only worth having if it cannot drift from the tree.
#[test]
fn recorded_blockers_match_the_tree() {
    let root = repo_root();
    let mut failures = String::new();

    for b in BLOCKERS {
        let probed = (b.probe)(&root);
        if probed == b.blocked {
            continue;
        }
        let _ = write!(
            failures,
            "\n{}: recorded {}, but the tree says {} — the pane needs {} ({})",
            b.id,
            if b.blocked { "missing" } else { "available" },
            if probed { "missing" } else { "available" },
            b.needs,
            b.stands,
        );
    }

    assert!(
        failures.is_empty(),
        "the tasks pane's input verdict disagrees with the source tree.\n\
         A row that became available may unblock part of the pane's key surface: re-verdict it \
         here and revisit docs/PHASE4-PANE-READINESS.md §15 in the same change.{failures}"
    );
}

/// Whether the keys are portable follows from the rows alone, so all three
/// answers are checkable: today's, a tree where only the vocabulary row landed,
/// and one where everything did.
#[test]
fn the_verdict_is_derived_from_the_blockers() {
    assert!(
        !keys_are_portable(BLOCKERS),
        "every blocker is recorded closed — the pane's keys may now be portable, so retire this \
         gate deliberately (and port them) rather than leaving it passing vacuously"
    );

    // The load-bearing half: the reason is structural. Closing the one vocabulary
    // row would improve the *reproduction* — a fitted title — and change nothing
    // about a single key.
    let structural = structural_blockers(BLOCKERS);
    assert!(
        structural.len() >= 5,
        "the recorded reason is supposed to be structural, but only {structural:?} are"
    );
    let vocabulary_closed: Vec<Blocker> = BLOCKERS
        .iter()
        .map(|b| Blocker {
            blocked: b.blocked && b.gap == Gap::Structural,
            ..*b
        })
        .collect();
    assert!(
        !keys_are_portable(&vocabulary_closed),
        "closing every vocabulary gap must not read as portability"
    );

    // And the other direction: a tree where every row landed permits the port, so
    // the gate gates rather than forbids.
    let all_closed: Vec<Blocker> = BLOCKERS
        .iter()
        .map(|b| Blocker {
            blocked: false,
            ..*b
        })
        .collect();
    assert!(keys_are_portable(&all_closed));
}

/// The correction ADR-35 forced on the global-search gate, asserted here from the
/// start: a plugin **can** change a task, and that is not the write these keys
/// need.
///
/// Without this the first row could be "simplified" to "is there any write-shaped
/// binding", which `setTaskStatus` already satisfies — reporting the pane's keys
/// portable while nothing can move a cursor.
#[test]
fn a_record_write_is_not_the_view_write_these_keys_need() {
    let root = repo_root();
    let bindings = module_bindings(&root);
    for record_write in ["setTaskStatus", "deleteTask"] {
        assert!(
            bindings.iter().any(|b| b == record_write),
            "`{record_write}` should be a granted binding — if it went, this test no longer \
             proves anything and should be revisited with it"
        );
    }
    assert!(
        !a_view_write_binding_exists(&root),
        "a binding that writes view state appeared: the pane's cursor may now be movable, so \
         re-verdict `no-cursor-write` rather than relaxing this test"
    );
}

/// The finding, asserted directly rather than only through its row: the two keys
/// that need no new host power still cannot name a row.
///
/// This is not a restatement of the probe. It pins the *reason* — the input path
/// and the cursor path are disjoint — so that a change publishing the cursor's
/// appearance to a focused plugin pane fails here with the argument attached,
/// rather than quietly making a key act on a row no user can see.
#[test]
fn the_two_expressible_keys_still_cannot_name_a_row() {
    let root = repo_root();
    let snapshot = method_body(&root, "src/app/mod.rs", "fn build_tasks_snapshot");
    assert!(
        snapshot.contains("matches!(self.focus, InputFocus::TaskList)"),
        "the published row's `selected` flag is expected to be gated on the *tasks* pane holding \
         focus; if that changed, the disjointness this pane's verdict rests on may have gone"
    );
    assert!(
        variant_names(&block(&root, "src/app/mod.rs", "pub enum InputFocus"))
            .iter()
            .any(|v| v == "PluginPane"),
        "a focused plugin pane is expected to be its own focus"
    );
    // The anchor is a different fact and *is* published unconditionally, which is
    // what makes the plugin's copy scroll. Asserted so the two are not conflated:
    // closing the appearance gap is not the same as having an anchor.
    assert!(
        snapshot.contains("cursor"),
        "the scroll anchor should be published regardless of focus — the half of this that did \
         land"
    );
}

/// The verdict and the shipped plugin must not disagree: a pane that declared
/// keys it could not act with would be the failure this gate records.
#[test]
fn the_bundled_plugin_takes_no_keys() {
    let root = repo_root();
    let manifest = source(&root, "src/plugin/bundled/tasks/plugin.toml");
    assert!(
        !manifest.contains("input"),
        "the bundled tasks plugin must not declare the input capability while its keys have \
         nowhere to land: {manifest}"
    );
    assert!(
        !manifest.contains("[[keybindings]]"),
        "the bundled tasks plugin must declare no keybinding: {manifest}"
    );
}

/// The pane is reproduced, not replaced — so the thing a user presses `j` in is
/// still the native renderer, and the teardown inventory's answer for it is
/// unchanged.
#[test]
fn the_native_pane_is_still_what_thurbox_draws() {
    let root = repo_root();
    assert!(
        root.join("src/ui/tasks_panel.rs").is_file(),
        "the native tasks pane's renderer must still exist"
    );
    assert!(
        source(&root, "src/app/view.rs").contains("tasks_panel"),
        "the interface must still draw the native tasks pane: a plugin that cannot take the \
         pane's keys cannot replace it"
    );
}
