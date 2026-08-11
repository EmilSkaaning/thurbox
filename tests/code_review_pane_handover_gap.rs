//! The code-review view's **handover** verdict, enforced as a test.
//!
//! `docs/PHASE4-PANE-READINESS.md` §19 records that this pane's whole *document* is
//! reproduced by a bundled plugin (ADR-44) — every row kind the native pane lists,
//! pinned to the untouched renderer row by row. §20 records the next step's answer:
//! the pane is **not** handed over, and this file is that half of the verdict in
//! executable form. [`BLOCKERS`] holds one row per requirement the handover could not
//! meet, each re-derived from the source.
//!
//! Why a test rather than only the document, in `tests/global_search_pane_gap.rs`'s
//! words: a verdict written in markdown is a fact about a build that expires without
//! telling anyone.
//!
//! **Three findings this gate exists to keep true**, and they are why this pane is
//! the furthest from a handover rather than the closest:
//!
//! - [`the_review_is_two_seats_not_one`]. Every earlier refusal needed one seat.
//!   This surface is two panes: the diff in the **central** pane, and the
//!   changed-files list in the **file-viewer column** — its own focus
//!   (`InputFocus::ReviewFiles`), its own keys, and force-shown by `App::layout_for`
//!   for as long as a review is open. One plugin would have to be granted, focused
//!   and navigated in two columns at once. ADR-46 gave a plugin the first of the two
//!   and deliberately not the second, so the finding narrowed rather than closed.
//! - [`the_reviews_keyboard_resolves_no_action`]. The tasks, automations and
//!   session-list refusals could each name the scoped `Action`s a plugin binding
//!   would replace. This pane's keys are **not actions at all**: `KeyContext` has six
//!   members and none is a review, and `handle_code_review_key` /
//!   `handle_review_files_key` are captures keyed on `self.focus`, run ahead of the
//!   keybinding lookup. So the keys are not rebindable, the F1 editor has never
//!   listed them, and no `keybindings.json` could restore them after a handover.
//! - [`the_mouse_channel_carries_a_row_where_the_pane_needs_a_column`]. The pane is
//!   documented mouse-first, and a plugin pane's click reports a row. Most of the
//!   difference is missing target *kinds* (eleven footer buttons, a scrollbar, a
//!   wheel, picker entries) — but one is a missing *coordinate*: on a paired
//!   side-by-side row, the half clicked decides which side of the diff a comment
//!   attaches to (`CodeReviewState::click_side`). No extra target kind closes that.
//!
//! And one row where this pane is **closer** than the session list, which is the
//! point of a gate rather than a verdict:
//! [`the_review_cursor_is_a_narrower_grant_than_the_session_lists`]. The session
//! list's cursor *is* the application's active session, so making it writable is the
//! widest grant in the host. This pane's cursor is a row inside a view the user
//! already opened. Two rows spelled alike, very different prices — and the narrower
//! one is where the work would start.
//!
//! Three things this gate is deliberately not:
//!
//! - it is **not** the teardown gate, which answers whether
//!   `src/ui/code_review.rs` may be deleted — already no, and no either way. One
//!   table answering two questions produces failures that do not say which question
//!   moved;
//! - it is **not** a claim that the pane's rendering falls short.
//!   `tests/bundled_code_review.rs` asserts the plugin builds the identical view tree
//!   for every row kind and paints the identical frame when the pane scrolls;
//! - it is **not** a request for a capability. This change adds none, because a
//!   capability landing beside a verdict has no consumer — the defect the earlier
//!   gates found in `input`, `tasks-write` and `automations-write`.
//!
//! Its probes read the source the way a human auditor would, so the gate runs, and
//! means the same thing, with or without the `plugins` Cargo feature. The helpers
//! below duplicate the sibling gates', because an integration test cannot import
//! another one — the alternative is a shared crate for six readers, which is more
//! machinery than the duplication costs.

use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

/// Why a requirement is unmet — which decides the *order* the work would be done in,
/// not only that it is outstanding.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Gap {
    /// A power a plugin is not given, on purpose. Reversing it changes what a plugin
    /// *is*, so no node and no wiring closes it.
    Structural,
    /// Something today's drawing catalogue cannot say and could.
    Vocabulary,
    /// Something the host could do today with no new plugin-facing concept: which
    /// facts a plugin is told, or which events reach it. Cheapest to close, and filed
    /// apart from the other two because calling it structural would claim the model
    /// forbids it — it does not.
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

/// Every requirement the handover of this pane needs and does not have, structural
/// rows first.
///
/// Each probe is scoped to the *declaration* it reads rather than to a whole file,
/// copied from `tests/teardown_gate.rs` for its reason: an unrelated mention
/// elsewhere in a file must not flip a verdict.
const BLOCKERS: &[Blocker] = &[
    Blocker {
        id: "no-central-seat",
        needs: "the seat the diff is drawn in: the review owns the **central** pane, the one the \
                agent terminal and the shell share, selected from the tab strip",
        stands: "**closed as a seat** (ADR-46). `PaneSlot::Center` names `RegionId::Center` and \
                 `App::render_central_pane` stands down for a pane that claims it, so the diff \
                 is placeable where the review is drawn. Two things the seat does not carry, and \
                 both are recorded rather than absorbed: the tab strip that *selects* this view \
                 is kernel chrome and is not drawn over a plugin-owned centre, and the review's \
                 second seat is a different row below",
        gap: Gap::Structural,
        blocked: false,
        probe: |root| {
            let seat_exists = method_body(root, "src/session/plugin_manifest.rs", "pub fn seat(")
                .contains("Some(RegionId::Center)");
            let native_stands_down = method_body(root, "src/app/view.rs", "fn render_central_pane")
                .contains("seat_taken(PaneSlot::Center)");
            // And the central pane is still where the review is drawn, from the one
            // file that draws it — a seat for a surface drawn elsewhere would prove
            // nothing.
            let central = source(root, "src/app/view.rs").contains("code_review::render(");
            !(seat_exists && native_stands_down && central)
        },
    },
    Blocker {
        id: "no-second-seat-for-the-changed-files-list",
        needs: "the review's **second** pane: the changed-files list in the file-viewer column, \
                with its own focus, its own navigation keys (`j`/`k`, `g`/`G`, `Enter`, `r`/`R`, \
                `/`) and a diff that follows its selection",
        stands: "`App::layout_for` forces the file-viewer column present for as long as a review \
                 is open, and `InputFocus::ReviewFiles` is a focus ring stop of its own. The \
                 workspace tree seats this list as `RegionId::FileViewer` and a plugin pane as \
                 `RegionId::Plugin(n)` — a *separate* region. ADR-46 gave a plugin four seats and \
                 this is not one of them: no slot names `RegionId::FileViewer`, deliberately, \
                 because that column's occupant is the native list a review forces present. So \
                 handing over the diff alone would leave the interface drawing a native \
                 changed-files list beside a plugin diff, and handing over both still needs a \
                 second seat this pane's column does not offer",
        gap: Gap::Structural,
        blocked: true,
        probe: |root| {
            let forces_column = method_body(root, "src/app/mod.rs", "fn layout_for")
                .contains("self.active_review().is_some()");
            let own_focus = source(root, "src/app/view.rs").contains("render_files_list(");
            let is_a_focus = source(root, "src/app/key_handlers.rs").contains("ReviewFiles");
            // The seats themselves: this list is the file-viewer region, a plugin
            // pane is its own indexed region, and no slot reaches that region. Read
            // from the slot→region table rather than from the variant list, so a
            // slot added for another seat does not flip this row.
            let regions = variant_names(&block(
                root,
                "src/session/workspace_tree.rs",
                "pub enum RegionId",
            ));
            let separate_regions = regions.contains(&"FileViewer".to_string())
                && regions.contains(&"Plugin".to_string());
            let no_slot_reaches_it =
                !method_body(root, "src/session/plugin_manifest.rs", "pub fn seat(")
                    .contains("RegionId::FileViewer");
            forces_column && own_focus && is_a_focus && separate_regions && no_slot_reaches_it
        },
    },
    Blocker {
        id: "keys-are-a-capture-not-actions",
        needs: "the pane's keyboard: navigation (`j`/`k`, `g`/`G`, `{`/`}`, `[`/`]`, page and \
                half-page), the comment keys (`c`/`f`/`s`), marks (`r`/`R`), layout (`v`/`w`), \
                the target picker (`t`), find (`/`, `n`/`N`) and export (`y`/`e`)",
        stands: "none of it is an `Action`. `KeyContext` names six scopes and no review, and \
                 `App::handle_key` runs `handle_code_review_key` and `handle_review_files_key` \
                 **before** the keybinding lookup, keyed on `self.focus`. A handed-over pane is \
                 focused as `InputFocus::PluginPane`, which those captures do not name and which \
                 `focus_key_context` maps to `KeyContext::Global` — so a plugin binding has no \
                 action to claim, the keys are not rebindable today, and no keybindings file \
                 could restore them. Turning them into scoped actions is a keybinding-vocabulary \
                 change and precedes anything plugin-facing",
        gap: Gap::Structural,
        blocked: true,
        probe: |root| {
            let contexts = variant_names(&block(
                root,
                "src/session/keybindings.rs",
                "pub enum KeyContext",
            ));
            let no_review_context = !contexts.iter().any(|c| c.contains("Review"));
            let handle = method_body(root, "src/app/key_handlers.rs", "fn handle_key");
            let captured_before_lookup = handle
                .find("handle_code_review_key")
                .zip(handle.find("lookup_in"))
                .is_some_and(|(capture, lookup)| capture < lookup);
            let focus_ctx = method_body(root, "src/app/key_handlers.rs", "fn focus_key_context");
            no_review_context && captured_before_lookup && !focus_ctx.contains("PluginPane")
        },
    },
    Blocker {
        id: "no-review-write",
        needs: "`r`/`R` (mark a file or hunk reviewed), `c`/`f`/`s` and the compose box (write a \
                line, file or summary comment), and editing or deleting one",
        stands: "the mutation seam's five operations set a task's status, delete a task, set an \
                 automation's enabled flag, run one and delete one. None addresses a review: \
                 there is no `review_comments` write and no `review_marks` write, and denial is \
                 by the absence of a binding, so there is nothing to refuse either",
        gap: Gap::Structural,
        blocked: true,
        probe: |root| {
            let methods = writer_methods(root);
            !methods.iter().any(|m| {
                ["review", "comment", "mark", "reviewed"]
                    .iter()
                    .any(|n| m.contains(n))
            })
        },
    },
    Blocker {
        id: "no-retarget-operation",
        needs: "`t`, the target picker: choosing Working, Branch or a commit rebuilds the diff",
        stands: "a retarget resolves each repo's base, lists commits and runs `git diff` or `git \
                 show` — over SSH for a remote session — on a `spawn_blocking` worker \
                 (`ReviewBuildKind::Retarget`, ADR-P8). The capability vocabulary defines nothing \
                 that runs a version-control command, which `review`'s own contract states as the \
                 point: it publishes the review the **user** opened. A pane asking to retarget is \
                 asking to choose what gets diffed, which is a different power from reading a \
                 diff",
        gap: Gap::Structural,
        blocked: true,
        probe: |root| {
            let no_vcs_capability = !capability_names(root)
                .iter()
                .any(|c| ["git", "vcs", "exec", "fs"].contains(&c.as_str()));
            let retarget_is_a_worker_build = source(root, "src/app/code_review.rs")
                .contains("ReviewBuildKind")
                && source(root, "src/app/code_review.rs").contains("Retarget");
            no_vcs_capability && retarget_is_a_worker_build
        },
    },
    Blocker {
        id: "no-export-operation",
        needs: "`y` (copy the compiled review as markdown to the clipboard) and `e` (paste it \
                into the session's agent as a prompt to address it)",
        stands: "neither reach exists. The seam writes records, not the clipboard and not a \
                 session's pty, and no capability names either. `e` is the review's whole export \
                 story — thurbox deliberately has no submit-to-forge — so a handed-over pane \
                 would lose the loop the feature exists to close",
        gap: Gap::Structural,
        blocked: true,
        probe: |root| {
            let methods = writer_methods(root);
            let no_seam_reach = !methods.iter().any(|m| {
                ["clipboard", "copy", "prompt", "send", "paste"]
                    .iter()
                    .any(|n| m.contains(n))
            });
            let no_capability = !capability_names(root)
                .iter()
                .any(|c| ["clipboard", "prompt", "agent", "pty"].contains(&c.as_str()));
            no_seam_reach && no_capability
        },
    },
    Blocker {
        id: "no-cursor-write",
        needs: "every navigation key, and the wheel: they move `CodeReviewState::selected`, which \
                is the row the published section names as its cursor and the row the \
                changed-files list highlights",
        stands: "a pane reads the cursor and cannot move it. This is the session list's row \
                 (ADR-43) with a **narrower** shape, which is the reason to state it separately: \
                 that pane's cursor is the application's active session, so writing it is the \
                 widest grant in the host, whereas this one is a row inside a view the user \
                 already opened — nothing outside the review reads it except the changed-files \
                 highlight. It is therefore the cheapest place a view-state write could start, \
                 and the order this table implies",
        gap: Gap::Structural,
        blocked: true,
        probe: |root| {
            let methods = writer_methods(root);
            !methods
                .iter()
                .any(|m| ["cursor", "select", "focus"].iter().any(|n| m.contains(n)))
        },
    },
    Blocker {
        id: "no-resolved-width",
        needs: "`v` (paired side-by-side), `w` (soft wrap) and `←`/`→` (horizontal scroll) — and, \
                behind them, the ellipsis four row kinds are truncated with",
        stands: "a view tree carries no width by construction: `ViewNode::Fill` exists precisely \
                 because \"a plugin is never told the width it got\", and this pane's three \
                 layouts each *divide* or *chunk* against a resolved one \
                 (`paired_body_width`, `unified_diff_line_wrapped`). Publishing a width would \
                 make every pane a geometry problem — and a pane that mis-measured a \
                 double-width character would paint over its neighbour",
        gap: Gap::Structural,
        blocked: true,
        probe: |root| {
            let fill = block(root, "src/session/view_tree.rs", "pub enum ViewNode");
            let width_is_withheld = fill.contains("a plugin is never told the width");
            let layouts_need_one = source(root, "src/ui/code_review.rs")
                .contains("paired_body_width")
                && source(root, "src/ui/code_review.rs").contains("unified_diff_line_wrapped");
            width_is_withheld && layouts_need_one
        },
    },
    Blocker {
        id: "mouse-carries-no-column",
        needs: "the pane's mouse surface: eleven footer buttons, a draggable scrollbar, the \
                wheel, the target picker's entries, and a click on a paired row whose **half** \
                decides which side of the diff a comment attaches to",
        stands: "a plugin pane's click reaches `onClick(paneId, row)` — the row of the pane's \
                 outermost list and nothing else. Most of the difference is missing target \
                 *kinds*, which a wider event could carry. `click_side` is a missing \
                 **coordinate**: `App::cr_click_row` takes the click's `rel_x` and the pane's \
                 width and resolves the half from them, and no additional target kind expresses \
                 \"the user clicked the old side\" — so this half of the row does not close by \
                 widening the event's variants",
        gap: Gap::Wiring,
        blocked: true,
        probe: |root| {
            let click = source(root, "src/plugin/runtime.rs");
            let row_only = click.contains("fn on_click(&self, pane_id: &str, row: usize)");
            let side_matters = source(root, "src/app/code_review.rs")
                .contains("fn cr_click_row(&mut self, idx: usize, rel_x: u16, width: u16)");
            let buttons = !variant_names(&block(
                root,
                "src/app/code_review.rs",
                "pub(crate) enum ReviewButton",
            ))
            .is_empty();
            row_only && side_matters && buttons
        },
    },
    Blocker {
        id: "no-anchored-overlay",
        needs: "the compose box, which floats **inline at the selected row**, and the target \
                picker, which floats over the diff",
        stands: "both are `session::overlay` placements resolved against the pane's rect, and the \
                 view tree has no node that floats: a tree's nodes stack. This is §10's \
                 still-open bottom-anchored-region row in a second shape — anchored to a *row* \
                 rather than to an edge — and the fill added by ADR-31 does not close it, since \
                 an inline residue anchors nothing",
        gap: Gap::Vocabulary,
        blocked: true,
        probe: |root| {
            let kinds = view_node_kinds(root);
            let nothing_floats = !kinds
                .iter()
                .any(|k| ["Overlay", "Float", "Anchored", "Popup"].contains(&k.as_str()));
            let compose_is_an_overlay = source(root, "src/ui/code_review.rs")
                .contains("fn compose_anchor(")
                && source(root, "src/ui/code_review.rs").contains("Overlay");
            nothing_floats && compose_is_an_overlay
        },
    },
    Blocker {
        id: "no-in-pane-text-field",
        needs: "the find sub-mode's bar and the compose box's body — a query and a multi-line \
                body typed **inside** the pane, with a cursor shown where the caret is",
        stands: "a plugin pane can receive keys with `input`, so accumulating a string is within \
                 reach, but drawing one is not: no node carries a caret appearance, and the find \
                 bar's in-row match emphasis *replaces* the syntax colouring for the matched run \
                 — a third styling mode over a tree whose runs are already the pane's tokens. \
                 The other half of find is `no-cursor-write`: stepping matches moves the review's \
                 selection",
        gap: Gap::Vocabulary,
        blocked: true,
        probe: |root| {
            let kinds = view_node_kinds(root);
            let no_caret_node = !kinds
                .iter()
                .any(|k| ["Input", "Field", "Cursor", "Caret"].contains(&k.as_str()));
            let find_highlights_in_row = source(root, "src/ui/code_review.rs")
                .contains("fn highlight_text(")
                && source(root, "src/ui/code_review.rs").contains("fn render_search_bar(");
            no_caret_node && find_highlights_in_row
        },
    },
];

/// Whether every requirement is met — the verdict, derived from the rows so that both
/// answers are testable.
fn handover_is_possible(blockers: &[Blocker]) -> bool {
    blockers.iter().all(|b| !b.blocked)
}

/// The outstanding rows of one kind, which is the order the work sorts into.
fn outstanding(blockers: &[Blocker], gap: Gap) -> Vec<&'static str> {
    blockers
        .iter()
        .filter(|b| b.blocked && b.gap == gap)
        .map(|b| b.id)
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

/// The body of the top-level `struct`/`enum` whose declaration starts with `header`.
/// rustfmt closes a top-level item with a `}` in column zero.
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

/// The body of a method — an item **inside** an `impl`, which [`block`] cannot scope
/// because its close is indented rather than in column zero.
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

/// The **signatures** of the mutation seam's methods, lowercased.
///
/// A signature rather than the whole trait body, because the trait's own
/// documentation names the powers it withholds — so a probe over the prose would read
/// the sentence stating a rule as the rule being broken.
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
        methods.iter().any(|m| m.contains("set_automation_enabled")),
        "no writer signatures found — the probe's anchor changed shape, so every verdict derived \
         from it is meaningless"
    );
    methods
}

/// The capability **wire names** the host defines, lowercased.
///
/// Read from `as_str`'s arms rather than from the variant list, because a wire name
/// is what a manifest declares and what this gate's rows talk about.
fn capability_names(root: &Path) -> Vec<String> {
    let text = source(root, "src/session/plugin_manifest.rs");
    let start = text
        .find("Capability::Render => \"render\"")
        .unwrap_or_else(|| panic!("the capability wire-name table changed shape"));
    let rest = &text[start..];
    let end = rest
        .find("\n        }")
        .unwrap_or_else(|| panic!("the capability wire-name table has no close"));
    let names: Vec<String> = rest[..end]
        .lines()
        .filter_map(|line| {
            let (_, after) = line.split_once("=> \"")?;
            let (name, _) = after.split_once('"')?;
            Some(name.to_lowercase())
        })
        .collect();
    assert!(
        names.contains(&"review".to_string()),
        "the capability names probe found no `review`: {names:?}"
    );
    names
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
        "the code review's handover verdict disagrees with the source tree.\n\
         A row that closed changes what a handover would cost: re-verdict it here, revisit \
         docs/PHASE4-PANE-READINESS.md §20 and ADR-45, and check whether the pane is now \
         portable.{failures}"
    );
}

/// The verdict follows from the rows, so both answers are checkable — today's, and a
/// table where everything landed.
#[test]
fn the_verdict_is_derived_from_the_blockers() {
    assert!(
        !handover_is_possible(BLOCKERS),
        "every requirement is recorded met — the code review is portable, so hand it over \
         deliberately (and retire this gate) rather than leaving it passing vacuously"
    );
    // The ordering the table implies, asserted rather than described: the rows that
    // decide the verdict are structural, and the two seats are among them.
    assert!(!outstanding(BLOCKERS, Gap::Wiring).is_empty());
    assert!(!outstanding(BLOCKERS, Gap::Vocabulary).is_empty());
    let structural = outstanding(BLOCKERS, Gap::Structural);
    // The central seat closed with ADR-46; the review's *second* seat did not, and
    // with the capture-keyed keyboard it is what still decides the verdict.
    assert!(
        structural.contains(&"no-second-seat-for-the-changed-files-list")
            && structural.contains(&"keys-are-a-capture-not-actions"),
        "the second seat and the capture-keyed keyboard are what decide this verdict, and both \
         are structural: {structural:?}"
    );
    assert!(
        !structural.contains(&"no-central-seat"),
        "the central seat is closed (ADR-46) — a table recording it outstanding again means the \
         seating regressed: {structural:?}"
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

/// **The first finding.** The review is *two* panes, in two different columns, and a
/// plugin can be seated in one of them.
///
/// Not a restatement of the two seat rows: it names what each seat draws and what a
/// half-handover would leave on screen, so a failure says which surface went.
#[test]
fn the_review_is_two_seats_not_one() {
    let root = repo_root();

    // Seat one: the diff, in the central pane.
    let view = source(&root, "src/app/view.rs");
    assert!(
        view.contains("fn render_code_review_pane"),
        "the diff should be drawn as a central-pane surface"
    );
    // Seat two: the changed-files list, in the file-viewer column — forced present
    // for as long as a review is open, so it is not an optional extra.
    assert!(
        view.contains("render_files_list("),
        "the changed-files list should be drawn into the file-viewer column"
    );
    let layout = method_body(&root, "src/app/mod.rs", "fn layout_for");
    assert!(
        layout.contains("self.show_file_viewer || self.active_review().is_some()"),
        "an open review should force the file-viewer column present: {layout}"
    );

    // And the second seat is a pane in its own right: its own focus, its own key
    // capture, its own ring stop.
    let keys = source(&root, "src/app/key_handlers.rs");
    assert!(
        keys.contains("handle_review_files_key"),
        "the changed-files list should capture its own keys"
    );
    assert!(
        keys.contains("InputFocus::ReviewFiles"),
        "the changed-files list should be a focus of its own"
    );

    // A plugin, meanwhile, reaches exactly one of the two: ADR-46 gave it the
    // central seat and deliberately not the file-viewer column, whose occupant is
    // the native list a review forces present. Read from the slot→region table, so
    // the assertion says which seat is missing rather than how many slots exist.
    let seats = method_body(&root, "src/session/plugin_manifest.rs", "pub fn seat(");
    assert!(
        seats.contains("Some(RegionId::Center)"),
        "a plugin should reach the central seat (ADR-46): {seats}"
    );
    assert!(
        !seats.contains("RegionId::FileViewer"),
        "a plugin pane now reaches the file-viewer column — re-verdict the second seat row and \
         check what a two-seat surface would need: {seats}"
    );
}

/// **The second finding.** The pane's keys are a capture, not actions — so a handover
/// loses them with nothing to name in their place, and no configuration file can put
/// them back.
///
/// Pinned separately because it is the sentence a future attempt will want to argue
/// with. The earlier refusals lost *bindable* actions; this one loses keys that were
/// never bindable, which makes "let the plugin declare the bindings" not merely
/// insufficient but inexpressible.
#[test]
fn the_reviews_keyboard_resolves_no_action() {
    let root = repo_root();

    // No review scope exists in the keybinding vocabulary.
    let contexts = variant_names(&block(
        &root,
        "src/session/keybindings.rs",
        "pub enum KeyContext",
    ));
    assert!(
        !contexts.iter().any(|c| c.contains("Review")),
        "a review key context now exists — if the pane's keys became actions, re-verdict \
         `keys-are-a-capture-not-actions`: {contexts:?}"
    );

    // The captures run ahead of the lookup, keyed on the focused surface.
    let handle = method_body(&root, "src/app/key_handlers.rs", "fn handle_key");
    let capture = handle
        .find("handle_code_review_key")
        .expect("the review should capture keys in handle_key");
    let files = handle
        .find("handle_review_files_key")
        .expect("the changed-files list should capture keys in handle_key");
    let lookup = handle
        .find("lookup_in")
        .expect("the keybinding lookup should follow the captures");
    assert!(
        capture < lookup && files < lookup,
        "both captures should run before the keybinding lookup, which is why a scoped action \
         could not reach them anyway"
    );

    // And a focused plugin pane resolves in the global scope, so it sees neither.
    let focus_ctx = method_body(&root, "src/app/key_handlers.rs", "fn focus_key_context");
    assert!(
        !focus_ctx.contains("PluginPane"),
        "`focus_key_context` now names a plugin pane — re-verdict the keyboard row and check \
         which scope it lands in: {focus_ctx}"
    );

    // The one review key that *is* an action is the toggle that opens the view, which
    // is not one of the pane's own keys — stated so the row cannot be read as "the
    // review has no actions at all".
    let actions = source(&root, "src/session/keybindings.rs");
    assert!(
        actions.contains("Action::ToggleReview"),
        "opening the review is a bindable action, unlike anything inside it"
    );
}

/// **The third finding.** The mouse channel carries a row; one of this pane's clicks
/// means a *column*.
///
/// The distinction is the point: buttons, a scrollbar, a wheel and picker entries are
/// missing target **kinds**, and a wider event closes them. `click_side` is a missing
/// **coordinate** — "the user clicked the old half of this row" is not a row, so no
/// extra variant expresses it.
#[test]
fn the_mouse_channel_carries_a_row_where_the_pane_needs_a_column() {
    let root = repo_root();

    // What a plugin gets.
    assert!(
        source(&root, "src/plugin/runtime.rs")
            .contains("fn on_click(&self, pane_id: &str, row: usize)"),
        "a plugin pane's click should carry a pane and a row"
    );
    assert!(
        source(&root, "src/app/view.rs").contains("ClickAction::PluginPaneRow"),
        "a plugin pane's rows should be recorded as click targets"
    );

    // The target kinds the pane has and the channel does not.
    let buttons = variant_names(&block(
        &root,
        "src/app/code_review.rs",
        "pub(crate) enum ReviewButton",
    ));
    assert!(
        buttons.len() >= 10,
        "the review's footer should carry a bar of buttons a plugin pane cannot receive: \
         {buttons:?}"
    );
    for target in ["ClickAction::ReviewTarget", "ScrollTarget::"] {
        assert!(
            source(&root, "src/app/mod.rs").contains(target),
            "the review should have a `{target}` a plugin pane has no equivalent of"
        );
    }

    // And the coordinate, which is the half that does not close by widening variants.
    let state = source(&root, "src/app/code_review.rs");
    assert!(
        state.contains("click_side"),
        "a click's side should steer which half of a paired row a comment anchors to"
    );
    assert!(
        state.contains("fn cr_click_row(&mut self, idx: usize, rel_x: u16, width: u16)"),
        "the click handler should take the click's x and the pane's width, which is the \
         coordinate a row index cannot carry"
    );
}

/// **The row where this pane is closer than the session list**, which a gate that only
/// repeated "the cursor is kernel state" would hide.
///
/// The session list's cursor is the application's active session — the thing the
/// central pane, the info panel, the file viewer and the review are all showing — so
/// writing it is the widest grant in the host. This pane's cursor is a row inside a
/// view the user already opened, read by the review and by the changed-files
/// highlight and by nothing else. So a narrow "name the row you are on" capability is
/// the cheapest first step, and this test is what keeps that ordering true.
#[test]
fn the_review_cursor_is_a_narrower_grant_than_the_session_lists() {
    let root = repo_root();

    // The session list's cursor is the active session, switched by the app itself.
    let app = source(&root, "src/app/mod.rs");
    assert!(
        app.contains("active_index"),
        "the session list's cursor should be the app's active session index"
    );

    // The review's cursor is a field of the review's own state, and what reads it is
    // the review: its rows, its published section, and the changed-files highlight.
    let review = source(&root, "src/app/code_review.rs");
    assert!(
        review.contains("pub selected: usize"),
        "the review's cursor should be its own state"
    );
    assert!(
        review.contains("fn current_file(&self)"),
        "the changed-files highlight should be derived from that cursor, which is the one reader \
         outside the diff"
    );
    assert!(
        review.contains("fn snapshot_rows(&self)"),
        "the published section should carry that cursor, which is what a plugin reads"
    );

    // Neither is writable, which is the row itself.
    let methods = writer_methods(&root);
    assert!(
        !methods
            .iter()
            .any(|m| ["cursor", "select"].iter().any(|n| m.contains(n))),
        "a cursor write now exists — re-verdict `no-cursor-write` here and in the session list's \
         gate, and check which of the two it covers: {methods:?}"
    );
}

/// The pane the interface draws is still the native one, which is what makes every row
/// above a statement about a handover that has not happened.
#[test]
fn the_native_pane_is_still_what_thurbox_draws() {
    let root = repo_root();
    let view = source(&root, "src/app/view.rs");
    assert!(
        view.contains("code_review::render(") && view.contains("render_files_list("),
        "`src/app/view.rs` no longer draws the native review — if the pane was handed over, this \
         gate's rows are the record of what that cost"
    );
    // And the bundled plugin's pane is still off by default, so the reproduction is
    // not on screen beside the pane it reproduces.
    let manifest = source(&root, "src/plugin/bundled/code-review/plugin.toml");
    assert!(
        manifest.contains("default_visible = false"),
        "the reproduction must stay hidden while the native pane is what users see"
    );
    // The port that *did* land is read-only, and the refusal added nothing to it —
    // stated here so a failure distinguishes "the port grew" from "the handover
    // happened".
    assert!(
        manifest.contains("capabilities = [\"render\", \"review\"]"),
        "the reproduction should still declare exactly the two capabilities its document needs: \
         a refusal that granted a capability would be a capability with no consumer"
    );
    for refused in ["input", "tasks-write", "automations-write"] {
        assert!(
            !manifest.contains(&format!("\"{refused}\"")),
            "the plugin declares `{refused}` — a pane that takes a power it cannot act with is \
             worse than one that takes none (ADR-38)"
        );
    }
}
