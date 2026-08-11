//! The automations pane's **handover** verdict, enforced as a test.
//!
//! `docs/PHASE4-PANE-READINESS.md` §17 records that this pane's rendering *and*
//! five of its seven keys are reproduced by a bundled plugin (ADR-41) — the
//! furthest any port has got, and the first to declare `input` and
//! `automations-write`. §18 records the next step's answer: the pane is **not**
//! handed over, and this file is that half of the verdict in executable form.
//! [`BLOCKERS`] holds one row per requirement the handover could not meet, each
//! re-derived from the source.
//!
//! Why a test rather than only the document, in `tests/global_search_pane_gap.rs`'s
//! words: a verdict written in markdown is a fact about a build that expires
//! without telling anyone.
//!
//! **The finding this gate exists to keep true** is
//! [`the_central_pane_follows_the_native_panes_focus_not_the_panes_identity`].
//! §17 stopped at the pane's seat and its keys, and the seat is the row a reader
//! expects to decide this handover. It does not. Focusing the *native* pane is what
//! turns the **central** pane into the automation editor plus its run history, and
//! that branch tests `self.focus` against three native focuses. A pane handed over
//! **the way this plugin is built** is focused as `InputFocus::PluginPane`, so the
//! branch never fires: the editor, the run history and
//! `Enter`-opens-that-run's-session all disappear from a pane that still looks
//! complete. It is the same coupling that blocked the tasks pane (§15), found here
//! on the pane whose keys were supposed to be the hard part — and it means the two
//! unported keys are not a five-sevenths shortfall but the pane's whole authoring
//! surface.
//!
//! ## Which route this gate measures, since ADR-51 there are two
//!
//! Every row below is about a pane whose **keys are the plugin's**: it declares
//! `input`, receives each chord addressed by binding (ADR-34) and acts through the
//! capabilities it was granted. That is what the bundled `automations` plugin is —
//! five declared bindings, `automations-write` — and it is the port §17 measured.
//!
//! ADR-51 added a second route: a pane may declare that it **is** thurbox's pane
//! for a key context, and then the kernel resolves that context's actions and
//! performs them itself while the pane holds focus, as the kernel's own pane of
//! that name. A pane taking that route would answer all seven of this pane's keys,
//! including the two that author, and would open the central editor — because it is
//! focused as `InputFocus::Automations`, which the branch above *does* name. The
//! manifest refuses both routes in one pane, so this is a fork rather than a
//! supplement, and this plugin has not taken it.
//!
//! So a blocked row here says "this plugin's port cannot do X", never "this pane
//! cannot be handed over". Where the second route changes a row's answer the row
//! says so; where it does not — the module that is also the kernel's model, the
//! drawing vocabulary — nothing about it depends on which route the keys took.
//!
//! Three things this gate is deliberately not:
//!
//! - it is **not** the teardown gate, which answers whether
//!   `src/ui/automations_panel.rs` may be deleted — already no, and no either way.
//!   One table answering two questions produces failures that do not say which
//!   question moved;
//! - it is **not** a claim that the pane's rendering or its ported keys fall short.
//!   `tests/bundled_automations_panel.rs` asserts the plugin builds the identical
//!   view tree, paints the identical frame when the pane scrolls, and reaches the
//!   same database rows its five keys do;
//! - it is **not** the session list's gate. That pane's wall is its **keys** — its
//!   cursor is the active session. This pane's keys are five-sevenths ported and
//!   its wall is its **seat** — of the two columns it needs, the left column it is
//!   drawn in is now a slot a manifest can name (ADR-46), and the central pane it
//!   drives is still opened by the *native* pane's focus rather than by any pane's
//!   identity.
//!
//! Its probes read the source the way a human auditor would, so the gate runs, and
//! means the same thing, with or without the `plugins` Cargo feature. The helpers
//! below duplicate the sibling gates', because an integration test cannot import
//! another one — the alternative is a shared crate for five readers, which is more
//! machinery than the duplication costs.

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
    /// plugin is asked to render, which facts it is told, or how the host draws a
    /// pane it already knows is focused. Cheapest to close, and filed apart from
    /// the other two because calling it structural would claim the model forbids
    /// it — it does not.
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
        id: "central-seat-follows-the-native-focus",
        needs: "the central pane the pane drives: focusing it shows the selected automation's \
                editor and, one `Ctrl+L` further, its run history — where `r` reruns and `Enter` \
                opens the session that run touched",
        stands: "`App::render_central_pane` picks that view by testing `self.focus` against three \
                 **native** focuses (`Automations`, `AutomationEditor`, `AutomationRunHistory`). A \
                 pane handed over on this plugin's route — its own keys, via `input` — is focused \
                 as `InputFocus::PluginPane`, which the branch does not name, so the editor and \
                 the run history never appear. This is the row that decides *that* handover: the \
                 two unported keys are not a shortfall of two, they are the pane's whole authoring \
                 surface, and the same central-seat coupling blocked the tasks pane (ADR-38). It \
                 is **not** a wall for the other route (ADR-51): a pane declaring this keyboard is \
                 focused as `InputFocus::Automations`, which the branch names, so the editor and \
                 the run history appear for it. Recorded outstanding because the shipped plugin \
                 declares bindings of its own and the manifest refuses both at once",
        gap: Gap::Structural,
        blocked: true,
        probe: |root| {
            let central = method_body(root, "src/app/view.rs", "fn render_central_pane");
            let branches_on_native_focus = [
                "InputFocus::Automations",
                "InputFocus::AutomationEditor",
                "InputFocus::AutomationRunHistory",
            ]
            .iter()
            .all(|f| central.contains(f));
            // And says nothing about a plugin pane, so the branch cannot fire for
            // one.
            branches_on_native_focus && !central.contains("PluginPane")
        },
    },
    Blocker {
        id: "no-left-seat",
        needs:
            "the pane's seat: the native pane is the **bottom** of the left column, beneath the \
                session list, and its height grows with the automation count",
        stands: "**closed** (ADR-46). `PaneSlot::LeftBottom` names `RegionId::Automations` — the \
                 region the native pane occupies — and a visible pane claiming it is drawn into \
                 that rect while `App::render_automations_pane` stands down. The load-bearing \
                 half was the height, since the left column is the one place in thurbox where a \
                 pane's geometry follows from its own content: the kernel keeps its \
                 `(count + 2).clamp(3, 10)` policy and reads the count off the seated pane's \
                 tree (`ViewNode::stacked_row_count`), so a plugin still learns nothing about \
                 its size. The reproduction ships in this seat",
        gap: Gap::Structural,
        blocked: false,
        probe: |root| {
            let seats = method_body(root, "src/session/plugin_manifest.rs", "pub fn seat(");
            let seat_exists = seats.contains("Some(RegionId::Automations)");
            // The seat is only real if the kernel steps aside for it.
            let native_stands_down =
                method_body(root, "src/app/view.rs", "fn render_automations_pane")
                    .contains("seat_taken(PaneSlot::LeftBottom)");
            // And the height half: the kernel's own policy still sizes the band,
            // with the count coming off the seated pane's tree rather than out of
            // the automation list. `block` rather than `method_body` because
            // `left_column` is a free function, so rustfmt closes it in column zero.
            let column = block(root, "src/ui/layout.rs", "fn left_column(");
            let policy_kept = column.contains("AUTOMATIONS_PANE_MIN_ROWS")
                && column.contains("RegionId::Automations")
                && column.contains("RegionId::SessionList");
            let count_from_the_pane = method_body(root, "src/app/mod.rs", "fn lower_left_rows(")
                .contains("stacked_row_count");
            !(seat_exists && native_stands_down && policy_kept && count_from_the_pane)
        },
    },
    Blocker {
        id: "no-creation-operation",
        needs: "`n`, which creates an automation from the pane",
        stands: "the mutation seam's five operations set an automation's enabled flag, mark one \
                 due, delete one, and set or delete a task — none creates a record. That is \
                 ADR-35 by construction rather than an omission: creation has no id to address, \
                 so a grant to create is a grant to add rows without bound",
        gap: Gap::Structural,
        blocked: true,
        probe: |root| {
            let methods = writer_methods(root);
            !methods.iter().any(|m| {
                ["create", "insert", "new_", "add_"]
                    .iter()
                    .any(|n| m.contains(n))
            })
        },
    },
    Blocker {
        id: "no-authoring-operation",
        needs: "`Enter`/`e`, which edits the selected automation's name, schedule, action and \
                prompt",
        stands: "every seam operation addresses a record by id and writes one flag; none writes a \
                 field the user typed. `automations-write` is defined to exclude text authoring, \
                 so this is refused twice over — no operation, and no seat to type into (see \
                 `central-seat-follows-the-native-focus`)",
        gap: Gap::Structural,
        blocked: true,
        probe: |root| {
            let methods = writer_methods(root);
            !methods.iter().any(|m| {
                ["name", "prompt", "schedule", "command", "title", "body"]
                    .iter()
                    .any(|n| m.contains(n))
            })
        },
    },
    Blocker {
        id: "wrap-out-of-the-pane-is-unowned",
        needs: "the circular coupling with the session list — `k` at the first automation hands \
                focus back to the last session, `j` past the last automation loops to the top of \
                the session list",
        stands: "the plugin does the half it can, declining the key at either edge (ADR-41). The \
                 kernel's half does not exist: an unconsumed key from a focused plugin pane \
                 resolves in `KeyContext::Global`, and no action means \"leave this pane \
                 downward\" — `Esc` leaves, but not directionally. A wrap is a claim about \
                 adjacency and adjacency is layout, so this row cannot close before `no-left-seat` \
                 does",
        gap: Gap::Structural,
        blocked: true,
        probe: |root| {
            let resolver = method_body(
                root,
                "src/app/key_handlers.rs",
                "pub(crate) fn focus_key_context",
            );
            let falls_through_to_global =
                !resolver.contains("PluginPane") && resolver.contains("_ => KeyContext::Global");
            // And no action spells a directional exit from a pane.
            let actions = block(root, "src/session/keybindings.rs", "pub enum Action");
            let no_directional_leave = !actions.to_lowercase().contains("leave");
            falls_through_to_global && no_directional_leave
        },
    },
    Blocker {
        id: "the-module-is-a-model-too",
        needs: "deleting `src/ui/automations_panel.rs`, which is what a handover is for",
        stands: "that module is not only the pane's renderer. `row_summary` composes the row's \
                 tail for this pane **and** for the `Ctrl+P` list modal — `src/app/automation.rs` \
                 calls it — so the two native surfaces cannot disagree. Deleting the module \
                 deletes the modal's summary. It used to own the pane's **width** step too, and \
                 no longer does (ADR-55): `resolve_rows` takes no geometry, which narrows this row \
                 to the one function that has a second consumer. The file viewer's gate found the \
                 same class (ADR-39)",
        gap: Gap::Structural,
        blocked: true,
        probe: |root| {
            let modal_path = source(root, "src/app/automation.rs")
                .contains("crate::ui::automations_panel::row_summary");
            let module = source(root, "src/ui/automations_panel.rs");
            modal_path
                && module.contains("pub fn row_summary")
                && module.contains("pub fn resolve_rows")
        },
    },
    Blocker {
        id: "pane-is-not-told-its-own-focus",
        needs: "drawing the cursor only while focused, and acting on the row the user can see: \
                the native pane highlights its selection when focused and not otherwise",
        stands: "`render(paneId)` is told nothing about its pane's focus — every published \
                 `focused` field describes the *native* surface being reproduced. So the plugin \
                 draws its own cursor whether or not its pane is focused, cannot learn that focus \
                 left, and **refuses a write** until it has drawn a cursor of its own, because \
                 otherwise `Space` would toggle a row the user cannot see (ADR-41). The mechanism \
                 exists: `session::pane_visibility` publishes a per-pane boolean into a \
                 process-wide slot the render worker already reads, and a focus sibling is the \
                 same shape",
        gap: Gap::Wiring,
        blocked: true,
        probe: |root| {
            let published = source(root, "src/session/pane_visibility.rs");
            let publishes_no_focus =
                !published.contains("fn focused") && !published.contains("focus(");
            let no_focus_module = !root.join("src/session/pane_focus.rs").exists();
            publishes_no_focus && no_focus_module
        },
    },
    Blocker {
        id: "focused-pane-draws-an-unfocused-border",
        needs: "the pane's focused border — the native pane switches to the accent frame while it \
                holds focus, which is how a user sees where the keyboard is",
        stands: "**closed** (ADR-51). `App::paint_plugin_pane` built every pane's block at \
                 `FocusLevel::Inactive` unconditionally; it now takes the level as an argument and \
                 `App::plugin_pane_focus_level` resolves it from the focus the kernel owns — the \
                 pane's own rule when it declared one of thurbox's keyboards, otherwise focused \
                 while it is the pane holding `InputFocus::PluginPane`. Nothing is published to \
                 the plugin: a frame is the host's, which is why closing this did not close \
                 `pane-is-not-told-its-own-focus` above",
        gap: Gap::Wiring,
        blocked: false,
        probe: |root| {
            // Two halves, because either alone would be the wrong claim: the painter
            // must accept a level rather than hard-coding one, and the caller must
            // resolve it from a focus. A probe reading only the first would report
            // this closed for a painter handed `Inactive` at every call site.
            let paint = method_body(root, "src/app/view.rs", "fn paint_plugin_pane");
            let painter_takes_a_level = paint.contains("focus: crate::ui::FocusLevel")
                && !paint.contains("focus_block(&title, FocusLevel::Inactive)");
            let caller_resolves_it =
                method_body(root, "src/app/view.rs", "fn plugin_pane_focus_level")
                    .contains("InputFocus::PluginPane");
            !(painter_takes_a_level && caller_resolves_it)
        },
    },
    Blocker {
        id: "render-is-not-event-driven",
        needs: "the pane reflecting a change within one of the interface's own forced frames — a \
                countdown that ticks, and a row whose enabled marker flips when a key wrote it",
        stands: "**closed** (ADR-49), and the row's wording had to change with it — recorded here \
                 rather than quietly. The trigger is now the change: the publisher tells the \
                 worker which *sources* moved and it renders the panes that read them, so a \
                 countdown ticks because the automations source moved and a written marker \
                 re-renders on the same publication. A key answered by the plugin also re-renders \
                 its pane, whether or not the plugin claimed it, since answering may have moved \
                 state only the plugin can see. What is left is a rate ceiling of 100 ms, which \
                 coalesces rather than delays. The **original** wording asked for the frame the \
                 change happened in, and that is unreachable by construction: \
                 `plugin-host/panes` forbids the kernel calling a plugin during a frame, so a \
                 plugin tree is always produced off the drawing thread. 100 ms is inside the \
                 kernel's own 250 ms forced-redraw floor",
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
        id: "no-fitted-name",
        needs: "the row's name fitted to the column with an ellipsis, so a long name never pushes \
                the schedule-and-action tail off the pane",
        stands: "**closed** (ADR-55). The *vocabulary* closed first with ADR-52 — a run may \
                 declare that it yields its width and the renderer fits the group with an \
                 ellipsis — and this pane has now adopted it, in the two halves that had to land \
                 together: the runs carrying the name declare `ellipsize`, **and** the native \
                 pane stopped fitting (`resolve_rows` takes no width at all). Only the second \
                 half makes the trees equal — a pane that keeps cutting the string itself while \
                 its reproduction declares the fit produces trees that differ by construction, \
                 which is why the probe reads the *native* pane rather than the catalogue. The \
                 oracle's enumerated divergence is replaced by its opposite: at the pane's real \
                 width both panes paint one frame, ellipsis and summary tail included",
        gap: Gap::Vocabulary,
        blocked: false,
        probe: |root| {
            // Both halves, because either alone would be the wrong claim. A probe
            // reading only the declaration would report this closed for a pane whose
            // tree still carries an already-cut name; a probe reading only the
            // native pane would report it closed for a reproduction that never
            // declared the fit and simply clips.
            let module = source(root, "src/ui/automations_panel.rs");
            // Scoped to `resolve_rows` rather than to the file, because the file still
            // *contains* a fit: the retained pre-port oracle (`legacy_line`) cuts the
            // name itself, and that is what makes it the oracle for the fit the
            // renderer now applies. `block` rather than `method_body` — a free
            // function is closed in column zero.
            let resolve = block(root, "src/ui/automations_panel.rs", "pub fn resolve_rows(");
            let native_stopped_fitting =
                !resolve.contains("truncate_ellipsis") && !resolve.contains("width");
            let native_declares_it = module.contains("ellipsize: true");
            let plugin_declares_it = source(root, "src/plugin/bundled/automations/init.luau")
                .contains("ellipsize = true");
            !(native_stopped_fitting && native_declares_it && plugin_declares_it)
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

/// The rows outstanding of one kind — which is how the ordering of the work is read
/// off the table rather than argued in prose.
fn outstanding(blockers: &[Blocker], gap: Gap) -> Vec<&'static str> {
    blockers
        .iter()
        .filter(|b| b.blocked && b.gap == gap)
        .map(|b| b.id)
        .collect()
}

/// The `Action` names scoped to the automations pane — the keyboard a handover
/// would silence, and the set the port measured itself against.
fn automations_actions(root: &Path) -> Vec<String> {
    let body = method_body(root, "src/session/keybindings.rs", "pub fn context(self)");
    let end = body
        .find("KeyContext::Automations")
        .unwrap_or_else(|| panic!("`Action::context` no longer scopes anything to the pane"));
    // Back up to the start of this arm: the previous arm's `=>` ends before it.
    let arm_start = body[..end]
        .rfind("KeyContext::")
        .map(|i| i + "KeyContext::".len())
        .unwrap_or(0);
    body[arm_start..end]
        .split("Action::")
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

/// The **signatures** of the mutation seam's methods, lowercased.
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
        methods.iter().any(|m| m.contains("set_automation_enabled")),
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
        "the automations pane's handover verdict disagrees with the source tree.\n\
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
        "every requirement is recorded met — the automations pane is portable, so hand it over \
         deliberately (and retire this gate) rather than leaving it passing vacuously"
    );
    // The ordering the table implies, asserted rather than described. **No drawing
    // row is outstanding** since ADR-55 — which is the state the tasks pane's gate
    // reached before its handover, and the honest summary of this pane: what is left
    // is not a thing the catalogue cannot say.
    assert!(
        outstanding(BLOCKERS, Gap::Vocabulary).is_empty(),
        "the fitted name was this pane's last drawing gap; a new one means the table \
         grew a row that should be named here: {:?}",
        outstanding(BLOCKERS, Gap::Vocabulary)
    );
    assert!(!outstanding(BLOCKERS, Gap::Wiring).is_empty());
    let structural = outstanding(BLOCKERS, Gap::Structural);
    // The pane's own seat closed with ADR-46; what still decides the verdict is
    // the **central** seat, which the native pane's focus — not the pane's
    // identity — is what opens.
    assert!(
        structural.contains(&"central-seat-follows-the-native-focus"),
        "the central seat is what decides this verdict, and it is structural: {structural:?}"
    );
    assert!(
        !structural.contains(&"no-left-seat"),
        "the pane's own seat is closed (ADR-46) — a table that records it outstanding again means \
         the seating regressed: {structural:?}"
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

/// **The finding.** The central pane's mode follows a *focus*, not a pane's
/// identity — so handing this pane to a plugin that keeps its own keys removes the
/// automation editor and its run history, surfaces the pane itself does not draw.
///
/// This is not a restatement of the probe. It names the three focuses the branch
/// tests and the two things they open, so a failure says *which* surface the
/// handover would have removed, and it pins that the branch is a `self.focus` test.
///
/// The second half is the correction ADR-51 forced, and it is why the finding is
/// about a *focus* rather than about plugins: a pane that declares this keyboard is
/// focused as `InputFocus::Automations`, which the branch names, so the editor does
/// appear for it. Asserted here so the two routes cannot be conflated — the wall is
/// "focused as `PluginPane`", not "drawn by a plugin".
#[test]
fn the_central_pane_follows_the_native_panes_focus_not_the_panes_identity() {
    let root = repo_root();
    let central = method_body(&root, "src/app/view.rs", "fn render_central_pane");

    for focus in [
        "InputFocus::Automations",
        "InputFocus::AutomationEditor",
        "InputFocus::AutomationRunHistory",
    ] {
        assert!(
            central.contains(focus),
            "the central pane's automation branch should test `{focus}`: {central}"
        );
    }
    assert!(
        central.contains("render_automation_workspace"),
        "the branch should open the editor-plus-run-history workspace"
    );
    assert!(
        !central.contains("PluginPane"),
        "the central pane now names a plugin pane — if a plugin pane can drive the central seat, \
         re-verdict `central-seat-follows-the-native-focus` and check what the pane gains"
    );
    // The other route reaches the same branch, because it reaches the same focus.
    // Read from the host's own table so this cannot drift from what focus a declared
    // keyboard is delivered to (ADR-51).
    let table = method_body(&root, "src/app/mod.rs", "pub(crate) fn focus_for_keyboard");
    assert!(
        table.contains("KeyContext::Automations => Some(InputFocus::Automations)"),
        "a pane declaring this keyboard should be focused as the kernel's own automations pane, \
         which is what makes the branch above fire for it: {table}"
    );
    // The ring the branch serves is the automations one, so the run history is a
    // stop a handed-over pane could not reach either.
    let ring = method_body(&root, "src/app/key_handlers.rs", "fn focus_ring");
    assert!(
        ring.contains("AutomationRunHistory"),
        "the run history should be a stop in the automations focus ring: {ring}"
    );
}

/// Two of the pane's seven scoped actions are unported, and they are the two that
/// author — which is what makes "five of seven" the wrong way to read the shortfall.
///
/// Pinned separately from the table because it is the sentence a future attempt will
/// want to argue with: the port shipped keys, so "a plugin pane's keys can act" is
/// true. What is not true is that the pane's *authoring* keys can.
#[test]
fn the_two_unported_keys_are_the_panes_whole_authoring_surface() {
    let root = repo_root();

    let actions = automations_actions(&root);
    for expected in [
        "AutomationsNew",
        "AutomationsNext",
        "AutomationsPrev",
        "AutomationsOpen",
        "AutomationsToggle",
        "AutomationsRun",
        "AutomationsDelete",
    ] {
        assert!(
            actions.iter().any(|a| a == expected),
            "`{expected}` should be scoped to the automations pane: {actions:?}"
        );
    }

    // The five the plugin declares, read from its manifest so the claim is about
    // the shipped plugin rather than about this test's memory.
    let manifest = source(&root, "src/plugin/bundled/automations/plugin.toml");
    for ported in ["\"j\"", "\"k\"", "\"space\"", "\"r\"", "\"d\""] {
        assert!(
            manifest.contains(ported),
            "the plugin should declare a binding on {ported}: {manifest}"
        );
    }
    // And the two it does not: creating and opening the editor. `n` and `e` are
    // the native chords, so neither may appear as a declared chord.
    for unported in ["chord = \"n\"", "chord = \"e\"", "chord = \"enter\""] {
        assert!(
            !manifest.contains(unported),
            "the plugin declares `{unported}` — a pane that takes an authoring key it cannot \
             act with is worse than one that takes none (ADR-38), so re-verdict \
             `no-creation-operation` / `no-authoring-operation`"
        );
    }

    // Both refusals come from the same place: the seam writes flags, never fields.
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
}

/// The pane the interface draws is still the native one, which is what makes every
/// row above a statement about a handover that has not happened.
#[test]
fn the_native_pane_is_still_what_thurbox_draws() {
    let root = repo_root();
    assert!(
        source(&root, "src/app/view.rs").contains("automations_panel::render_automations_pane"),
        "`src/app/view.rs` no longer draws the native automations pane — if the pane was handed \
         over, this gate's rows are the record of what that cost"
    );
    // And the bundled plugin's pane is still off by default, so the reproduction is
    // not on screen beside the pane it reproduces.
    let manifest = source(&root, "src/plugin/bundled/automations/plugin.toml");
    assert!(
        manifest.contains("default_visible = false"),
        "the reproduction must stay hidden while the native pane is what users see"
    );
    // It does declare the keys and the write, which is the part of the port that
    // did land — stated here so a failure distinguishes "the port regressed" from
    // "the handover happened".
    assert!(
        manifest.contains("\"input\"") && manifest.contains("\"automations-write\""),
        "the reproduction should still declare the capabilities its five ported keys need"
    );
}
