//! The session list's **handover** verdict, enforced as a test.
//!
//! `docs/PHASE4-PANE-READINESS.md` §13 records that this pane's *rendering* is
//! reproduced by a bundled plugin — the port ADR-V1 hinges on, which needed no new
//! node kind, no new style token and no new capability. §18 recorded the next step's
//! answer (no) and §32 re-derives it after three other panes were handed over.
//! [`BLOCKERS`] holds one row per requirement the handover cannot meet, each
//! re-derived from the source.
//!
//! Why a test rather than only the document, in `tests/global_search_pane_gap.rs`'s
//! words: a verdict written in markdown is a fact about a build that expires
//! without telling anyone. "The session list cannot be handed over" stops being
//! true the moment someone adds a view write for an unrelated reason, and nothing
//! would say so.
//!
//! ## The reason moved, and this is the second verdict
//!
//! When this gate was written the wall was the **keys**: a handed-over pane was
//! focused as `InputFocus::PluginPane`, which resolves in `KeyContext::Global`, so all
//! six `KeyContext::SessionList` actions stopped resolving — and a plugin could not
//! substitute for them, because `j`/`k` move the **active session**, which is what the
//! central pane, the info column, the file viewer and the code review are all showing.
//!
//! ADR-51 answered that without granting anything: a pane declares that it *is*
//! thurbox's pane for a key context and the kernel performs that context's actions
//! itself. Two panes have since been handed over that way (ADR-53, ADR-56), so three
//! rows here are re-verdicted **closed on a conjunction** — the route exists *and* the
//! power the row named is still absent. The second half is the load-bearing one: it is
//! what keeps "the widening was unnecessary" distinguishable from "the widening
//! happened".
//!
//! **What decides the verdict now** is [`the_verdict_is_derived_from_the_blockers`]'s
//! two structural rows, and neither is about keys:
//!
//! * `the-window-is-the-list-widgets` — the native pane hands its nodes to a ratatui
//!   `List`, and *four* behaviours come off that widget's sticky offset: which rows are
//!   on screen, the `^ N` / `v N` indicators on the border, the click hitboxes (a
//!   repo-group header travels with the row below it, so a two-line item is one
//!   hitbox), and where the pending-spawn placeholder is inserted. A seated plugin pane
//!   windows by the kernel's shared rule over flat children, whose count differs
//!   because the plugin's index counts those headers. Both keep the cursor visible;
//!   they do not agree on which other rows are beside it.
//! * `the-module-is-the-kernels-model` — `src/ui/project_list.rs` owns the comparator
//!   `Ctrl+J`/`Ctrl+K` navigate by, the reorder, the sort, the snapshot the *plugin*
//!   reads, and global search's session matcher.
//!
//! Two of the rows below were promoted out of `tests/bundled_session_list.rs`'s
//! enumerated divergences, where they were documented in `///` blocks — which is the
//! same expiry this gate exists to prevent, one layer down. The port keeps its
//! `assert_ne!`s: those fail when a divergence *closes*, and these rows fail when the
//! tree stops matching the recorded verdict.
//!
//! The left column's circular wrap is deliberately **not** a row; ADR-56 settled it and
//! [`the_left_columns_wrap_is_not_a_blocker`] is why.
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
//! - it is **not** a copy of the automations pane's gate, which is retired: that pane
//!   was handed over (ADR-56) and its rows are preserved in that ADR because none of
//!   the powers they named was granted.
//!
//! Its probes read the source the way a human auditor would, so the gate runs, and
//! means the same thing, with or without the `plugins` Cargo feature. The helpers
//! below duplicate the sibling gates', because an integration test cannot import
//! another one — the alternative is a shared crate for three readers, which is more
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
        stands: "**closed as a handover requirement** (ADR-51), and two panes have since taken \
                 that route in practice (ADR-53, ADR-56). A pane declaring `key_context = \
                 \"SessionList\"` is focused as `InputFocus::SessionList`, so all six actions \
                 resolve and the *kernel* performs them against its own state, still rebindable \
                 in F1. The route this row was written against — a plugin holding its **own** \
                 keys via `input` — is still silenced, and still is not survivable for any of \
                 these six (see the two rows below); the shipped plugin declares no keyboard and \
                 no `input`, which is what makes it a reproduction rather than a half-handover",
        gap: Gap::Structural,
        blocked: false,
        probe: |root| {
            // Four halves, because any one alone would be the wrong claim: the context
            // must be declarable, it must map to *this* pane's focus, the kernel must
            // still resolve that focus to this context, and the shipped plugin must
            // still hold no keys of its own — asserted here so "closed" can never come
            // to mean "the plugin was given the keyboard".
            let declarable =
                method_body(root, "src/session/keybindings.rs", "pub fn pane_keyboards(")
                    .contains("KeyContext::SessionList");
            let maps_to_the_focus =
                method_body(root, "src/app/mod.rs", "pub(crate) fn focus_for_keyboard")
                    .contains("KeyContext::SessionList => Some(InputFocus::SessionList)");
            let kernel_still_dispatches = method_body(
                root,
                "src/app/key_handlers.rs",
                "pub(crate) fn focus_key_context",
            )
            .contains("InputFocus::SessionList => KeyContext::SessionList");
            let plugin_holds_no_keys =
                !source(root, "src/plugin/bundled/session-list/plugin.toml").contains("\"input\"");
            !(declarable
                && maps_to_the_focus
                && kernel_still_dispatches
                && plugin_holds_no_keys
                && !session_list_actions(root).is_empty())
        },
    },
    Blocker {
        id: "no-active-session-write",
        needs: "`j`/`k`/`Enter`, which move the **active session** — and so change what the \
                central pane, the info panel, the file viewer and the code review are all showing",
        stands: "**closed as a handover requirement, and the write was not granted** — which is \
                 the finding, because this row was written expecting the opposite. The kernel \
                 keeps the keys (ADR-51), so `App::switch_session_forward` and its siblings move \
                 the active session exactly as they do today; no binding writes view state, \
                 nothing a plugin holds moves a cursor or takes focus, and the reproduction \
                 declares no `input`. This row is now what stops \"the grant was unnecessary\" \
                 from quietly becoming \"the grant happened\"",
        gap: Gap::Structural,
        blocked: false,
        probe: |root| {
            let the_kernel_keeps_the_keys =
                method_body(root, "src/app/mod.rs", "pub(crate) fn focus_for_keyboard")
                    .contains("KeyContext::SessionList => Some(InputFocus::SessionList)");
            let no_view_write_was_granted = !a_view_write_binding_exists(root);
            !(the_kernel_keeps_the_keys && no_view_write_was_granted)
        },
    },
    Blocker {
        id: "no-session-record-write",
        needs: "`Shift+J`/`Shift+K`, which renumber `sessions.display_order` densely and persist \
                it, and `Shift+S`, which sorts every session within its repo group in one \
                keystroke",
        stands: "**closed as a handover requirement, and no session operation was added** to the \
                 write seam — whose five operations still each address a task or an automation by \
                 id. The kernel performs `SessionListMoveDown`/`MoveUp`/`SortAlphabetically` \
                 (ADR-51), so it renumbers `display_order` densely and persists it exactly as \
                 today. Adding the operation would have widened a plugin's reach over the \
                 database for a pane that still could not name the row to act on, since this \
                 pane's cursor is the kernel's",
        gap: Gap::Structural,
        blocked: false,
        probe: |root| {
            let the_kernel_keeps_the_keys =
                method_body(root, "src/app/mod.rs", "pub(crate) fn focus_for_keyboard")
                    .contains("KeyContext::SessionList => Some(InputFocus::SessionList)");
            let no_session_operation = !writer_methods(root).iter().any(|m| {
                ["session", "order", "reorder", "sort", "move"]
                    .iter()
                    .any(|n| m.contains(n))
            });
            !(the_kernel_keeps_the_keys && no_session_operation)
        },
    },
    Blocker {
        id: "no-left-seat",
        needs: "the pane's seat: the session list **is** the left column, above the automations \
                pane",
        stands: "**closed** (ADR-46). `PaneSlot::Left` names `RegionId::SessionList`, a visible \
                 pane claiming it is drawn into that rect, and `App::render_left_panel` stands \
                 down for it — so the reproduction is now placeable *where this pane is*, and it \
                 ships in that seat. It closes the seat only: the pane drawn there is still \
                 focused as a plugin pane, which is what \
                 `scoped-keys-silenced-by-the-handover` is about",
        gap: Gap::Structural,
        blocked: false,
        probe: |root| {
            let seat_exists = method_body(root, "src/session/plugin_manifest.rs", "pub fn seat(")
                .contains("Some(RegionId::SessionList)");
            let native_stands_down = method_body(root, "src/app/view.rs", "fn render_left_panel")
                .contains("seat_taken(PaneSlot::Left)");
            !(seat_exists && native_stands_down)
        },
    },
    Blocker {
        id: "the-window-is-the-list-widgets",
        needs: "which rows are on screen when the list overflows — and the three things the \
                native pane derives from the same offset: the `^ N` / `v N` clipped-row \
                indicators on the block border, the click hitboxes (a repo-group header travels \
                with the row below it, so a two-line item is **one** hitbox), and the index the \
                pending-spawn placeholder is inserted at",
        stands: "`render_session_section` hands its nodes to a ratatui `List` with a `ListState`, \
                 and reads `list_state.offset()` back **after** the stateful render for all three \
                 of the above. A seated plugin pane declares its cursor and the kernel windows it \
                 with `ui::visible_window` over flat single rows — a different rule, \
                 over a different row count, since the plugin's index counts the headers the \
                 native item folds in (`the_two_panes_window_a_long_list_by_different_rules`). So \
                 both keep the cursor visible and they do **not** agree on which other rows are \
                 beside it: a handover would change which sessions are on screen whenever the \
                 list overflows, in the pane whose selection decides what the central pane, the \
                 info column, the file viewer and the code review are all showing. Not closable \
                 by teaching `visible_window` the widget's sticky offset — that helper is what \
                 every plugin list and three native panes scroll by, so a change for this pane \
                 changes all of them (the hazard ADR-39 recorded from the other side). The \
                 port's own file calls this Phase 6 work; this row is what that work is",
        gap: Gap::Structural,
        blocked: true,
        probe: |root| {
            // Four halves, one per behaviour derived from the offset, because a probe
            // reading only the window would report a wiring detail where there are four
            // consumers. Plus the plugin side, so "the two windows differ" is derived
            // rather than assumed.
            let pane = source(root, "src/ui/project_list.rs");
            let widget_owns_the_window =
                pane.contains("render_stateful_widget") && pane.contains("ListState");
            let indicators_come_off_it = pane.contains("render_scroll_indicators_variable");
            let hitboxes_come_off_it = pane.contains("skip(list_state.offset())");
            let placeholder_comes_off_it = pane.contains("items.insert(slot.index");
            // And a plugin pane's window is the kernel's shared helper over the tree's
            // own children, which is a different rule over a different count.
            let plugin_window_is_the_shared_rule =
                source(root, "src/ui/plugin_pane.rs").contains("super::visible_window");
            widget_owns_the_window
                && indicators_come_off_it
                && hitboxes_come_off_it
                && placeholder_comes_off_it
                && plugin_window_is_the_shared_rule
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
        needs: "the highlight following the cursor within one of the interface's own forced \
                frames, rather than on a cadence of its own",
        stands: "**closed** (ADR-49), and the row's wording had to change with it — recorded here \
                 rather than quietly. The trigger is now the change: the publisher tells the \
                 worker which *sources* moved and the worker renders the panes that read them, so \
                 `Ctrl+J` re-renders this pane because the session source moved. What is left is \
                 a rate ceiling of 100 ms, which coalesces rather than delays — a change at rest \
                 renders with no wait, and the worst case is a change arriving just after a pass. \
                 The **original** wording asked for the highlight to move *in the frame the key \
                 was handled*, and that is unreachable by construction, not by wiring: \
                 `plugin-host/panes` forbids the kernel calling a plugin during a frame, so a \
                 plugin tree is always produced off the drawing thread. The achievable bar is the \
                 one above, and 100 ms is inside the kernel's own 250 ms forced-redraw floor. The \
                 spike's 5 ms figure is therefore not met and not claimed",
        gap: Gap::Wiring,
        blocked: false,
        probe: |root| {
            let main = source(root, "src/main.rs");
            // Present again if the worker goes back to rendering on a cadence, or
            // if the nudge that replaced it stops arriving.
            let fixed_wait = main.contains("for _ in 0..PLUGIN_RENDER_SLICES");
            let no_trigger = !main.contains("RenderTrigger");
            let no_nudge =
                !source(root, "src/app/mod.rs").contains("PluginWorkerRequest::StateMoved");
            fixed_wait || no_trigger || no_nudge
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
    Blocker {
        id: "non-ascii-whitespace-is-the-kernels-trim",
        needs: "the agent activity text a row shows, trimmed the way the kernel trims it: \
                `str::trim`, which is Unicode-aware",
        stands: "the plugin trims with Luau's `%s`, which is ASCII-only, so a no-break space \
                 around an activity title survives in its copy \
                 (`non_ascii_whitespace_is_trimmed_by_the_kernel_only`). Left open by the port \
                 rather than closed by publishing the *trimmed* text, because a trim is a \
                 presentation decision about the pane's own row and the port's rule is that the \
                 kernel publishes no rendering — so closing it needs either a Unicode-aware \
                 predicate a plugin can reach or a decision to reverse that rule. Small, and a \
                 row because a handover would ship it",
        gap: Gap::Vocabulary,
        blocked: true,
        probe: |root| {
            // Both sides, so the row is about a *difference* rather than about either
            // implementation: the kernel's trim is Rust's, and the plugin's is Luau's
            // ASCII class.
            let kernel_trims_unicode =
                source(root, "src/ui/project_list.rs").contains(".map(str::trim)");
            let plugin_trims_ascii = source(root, "src/plugin/bundled/session-list/init.luau")
                .contains(r#"string.match(activity, "^%s*(.-)%s*$")"#);
            kernel_trims_unicode && plugin_trims_ascii
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
    // The ordering the table implies, asserted rather than described. The cheapest
    // kind is **no longer** outstanding: this pane's only wiring row was the render
    // trigger, closed by ADR-49. So what is left of the verdict is vocabulary and
    // the kind no amount of vocabulary reaches — which is the honest summary of why
    // this pane still cannot go.
    assert!(
        outstanding(BLOCKERS, Gap::Wiring).is_empty(),
        "the render trigger was this pane's last wiring gap; a new one means the \
         table grew a row that should be named here: {:?}",
        outstanding(BLOCKERS, Gap::Wiring)
    );
    assert!(!outstanding(BLOCKERS, Gap::Vocabulary).is_empty());
    let structural = outstanding(BLOCKERS, Gap::Structural);
    // The two that decide it, and they are **not** the two this gate was written
    // around. Those were the keys (`scoped-keys-silenced-by-the-handover`,
    // `no-active-session-write`), which ADR-51 answered without granting anything;
    // what is left is the widget the pane windows through and the module that is the
    // kernel's own navigation (ADR-57).
    assert!(
        structural.contains(&"the-window-is-the-list-widgets")
            && structural.contains(&"the-module-is-the-kernels-model"),
        "the two rows that decide this verdict must be structural: {structural:?}"
    );
    // And the three the route retired are not blockers of any kind. Asserted
    // positively so a regression that reopened one — a plugin handed `input`, a view
    // write, a session operation on the seam — fails here with the reason attached
    // rather than merely growing the table.
    for closed in [
        "scoped-keys-silenced-by-the-handover",
        "no-active-session-write",
        "no-session-record-write",
    ] {
        assert!(
            !structural.contains(&closed),
            "`{closed}` is closed by ADR-51 *without* a grant; if it is outstanding \
             again, either the route regressed or a power was granted — and the second \
             is the one this row exists to catch: {structural:?}"
        );
    }

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

/// **The left column's circular wrap is not one of this pane's blockers**, and that
/// is asserted rather than described because a reader will re-derive it as one.
///
/// It was a row in the *automations* pane's gate for as long as that pane held its own
/// keys: a plugin declined the movement key at its edge, and nothing completed the
/// wrap, because moving focus is view state no capability writes. ADR-56 handed that
/// pane over on the kernel-keyboard route and the row disappeared rather than closing —
/// two facts make the wrap a non-issue, and this pins both:
///
/// * both ends are **kernel focuses** whoever draws either pane, since a pane declaring
///   a keyboard is focused as thurbox's own pane of that name; and
/// * the wrap's condition is already "a pane provides that list" rather than a
///   `[features]` flag, so it is correct for a handed-over pane on either side.
///
/// So a handover of this pane inherits the wrap unchanged, and the gate above does not
/// list it.
#[test]
fn the_left_columns_wrap_is_not_a_blocker() {
    let root = repo_root();

    // Both ends of the wrap are kernel focuses a declared keyboard resolves to.
    let table = method_body(&root, "src/app/mod.rs", "pub(crate) fn focus_for_keyboard");
    for arm in [
        "KeyContext::SessionList => Some(InputFocus::SessionList)",
        "KeyContext::Automations => Some(InputFocus::Automations)",
    ] {
        assert!(
            table.contains(arm),
            "the wrap's two ends must both be focuses a declared keyboard reaches: {table}"
        );
    }

    // And the kernel's own handlers are what move focus between them, gated on the
    // *pane* rather than on the feature flag (ADR-56).
    let handlers = source(&root, "src/app/key_handlers.rs");
    assert!(
        handlers.contains("fn automations_pane_provided"),
        "the wrap's condition should be that a pane provides the automations list"
    );
    let next = method_body(&root, "src/app/key_handlers.rs", "fn act_session_list_next");
    assert!(
        next.contains("self.automations_pane_provided()")
            && next.contains("InputFocus::Automations"),
        "`j` at the last session should still enter the automations pane: {next}"
    );
    assert!(
        !next.contains("features.automations"),
        "the wrap is gated on the pane, not on the flag — a flag can be on with no band \
         on screen: {next}"
    );
}

/// The order the work would be done in, asserted from the rows rather than argued in
/// prose.
///
/// The window is first because three of the other rows are **functions** of it — the
/// border indicators, the click hitboxes and the pending-spawn slot are all read off
/// `ListState::offset()` — and because `resolve_rows`, which the module row would
/// relocate, is what feeds both panes. A change that closed the chrome first would have
/// built it against a window that is about to change.
#[test]
fn the_window_is_settled_before_what_depends_on_it() {
    let deciders = [
        "the-window-is-the-list-widgets",
        "the-module-is-the-kernels-model",
    ];
    let dependents = ["no-pane-chrome", "no-pending-spawn-row"];
    let ids: Vec<&str> = BLOCKERS.iter().map(|b| b.id).collect();
    let position = |id: &str| ids.iter().position(|x| *x == id).expect(id);
    for decider in deciders {
        for dependent in dependents {
            assert!(
                position(decider) < position(dependent),
                "`{decider}` must be listed before `{dependent}`: the table's order is \
                 the ordering of the work, and the chrome and the placeholder are both \
                 read off the window"
            );
        }
    }
    // And the two dependents are still outstanding, or the ordering claim is vacuous.
    for dependent in dependents {
        assert!(
            BLOCKERS.iter().any(|b| b.id == dependent && b.blocked),
            "`{dependent}` should still be outstanding"
        );
    }
}

/// **The finding this gate was written for, kept scoped to the route it is true of.**
/// A pane handed over with its **own** keys is focused as a plugin pane, and that focus
/// resolves keys in the global scope — so every action scoped to the session list stops
/// resolving, and there are six of them.
///
/// Still true, and no longer the verdict: ADR-51's route is focused as
/// `InputFocus::SessionList` instead, which is why
/// `scoped-keys-silenced-by-the-handover` is closed. Kept because it is what makes
/// giving *this* pane's keys to a plugin unsurvivable, and because a change that added
/// a `PluginPane` arm to the resolver would be reversing a decision rather than fixing
/// a bug.
///
/// This is not a restatement of the probe. It names the actions, so a failure says
/// *which keyboard* the plugin-keys route would silence, and it pins the two facts that
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
