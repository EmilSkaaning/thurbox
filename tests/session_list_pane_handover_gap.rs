//! The session list's **handover** verdict, enforced as a test.
//!
//! `docs/PHASE4-PANE-READINESS.md` §13 records that this pane's *rendering* is
//! reproduced by a bundled plugin — the port ADR-V1 hinges on, which needed no new
//! node kind, no new style token and no new capability. §18 records the next
//! step's answer: the pane is **not** handed over, and this file is that half of
//! the verdict in executable form. [`BLOCKERS`] holds one row per requirement the
//! handover could not meet, each re-derived from the source.
//!
//! Why a test rather than only the document, in `tests/global_search_pane_gap.rs`'s
//! words: a verdict written in markdown is a fact about a build that expires
//! without telling anyone. "The session list cannot be handed over" stops being
//! true the moment someone adds a view write for an unrelated reason, and nothing
//! would say so.
//!
//! **The finding this gate exists to keep true** is [`the_panes_scoped_keys_stop_
//! resolving_when_a_plugin_pane_holds_focus`], because it is the one the spike
//! could not have seen. `docs/SPIKE-SESSION-LIST.md` measured whether the pane
//! *could* be a plugin and answered yes on three conditions, the second being
//! that the cursor stays kernel state — which is right, and is precisely what
//! makes the handover impossible. A handed-over pane is focused as
//! `InputFocus::PluginPane`, which `App::focus_key_context` maps to
//! `KeyContext::Global`, so all six `KeyContext::SessionList` actions stop
//! resolving; and a plugin cannot substitute for them, because `j`/`k` move the
//! **active session** — what the central pane, the info panel, the file viewer and
//! the code review are all showing — and no capability writes kernel view state.
//! The cursor cannot be kernel state *and* be driven by a plugin pane's keys.
//!
//! Three things this gate is deliberately not:
//!
//! - it is **not** the teardown gate, which answers whether
//!   `src/ui/project_list.rs` may be deleted — already no, and no either way. One
//!   table answering two questions produces failures that do not say which
//!   question moved;
//! - it is **not** a claim that the pane's rendering is inexpressible. The plugin
//!   builds the native pane's view tree, spinner included, and since
//!   `left-column-pane-oracles` that claim is recorded rather than differential
//!   (`tests/bundled_session_list.rs`);
//! - it is **not** a copy of the tasks pane's gate. That pane's keys act on
//!   *records*, so five of them became expressible when the write seam landed.
//!   This pane's keys act on the interface itself.
//!
//! Its probes read the source the way a human auditor would, so the gate runs, and
//! means the same thing, with or without the `plugins` Cargo feature. The helpers
//! below duplicate the sibling gates', because an integration test cannot import
//! another one — the alternative is a shared crate for four readers, which is more
//! machinery than the duplication costs, and two readers of source text that drift
//! answer independently anyway (unlike the *recorder* in
//! `tests/view_tree_record/`, which is shared for exactly that reason).

use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

/// Why a requirement is unmet — which decides the *order* the work would be done
/// in, not only that it is outstanding.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Gap {
    /// A power a plugin is not given, on purpose. Reversing it changes what a
    /// plugin *is*, so no node and no wiring closes it.
    Structural,
    /// Something today's drawing catalogue cannot say and could.
    Vocabulary,
    /// Something the host could do today with no new plugin-facing concept: when a
    /// plugin is asked to render, or which facts it is told. Cheapest to close, and
    /// filed apart from the other two because calling it structural would claim
    /// the model forbids it — it does not.
    Wiring,
}

/// One requirement the handover needs and does not have.
#[derive(Clone, Copy)]
struct Blocker {
    /// Stable id, named in a failure.
    id: &'static str,
    /// The pane behaviour that needs it, by its key where it has one.
    needs: &'static str,
    /// Where the host stands today.
    stands: &'static str,
    gap: Gap,
    /// The recorded verdict: is this still missing?
    blocked: bool,
    /// Re-derives that verdict from the source. `true` means "still missing", so a
    /// probe and its row read the same way round.
    probe: fn(&Path) -> bool,
}

/// Every requirement the handover of this pane needs, structural rows first.
///
/// Each probe is scoped to the *declaration* it reads rather than to a whole file,
/// copied from `tests/teardown_gate.rs` for its reason: an unrelated mention
/// elsewhere in a file must not flip a verdict.
const BLOCKERS: &[Blocker] = &[
    Blocker {
        id: "scoped-keys-silenced-by-the-handover",
        needs: "the pane's own keyboard — all six `KeyContext::SessionList` actions (next, \
                previous, open, move down, move up, sort A→Z), rebindable in the F1 editor",
        stands: "a handed-over pane is focused as `InputFocus::PluginPane`, and \
                 `App::focus_key_context` names no arm for it, so it falls to `KeyContext::Global` \
                 — the pane's scope never activates and none of its six actions resolves. A \
                 plugin may declare pane-addressed bindings of its own (ADR-34), so this is only \
                 survivable for keys whose whole effect a plugin can also perform; the rows below \
                 are why none of these six is",
        gap: Gap::Structural,
        blocked: true,
        probe: |root| {
            // Three halves, because any one alone would be the wrong claim: the
            // focus exists, the scope resolver ignores it, and there is a scope
            // that would be silenced.
            let plugin_pane_is_its_own_focus =
                variant_names(&block(root, "src/app/mod.rs", "pub enum InputFocus"))
                    .iter()
                    .any(|v| v == "PluginPane");
            let resolver = method_body(
                root,
                "src/app/key_handlers.rs",
                "pub(crate) fn focus_key_context",
            );
            let falls_through_to_global =
                !resolver.contains("PluginPane") && resolver.contains("_ => KeyContext::Global");
            plugin_pane_is_its_own_focus
                && falls_through_to_global
                && !session_list_actions(root).is_empty()
        },
    },
    Blocker {
        id: "no-active-session-write",
        needs: "`j`/`k`/`Enter`, which move the **active session** — and so change what the \
                central pane, the info panel, the file viewer and the code review are all showing",
        stands: "no binding writes *view* state. A plugin may change records it was granted \
                 (ADR-35); nothing it holds moves a cursor, takes focus, or switches the active \
                 session. This is the widest grant the model has refused, because the thing being \
                 written decides what the whole interface displays",
        gap: Gap::Structural,
        blocked: true,
        probe: |root| !a_view_write_binding_exists(root),
    },
    Blocker {
        id: "no-session-record-write",
        needs: "`Shift+J`/`Shift+K`, which renumber `sessions.display_order` densely and persist \
                it, and `Shift+S`, which sorts every session within its repo group in one \
                keystroke",
        stands:
            "the write seam's five operations each address a task or an automation by id; none \
                 addresses a session, and none reorders anything. Both keys are the right *shape* \
                 for it — one operation per single-keystroke effect, ADR-35's rule — so the \
                 missing piece is an operation and not a principle. It is recorded rather than \
                 added: the key would still act on the row the user is looking at, which for this \
                 pane is the kernel's cursor, so the grant would widen a plugin's reach over the \
                 database while the pane it exists for still could not use it",
        gap: Gap::Structural,
        blocked: true,
        probe: |root| {
            let methods = writer_methods(root);
            !methods.iter().any(|m| {
                ["session", "order", "reorder", "sort", "move"]
                    .iter()
                    .any(|n| m.contains(n))
            })
        },
    },
    Blocker {
        id: "no-left-seat",
        needs: "the pane's seat: the session list **is** the left column, above the automations \
                pane",
        stands: "`PaneSlot` is a closed set whose only member is the right-hand column, so the \
                 reproduction is placeable as a pane and not placeable where this pane is. \
                 `docs/PHASE4-PANE-READINESS.md` §17 tabulates the four things a `left` slot \
                 needs, of which the load-bearing one is a height policy: the left column is the \
                 one place in thurbox where a pane's geometry is derived from its own content",
        gap: Gap::Structural,
        blocked: true,
        probe: |root| {
            variant_names(&block(
                root,
                "src/session/plugin_manifest.rs",
                "pub enum PaneSlot",
            )) == ["Right"]
        },
    },
    Blocker {
        id: "the-module-is-the-kernels-model",
        needs: "deleting `src/ui/project_list.rs`, which is what a handover is for",
        stands: "that module is not only the pane's renderer. It owns \
                 `compute_session_order` (the comparator `App`'s `Ctrl+J`/`Ctrl+K` navigate by), \
                 `move_in_order`, `sort_alphabetically_within_groups`, `resolve_rows` — which \
                 builds the very snapshot the *plugin* reads — and `SessionMatch`, global \
                 search's session matcher. Deleting it deletes navigation, reordering, sorting \
                 and search. The file viewer's gate found the same class (ADR-39); this pane's \
                 case is larger",
        gap: Gap::Structural,
        blocked: true,
        probe: |root| {
            let app = source(root, "src/app/mod.rs");
            [
                "project_list::compute_session_order",
                "project_list::move_in_order",
                "project_list::sort_alphabetically_within_groups",
                "project_list::resolve_rows",
            ]
            .iter()
            .all(|needle| app.contains(needle))
        },
    },
    Blocker {
        id: "render-is-not-event-driven",
        needs: "the highlight moving in the frame the key was handled — the spike's fourth bar, \
                5 ms of added latency on a selection change",
        stands: "the plugin worker re-renders every pane, then waits out a fixed interval \
                 (`PLUGIN_RENDER_SLICE` × `PLUGIN_RENDER_SLICES` = 1 s) serving keys; nothing \
                 tells it that kernel state moved. For a hidden reproduction that is a \
                 reproduction trailing by up to a second, which §13 recorded as tolerable. For \
                 the pane a user navigates with it is the pane itself trailing, so the handover \
                 inverts that argument",
        gap: Gap::Wiring,
        blocked: true,
        probe: |root| {
            let main = source(root, "src/main.rs");
            let fixed_wait = main.contains("for _ in 0..PLUGIN_RENDER_SLICES");
            // A nudge would have to arrive on a channel the worker selects on, so
            // it would be named here. None of these words appears in the file.
            let no_nudge = ![
                "render_now",
                "state_changed",
                "snapshot_changed",
                "wake_render",
            ]
            .iter()
            .any(|n| main.contains(n));
            fixed_wait && no_nudge
        },
    },
    Blocker {
        id: "no-pane-chrome",
        needs: "the pane's border, which carries one status dot per session (right-aligned in the \
                block's top title) and `^ N` / `v N` indicators when rows are clipped",
        stands: "the host draws a plugin pane's block around whatever the plugin returned, and \
                 nothing in the catalogue describes an overlay on that frame. This is §9's \
                 frame-node row, at its fourth consumer",
        gap: Gap::Vocabulary,
        blocked: true,
        probe: |root| {
            // The native pane puts the dots on the block, outside any tree.
            let native_draws_on_the_border =
                source(root, "src/ui/project_list.rs").contains("block.title_top(");
            let manifest = block(
                root,
                "src/session/plugin_manifest.rs",
                "pub struct PaneDecl",
            )
            .to_lowercase();
            let nothing_declares_chrome = !["border", "chrome", "badge", "indicator"]
                .iter()
                .any(|n| manifest.contains(n));
            native_draws_on_the_border && nothing_declares_chrome
        },
    },
    Blocker {
        id: "no-centred-line",
        needs: "the empty state — `No sessions yet` and `Press Ctrl+N to create one`, drawn \
                centred",
        stands: "every node draws from the left; `Fill` can push a run flush right and nothing \
                 centres. The one enumerated divergence of the port's oracle, and the plugin \
                 draws the same words left-aligned",
        gap: Gap::Vocabulary,
        blocked: true,
        probe: |root| {
            let kinds = view_node_kinds(root);
            let no_centring_node = !kinds
                .iter()
                .any(|k| k == "Center" || k == "Centre" || k == "Align");
            let style_has_no_alignment =
                !block(root, "src/session/view_tree.rs", "pub struct TextStyle").contains("align");
            no_centring_node && style_has_no_alignment
        },
    },
    Blocker {
        id: "no-pending-spawn-row",
        needs: "the placeholder row a spawning session renders as, inside the repo group it will \
                land in — the whole non-blocking new-session flow's only progress surface \
                (ADR-P12)",
        stands: "the published session row carries a name, a status, a group, a depth and four \
                 flags; nothing says a row is a spawn in flight, and the slot it lands in is \
                 `ui::project_list::pending_spawn_slot` over `App::pending_spawn`. A publication \
                 the kernel could make, so it is filed as vocabulary rather than as a wall",
        gap: Gap::Vocabulary,
        blocked: true,
        probe: |root| {
            let row = block(
                root,
                "src/session/pane_context.rs",
                "pub struct SessionRowSnapshot",
            )
            .to_lowercase();
            let publishes_no_pending = !row.contains("pending") && !row.contains("placeholder");
            let native_owns_the_slot =
                source(root, "src/ui/project_list.rs").contains("pub fn pending_spawn_slot");
            publishes_no_pending && native_owns_the_slot
        },
    },
];

/// Whether the pane could be handed over at all: every requirement met.
///
/// Pure over the table so both answers are testable — today's, and a tree where
/// each row landed.
fn handover_is_possible(blockers: &[Blocker]) -> bool {
    blockers.iter().all(|b| !b.blocked)
}

/// The rows outstanding of one kind — which is how the ordering of the work is
/// read off the table rather than argued in prose.
fn outstanding(blockers: &[Blocker], gap: Gap) -> Vec<&'static str> {
    blockers
        .iter()
        .filter(|b| b.blocked && b.gap == gap)
        .map(|b| b.id)
        .collect()
}

/// Whether any granted binding writes **view** state, as opposed to a record.
///
/// Copied from `tests/tasks_pane_input_gap.rs`, where ADR-35 forced the
/// distinction: `setTaskStatus` is write-shaped and is *not* the write these keys
/// need. A **view verb** is a view write whatever it is applied to — there is no
/// record you `focus` or `jump` to. A **generic mutator** counts only when it names
/// a view noun, so `setTaskStatus` passes and a future `setActiveSession` does not.
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

/// The `Action` names scoped to the session list — the keyboard a handover would
/// silence.
fn session_list_actions(root: &Path) -> Vec<String> {
    let body = method_body(root, "src/session/keybindings.rs", "pub fn context(self)");
    let arm = body
        .find("KeyContext::SessionList")
        .map(|end| &body[..end])
        .unwrap_or_else(|| {
            panic!("`Action::context` no longer scopes anything to the session list")
        });
    arm.split("Action::")
        .skip(1)
        .map(|rest| {
            rest.chars()
                .take_while(|c| c.is_alphanumeric() || *c == '_')
                .collect()
        })
        .collect()
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
/// `header`. rustfmt closes a top-level item with a `}` in column zero.
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
/// Without this the probe would read the whole `impl`, and a mention of the gate's
/// spelling anywhere in it would answer for the one function that matters.
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

/// Every name inserted into a plugin's `@thurbox` module table. The granted
/// surface is exactly this list — enforcement in the host is by absence.
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
/// documentation names the powers it withholds — so a probe over the prose would
/// read the sentence stating a rule as the rule being broken.
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
            "\n{}: recorded {}, but the tree says {} — needs {} ({})",
            b.id,
            if b.blocked { "missing" } else { "present" },
            if probed { "missing" } else { "present" },
            b.needs,
            b.stands,
        );
    }

    assert!(
        failures.is_empty(),
        "the session list's handover verdict disagrees with the source tree.\n\
         A row that closed changes what a handover would cost: re-verdict it here, revisit \
         docs/PHASE4-PANE-READINESS.md §18 and ADR-43, and check whether the pane is now \
         portable.{failures}"
    );
}

/// The verdict follows from the rows, so both answers are checkable — today's, and
/// a table where everything landed.
#[test]
fn the_verdict_is_derived_from_the_blockers() {
    assert!(
        !handover_is_possible(BLOCKERS),
        "every requirement is recorded met — the session list is portable, so hand it over \
         deliberately (and retire this gate) rather than leaving it passing vacuously"
    );
    // The ordering the table implies, asserted rather than described: the cheapest
    // kind is outstanding, and so is the kind no amount of vocabulary reaches.
    assert!(!outstanding(BLOCKERS, Gap::Wiring).is_empty());
    assert!(!outstanding(BLOCKERS, Gap::Vocabulary).is_empty());
    let structural = outstanding(BLOCKERS, Gap::Structural);
    assert!(
        structural.contains(&"scoped-keys-silenced-by-the-handover")
            && structural.contains(&"no-active-session-write"),
        "the two rows that decide this verdict must be structural: {structural:?}"
    );

    // The other direction: a table where every row landed permits the handover.
    let all_met: Vec<Blocker> = BLOCKERS
        .iter()
        .map(|b| Blocker {
            blocked: false,
            ..*b
        })
        .collect();
    assert!(handover_is_possible(&all_met));
}

/// **The finding.** A handed-over pane is focused as a plugin pane, and that focus
/// resolves keys in the global scope — so every action scoped to the session list
/// stops resolving, and there are six of them.
///
/// This is not a restatement of the probe. It names the actions, so a failure says
/// *which keyboard* the handover would silence, and it pins the two facts that
/// together make the silence certain: the resolver has no arm for the plugin-pane
/// focus, and the scope it falls through to is the global one.
#[test]
fn the_panes_scoped_keys_stop_resolving_when_a_plugin_pane_holds_focus() {
    let root = repo_root();

    let actions = session_list_actions(&root);
    for expected in [
        "SessionListNext",
        "SessionListPrev",
        "SessionListOpen",
        "SessionListMoveDown",
        "SessionListMoveUp",
        "SessionListSortAlphabetically",
    ] {
        assert!(
            actions.iter().any(|a| a == expected),
            "`{expected}` should be scoped to the session list: {actions:?}"
        );
    }

    let resolver = method_body(
        &root,
        "src/app/key_handlers.rs",
        "pub(crate) fn focus_key_context",
    );
    assert!(
        resolver.contains("InputFocus::SessionList => KeyContext::SessionList"),
        "the native pane's focus is what activates the scope: {resolver}"
    );
    assert!(
        !resolver.contains("PluginPane"),
        "a plugin pane's focus now names a scope — if it is this pane's, the six actions above \
         resolve again and this verdict must be revisited"
    );
    assert!(
        resolver.contains("_ => KeyContext::Global"),
        "the fall-through is what makes the silence certain: {resolver}"
    );
}

/// The keys cannot be replaced by plugin bindings either, and the reason is the
/// *kind* of state they write.
///
/// Pinned separately from the table because it is the sentence a future attempt
/// will want to argue with: `Space` and `d` became portable in the automations
/// pane when the write seam landed, so "a plugin pane's keys can act" is true.
/// What is not true here is that any of this pane's keys act on a **record**.
#[test]
fn none_of_the_panes_keys_acts_on_a_record() {
    let root = repo_root();

    // The five operations the seam grants, so a reader sees what is on offer.
    let methods = writer_methods(&root);
    assert_eq!(methods.len(), 5, "{methods:?}");
    for expected in [
        "set_task_status",
        "delete_task",
        "set_automation_enabled",
        "run_automation",
        "delete_automation",
    ] {
        assert!(
            methods.iter().any(|m| m.contains(expected)),
            "{expected} should be one of the seam's operations: {methods:?}"
        );
    }
    // And none of them is about a session, which is the only record kind this
    // pane's keys touch.
    assert!(
        !methods.iter().any(|m| m.contains("session")),
        "the seam now addresses a session — `Shift+J`/`Shift+K`/`Shift+S` may have become \
         expressible, so re-verdict `no-session-record-write` and check whether the pane can \
         name the row to act on"
    );
}

/// Deleting the renderer would delete the kernel's own navigation, reorder, sort
/// and search rules — so the handover is not "stop calling one function".
///
/// The file viewer's gate found the same class for `FileViewerState` (ADR-39). This
/// pane's case is larger, and it is asserted per consumer so a failure says which
/// rule moved rather than that the module is "still used".
#[test]
fn the_module_is_the_kernels_navigation_not_only_the_panes_paint() {
    let root = repo_root();
    let app = source(&root, "src/app/mod.rs");
    for (rule, what) in [
        (
            "project_list::compute_session_order",
            "Ctrl+J/Ctrl+K navigation",
        ),
        ("project_list::move_in_order", "Shift+J/Shift+K reordering"),
        (
            "project_list::sort_alphabetically_within_groups",
            "Shift+S sorting",
        ),
        (
            "project_list::resolve_rows",
            "the snapshot the plugin itself reads",
        ),
    ] {
        assert!(
            app.contains(rule),
            "`App` no longer uses `{rule}` for {what} — if the rule moved out of \
             `src/ui/project_list.rs`, re-verdict `the-module-is-the-kernels-model`"
        );
    }
    // `SessionMatch` is global search's matcher, and it lives in the pane's module
    // too — checked separately because it is a type rather than a call.
    assert!(
        source(&root, "src/ui/project_list.rs").contains("pub struct SessionMatch"),
        "global search's session matcher should still live in the pane's module"
    );
}

/// The pane the interface draws is still the native one, which is what makes every
/// row above a statement about a handover that has not happened.
#[test]
fn the_native_pane_is_still_what_thurbox_draws() {
    let root = repo_root();
    assert!(
        source(&root, "src/app/view.rs").contains("project_list::render_left_panel"),
        "`src/app/view.rs` no longer draws the native session list — if the pane was handed \
         over, this gate's rows are the record of what that cost"
    );
    // And the bundled plugin still declares no input, so it is a reproduction: the
    // pane's keys are the kernel's, as ADR-33 decided.
    let manifest = source(&root, "src/plugin/bundled/session-list/plugin.toml");
    assert!(
        !manifest.contains("\"input\""),
        "the reproduction declares `input` — a pane that takes keys it cannot act with is worse \
         than one that takes none (ADR-38), so this needs a verdict of its own"
    );
    assert!(
        manifest.contains("default_visible = false"),
        "the reproduction must stay hidden while the native pane is what users see"
    );
}
