//! The file viewer's **handover** verdict, enforced as a test.
//!
//! `docs/PHASE4-PANE-READINESS.md` §16 records that the file viewer's *rendering* is
//! reproduced by a bundled plugin, scroll track included, and §29 records the next
//! step's answer: the pane is **not** handed over. This file is that half of the
//! verdict in executable form — [`BLOCKERS`] holds one row per thing the handover
//! still needs, each re-derived from the source.
//!
//! Why a test rather than only the document, in `tests/global_search_pane_gap.rs`'s
//! words: a verdict written in markdown is a fact about a build that expires
//! without telling anyone. This file is the proof of that claim rather than an
//! illustration of it — every row below was true when written, five of them stopped
//! being requirements, and nothing but a probe would have said so.
//!
//! ## The finding: the capability this pane was expected to need is **none**
//!
//! It began as an *input* gate. All seven of the pane's `KeyContext::FileViewer`
//! actions write view state, and two of them need powers the vocabulary does not
//! define — expanding a directory **reads the filesystem**, expanding a file
//! **launches a process** — so ADR-39 refused the grant and the rows recorded the
//! shortfall. The brief for the handover asked for the *minimum* widening of
//! `Capability::Files` that would close them.
//!
//! ADR-51 answered a different question. A pane may declare that it **is** thurbox's
//! file viewer, and the kernel then resolves those seven actions and performs them
//! itself: `FileViewerState::activate` keeps doing the read, `App::file_viewer_expand`
//! keeps launching the editor, and the plugin draws. So five rows close — and the three
//! that named a grant now assert that the grant is **still absent**, because "the
//! widening was unnecessary" and "the widening happened" are indistinguishable in a
//! table that stopped looking.
//!
//! What is left is three decisions, none of them a capability, and
//! [`the_verdict_is_derived_from_the_blockers`] asserts exactly that: **nothing
//! outstanding is structural**.
//!
//! 1. the **seat** — `PaneSlot` names none for this column's first occupant;
//! 2. the **module** — `src/ui/file_viewer.rs` is the pane's *model* and the home of
//!    `visible_window`, the rule every plugin list scrolls by;
//! 3. the **column's second kernel occupant** — the code review's changed-files list,
//!    which ADR-45 records as wanting this exact region, so the seat is the first where
//!    a plugin claim must *not* simply win.
//!
//! Three things this gate is deliberately not:
//!
//! - it is **not** the teardown gate, which answers whether `src/ui/file_viewer.rs`
//!   may be deleted — still no, and the row there stays blocked;
//! - it is **not** a claim that the pane's rendering is inexpressible. The plugin
//!   builds the native pane's view tree and paints its frame, track included
//!   (`tests/bundled_file_viewer.rs`);
//! - it is **not** a claim that the pane cannot be handed over. It can; three
//!   decisions have not been taken.
//!
//! Its probes read the source the way a human auditor would, so this gate runs, and
//! means the same thing, with or without the `plugins` Cargo feature. The helpers
//! below duplicate the sibling gates', because an integration test cannot import
//! another one — the alternative is a shared crate for four readers, which is more
//! machinery than the duplication costs.

use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

/// Whether the pane model withholds a power **by design**, or merely has not
/// spelled it yet.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Gap {
    /// A power a plugin pane is not given, on purpose. Reversing it changes what
    /// a plugin *is*, so no further node or token closes it.
    Structural,
    /// Something today's catalogue cannot say and could.
    Vocabulary,
}

/// One host power the file viewer's keys need and do not have.
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

/// Every power the pane's seven `KeyContext::FileViewer` actions and its search
/// sub-mode would need, structural rows first.
///
/// Each probe is scoped to the *declaration* it reads rather than to a whole file,
/// copied from `tests/teardown_gate.rs` for its reason: an unrelated mention
/// elsewhere in a file must not flip a verdict.
const BLOCKERS: &[Blocker] = &[
    Blocker {
        id: "no-file-viewer-seat",
        needs: "the pane's seat: the right column's **first** occupant when no review is \
                open, which is where the native pane draws",
        stands: "`PaneSlot::seat()` names `RegionId::Tasks` (ADR-53) and not \
                 `RegionId::FileViewer`. Adding it is one line of the same table — what makes \
                 it a decision rather than a chore is the row below: this seat has a *second* \
                 kernel occupant, so ADR-46's rule (a visible plugin pane takes the seat) is \
                 the wrong rule here and the right one is not written",
        gap: Gap::Vocabulary,
        blocked: true,
        probe: |root| {
            !method_body(root, "src/session/plugin_manifest.rs", "pub fn seat(")
                .contains("Some(RegionId::FileViewer)")
        },
    },
    Blocker {
        id: "the-column-has-a-second-kernel-occupant",
        needs: "a rule for what a claim does while a **code review** owns this column: the \
                review's changed-files list is force-shown into it, with its own focus \
                (`InputFocus::ReviewFiles`) and its own keys",
        stands: "`App::layout_for` forces the column present for an open review and \
                 `App::render_file_viewer` draws the review's list into it instead of the \
                 viewer. ADR-45 records that list as wanting `RegionId::FileViewer` \
                 *specifically*, which is why ADR-46 gave a plugin no second seat for it. So \
                 the seat has two kernel occupants and ADR-46's precedence rule would hand the \
                 column to the plugin while the review needed it — the first seat where the \
                 plugin must **not** simply win",
        gap: Gap::Vocabulary,
        blocked: true,
        probe: |root| {
            let forced = method_body(root, "src/app/mod.rs", "pub(crate) fn layout_for")
                .contains("self.active_review().is_some()");
            let drawn = method_body(root, "src/app/view.rs", "fn render_file_viewer")
                .contains("render_files_list");
            forced && drawn
        },
    },
    Blocker {
        id: "the-module-is-the-model-and-the-window",
        needs: "deleting `src/ui/file_viewer.rs`, which is what a handover is for",
        stands: "that module is not the pane's renderer. It is the pane's **model** — \
                 `FileNode`, `FileRow`, `Activation`, `FileViewerState` (the expansion set, \
                 the cursor, the search, the directory reads) and `enumerate_paths`, which \
                 `App` owns and the published section is derived from — **and** the home of \
                 `visible_window`, the rule every *plugin* list and three native panes scroll \
                 by (ADR-30). Deleting the renderer means relocating both: the model into \
                 `app`, the window into a `ui` home, at five call sites in four modules. \
                 ADR-39 called that motion without a destination; the destination exists now, \
                 and the move belongs in the change that uses it",
        gap: Gap::Vocabulary,
        blocked: true,
        probe: |root| {
            let module = source(root, "src/ui/file_viewer.rs");
            let is_the_model = module.contains("pub struct FileViewerState")
                && source(root, "src/app/mod.rs")
                    .contains("crate::ui::file_viewer::FileViewerState");
            let is_the_window = module.contains("fn visible_window")
                && source(root, "src/ui/plugin_pane.rs").contains("file_viewer::visible_window");
            is_the_model && is_the_window
        },
    },
    Blocker {
        id: "no-frame-node",
        needs: "the search bar: a three-row bordered ` Search (2/5) ` block below the tree, \
                with a `/ ` prefix, the query scrolled to its end, and a block cursor",
        stands: "it is **kernel chrome**, not a node — the query, the caret and the match \
                 counter are all kernel state, and a plugin that redrew them would be a second \
                 renderer for one fact (the argument ADR-53 made for the tasks pane's hint \
                 row). That mechanism now exists and is **one row deep**: `App::pane_hints` \
                 names a hint line and `paint_plugin_pane` reserves a single row for it. Three \
                 rows, a border and a cursor cell is the same mechanism widened, and it is not \
                 widened yet",
        gap: Gap::Vocabulary,
        blocked: true,
        probe: |root| {
            // Two halves: the chrome hook exists (so this is a widening rather than an
            // invention), and it is still a single row (so the bar does not fit).
            let chrome_exists = method_body(root, "src/app/mod.rs", "pub(crate) fn pane_hints")
                .contains("KeyContext::Tasks");
            let one_row_only = method_body(root, "src/app/view.rs", "fn paint_plugin_pane")
                .contains("inner.height -= 1");
            chrome_exists && one_row_only
        },
    },
    Blocker {
        id: "no-view-write",
        needs: "every one of the pane's seven keys — moving the cursor (`j`/`k`), collapsing \
                (`h`), expanding (`l`/`Enter`), starting a search (`/`) and stepping matches \
                (`n`/`N`) all write the cursor, the expansion set, or both",
        stands: "**closed as a handover requirement** (ADR-51), and *not* by granting the \
                 write: no binding writes view state, and none is needed. A pane declaring \
                 `key_context = \"FileViewer\"` is focused as `InputFocus::FileViewer`, so the \
                 kernel resolves those seven actions and performs them against \
                 `App::file_viewer` exactly as it does today. The plugin draws",
        gap: Gap::Structural,
        blocked: false,
        probe: |root| {
            // Three halves, because any one alone would be the wrong claim: the context
            // must be declarable, it must map to this pane's focus, and the kernel must
            // still resolve that focus to this context. And the write is still absent —
            // asserted here so "closed" can never come to mean "granted".
            let declarable =
                method_body(root, "src/session/keybindings.rs", "pub fn pane_keyboards(")
                    .contains("KeyContext::FileViewer");
            let maps_to_the_focus =
                method_body(root, "src/app/mod.rs", "pub(crate) fn focus_for_keyboard")
                    .contains("KeyContext::FileViewer => Some(InputFocus::FileViewer)");
            let kernel_still_dispatches =
                method_body(root, "src/app/key_handlers.rs", "fn focus_key_context")
                    .contains("KeyContext::FileViewer");
            let no_write_was_granted = !a_view_write_binding_exists(root);
            !(declarable && maps_to_the_focus && kernel_still_dispatches && no_write_was_granted)
        },
    },
    Blocker {
        id: "no-filesystem-read",
        needs: "filling a directory the first time it is expanded (`l`/`Enter` on a folder), \
                which the kernel does by reading it",
        stands: "**closed as a handover requirement, and the capability was not widened** — \
                 which is the finding, because this row was written expecting the opposite. \
                 The kernel keeps the key, so `FileViewerState::activate` keeps doing the \
                 read; `Capability::Files` still publishes basenames and nothing else, no \
                 binding lists a directory, and the published row still carries no path. This \
                 row is now what stops \"the grant was unnecessary\" from quietly becoming \
                 \"the grant happened\"",
        gap: Gap::Structural,
        blocked: false,
        probe: |root| {
            // The keyboard survives...
            let the_kernel_keeps_the_key =
                method_body(root, "src/session/keybindings.rs", "pub fn pane_keyboards(")
                    .contains("KeyContext::FileViewer")
                    && method_body(root, "src/app/key_handlers.rs", "fn file_viewer_expand")
                        .contains("activate");
            // ...and the grant it was expected to need is *still* absent. Three
            // independent halves, because closing any one alone would give a plugin the
            // power: a capability to hold, a binding to call, and a path to name.
            let no_capability = !capability_names(root)
                .iter()
                .any(|c| c == "Fs" || c == "FileSystem");
            let no_binding = !module_bindings(root).iter().any(|b| {
                let lower = b.to_lowercase();
                [
                    "readdir",
                    "read_dir",
                    "listdir",
                    "readfile",
                    "read_file",
                    "glob",
                ]
                .iter()
                .any(|n| lower.contains(n))
            });
            let no_path = !field_names(&block(
                root,
                "src/session/pane_context.rs",
                "pub struct FileNodeSnapshot",
            ))
            .iter()
            .any(|f| f.contains("path"));
            !(the_kernel_keeps_the_key && no_capability && no_binding && no_path)
        },
    },
    Blocker {
        id: "no-process-launch",
        needs: "opening the selected file (`l`/`Enter` on a file), which launches the \
                configured editor — detached, or in a tmux popup",
        stands: "**closed as a handover requirement, and no process reach was granted.** The \
                 kernel keeps the key, so `App::file_viewer_expand` keeps calling \
                 `open_file_in_editor`; the write seam still names five operations over tasks \
                 and automations, nothing reaches a process, and `Capability::Spawn` still \
                 only adds environment to spawns thurbox already makes",
        gap: Gap::Structural,
        blocked: false,
        probe: |root| {
            let the_kernel_keeps_the_key =
                method_body(root, "src/app/key_handlers.rs", "fn file_viewer_expand")
                    .contains("open_file_in_editor");
            // Signatures only, not the trait body: its prose says plainly that a plugin
            // thread never "spawns a process, or opens a session", and a probe fooled by
            // the sentence stating the rule would report the rule broken.
            let seam_reaches_no_process = !writer_methods(root).iter().any(|m| {
                ["editor", "launch", "spawn", "exec", "open"]
                    .iter()
                    .any(|n| m.contains(n))
            });
            let bound = |needle: &str| {
                module_bindings(root)
                    .iter()
                    .any(|b| b.to_lowercase().contains(needle))
            };
            let no_binding = !bound("editor") && !bound("exec") && !bound("launch");
            !(the_kernel_keeps_the_key && seam_reaches_no_process && no_binding)
        },
    },
    Blocker {
        id: "sub-mode-keys-are-not-rebindable",
        needs: "nothing, since ADR-51 — recorded as a **property** of the pane rather than a \
                blocker, because it is equally true before and after a handover",
        stands: "while a search is active the pane's scoped key context is abandoned for the \
                 global one so that every character types into the query, and the sub-mode's \
                 keys are matched literally rather than resolved through an action. That is \
                 kernel state either way: a handover moves who *draws* the pane, and these \
                 keys are not in the keybinding editor before it or after it. Kept as a row \
                 because it is the honest answer to \"are this pane's keys rebindable\" — the \
                 seven scoped ones are, the sub-mode's are not, and that predates v2",
        gap: Gap::Structural,
        blocked: false,
        probe: |root| {
            let context_falls_back =
                method_body(root, "src/app/key_handlers.rs", "fn focus_key_context")
                    .contains("InputFocus::FileViewer if !self.file_viewer.search_active");
            let keys_are_literal = method_body(
                root,
                "src/app/key_handlers.rs",
                "fn handle_file_viewer_search_key",
            )
            .contains("KeyCode::Char(c) if !ctrl => self.file_viewer.search_push(c)");
            // Recorded as a property: the probe asserts the *fact* still holds, and the
            // row is not blocked, so it cannot be read as something a handover must fix.
            !(context_falls_back && keys_are_literal)
        },
    },
    Blocker {
        id: "no-query-write",
        needs: "nothing, since ADR-51 — the query the search collects is the **kernel's**, \
                and so is the bar that shows it (see `no-frame-node`)",
        stands: "a plugin declaring `input` receives keystrokes and nothing carries a query it \
                 collected into the pane's search: no binding names one, and the published \
                 section carries the search's *verdict* per row rather than its text. Both \
                 remain true, and neither is a handover requirement — the kernel keeps the \
                 `/` key, so it keeps the query",
        gap: Gap::Structural,
        blocked: false,
        probe: |root| {
            let no_binding = !module_bindings(root).iter().any(|b| {
                let lower = b.to_lowercase();
                lower.contains("search") || lower.contains("query")
            });
            let no_query_published = !field_names(&block(
                root,
                "src/session/pane_context.rs",
                "pub struct FilesSnapshot",
            ))
            .iter()
            .any(|f| f.contains("query"));
            !(no_binding && no_query_published)
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
/// The distinction ADR-35 forced on the global-search gate: `setTaskStatus` is
/// write-shaped and is *not* the write these keys need. A **view verb** is a view
/// write whatever it is applied to — there is no record you `focus` or `expand`.
/// A **generic mutator** counts only when it names a view noun, so `setTaskStatus`
/// passes and a future `setActiveSession` does not.
fn a_view_write_binding_exists(root: &Path) -> bool {
    // `expand`/`collapse` are this pane's own view verbs: an expansion state is
    // what the user has open, not a record anyone stores.
    const VIEW_VERBS: [&str; 8] = [
        "focus", "jump", "goto", "reveal", "scroll", "select", "expand", "collapse",
    ];
    const MUTATORS: [&str; 4] = ["set", "move", "open", "write"];
    const VIEW_NOUNS: [&str; 9] = [
        "Focus",
        "Cursor",
        "Selection",
        "Selected",
        "Row",
        "Pane",
        "Panel",
        "Active",
        "Expanded",
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

/// The body of the top-level `struct`/`enum`/`trait` whose declaration starts with
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

/// The field names declared in a struct body, doc comments and attributes
/// ignored.
///
/// Reading the raw text would not do, and both attempts to take that shortcut
/// failed the same way: these types *document* what they refuse ("never a path";
/// the run "belongs to the row the user's cursor is on"), so a probe over the
/// prose reads the sentence stating a rule as the rule being broken. A field is a
/// `pub` line at exactly one level of indentation, which is what rustfmt produces
/// for every struct in the tree.
fn field_names(body: &str) -> Vec<String> {
    body.lines()
        .filter_map(|line| {
            let unindented = line.strip_prefix("    ")?.strip_prefix("pub ")?;
            let name: String = unindented
                .chars()
                .take_while(|c| c.is_alphanumeric() || *c == '_')
                .collect();
            (!name.is_empty()).then_some(name)
        })
        .collect()
}

/// The capabilities a manifest may declare.
fn capability_names(root: &Path) -> Vec<String> {
    let names = variant_names(&block(
        root,
        "src/session/plugin_manifest.rs",
        "pub enum Capability",
    ));
    assert!(
        names.iter().any(|n| n == "Files"),
        "no capabilities found — the probe's anchor changed shape, so every verdict derived from \
         it is meaningless"
    );
    names
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
        "the file viewer's handover verdict disagrees with the source tree.\n\
         A row that closed changes what the handover costs: re-verdict it here and revisit \
         docs/PHASE4-PANE-READINESS.md §29 in the same change.{failures}"
    );
}

/// The verdict follows from the rows, and **this** is the headline: the handover is
/// still refused, and nothing outstanding is structural.
///
/// That second half is the finding (ADR-54). Every row this gate was written around was
/// structural — a power a plugin pane is not given — and the change that would have
/// closed them was expected to be a widened `files` capability. It was not: ADR-51 left
/// the keys with the kernel, so the reads and the launch stay where they were and no
/// grant was made. What remains is three unmade decisions, and a reader who sees
/// "blocked" must be able to tell that from "refused in principle".
#[test]
fn the_verdict_is_derived_from_the_blockers() {
    assert!(
        !keys_are_portable(BLOCKERS),
        "every requirement is recorded met — the pane may now be handoverable, so hand it over \
         deliberately (and retire this gate) rather than leaving it passing vacuously"
    );

    // The headline: nothing left is structural. If a structural row comes back, the
    // model started withholding something again and the ordering changed with it.
    let structural = structural_blockers(BLOCKERS);
    assert!(
        structural.is_empty(),
        "a structural row is outstanding again: {structural:?}. Since ADR-51 this pane needs no \
         power a plugin pane is denied — if that stopped being true, say what changed rather \
         than recording it as one more row"
    );

    // And the rows that closed did so **without** a grant, which is the fact the
    // capability rows now exist to keep. Stated here too, so the gate's headline cannot
    // be satisfied by widening the host.
    let root = repo_root();
    assert!(
        !capability_names(&root)
            .iter()
            .any(|c| c == "Fs" || c == "FileSystem"),
        "a filesystem capability appeared: this pane's handover was recorded as needing none, \
         so either that record is wrong or the grant is"
    );

    // The other direction: a tree where every row landed permits the handover, so the
    // gate gates rather than forbids.
    let all_closed: Vec<Blocker> = BLOCKERS
        .iter()
        .map(|b| Blocker {
            blocked: false,
            ..*b
        })
        .collect();
    assert!(keys_are_portable(&all_closed));

    // And it is not vacuous in the other direction either: every outstanding row is a
    // decision the host could take (`Vocabulary`), so closing them all is a change
    // someone can plan rather than a wall.
    assert_eq!(
        outstanding(BLOCKERS, Gap::Vocabulary).len(),
        BLOCKERS.iter().filter(|b| b.blocked).count(),
        "every outstanding row should be a decision, not a refusal"
    );
}

/// The rows outstanding of one kind — which is how the ordering of the remaining work is
/// read off the table rather than argued in prose.
fn outstanding(blockers: &[Blocker], gap: Gap) -> Vec<&'static str> {
    blockers
        .iter()
        .filter(|b| b.blocked && b.gap == gap)
        .map(|b| b.id)
        .collect()
}

/// The distinction ADR-35 forced on the global-search gate, asserted here too: a
/// plugin **can** change a task, and that is not the write this pane's keys need.
///
/// Without this the first row could be "simplified" to "is there any write-shaped
/// binding", which `setTaskStatus` already satisfies — reporting the pane's keys
/// portable while nothing can move a cursor.
#[test]
fn a_record_write_is_not_the_view_write_these_keys_need() {
    let root = repo_root();
    let bindings = module_bindings(&root);
    assert!(
        bindings.iter().any(|b| b == "setTaskStatus"),
        "`setTaskStatus` should be a granted binding — if it went, this test no longer proves \
         anything and should be revisited with it"
    );
    assert!(
        !a_view_write_binding_exists(&root),
        "a binding that writes view state appeared: the pane's cursor or its expansion state may \
         now be writable, so re-verdict `no-view-write` rather than relaxing this test"
    );
}

/// The finding that makes this pane's verdict stronger than the tasks pane's,
/// asserted directly rather than only through its rows: **not one** of the pane's
/// keys is a record write, so there is no partial key surface to argue about.
///
/// Derived from the dispatch itself, so a key that later became a record write
/// fails here with the argument attached.
#[test]
fn not_one_of_the_panes_keys_is_a_record_write() {
    let root = repo_root();
    let dispatch = method_body(
        &root,
        "src/app/key_handlers.rs",
        "fn dispatch_file_viewer_action",
    );
    // Every arm writes the pane's own view state, which is what `self.file_viewer`
    // is — or, for the expand arm, that plus an editor launch.
    let arms: Vec<&str> = dispatch
        .lines()
        .map(str::trim)
        .filter(|line| line.starts_with("Action::FileViewer"))
        .collect();
    assert_eq!(
        arms.len(),
        7,
        "the pane is expected to answer seven actions; if that changed, this verdict is about a \
         different pane: {arms:?}"
    );
    for arm in &arms {
        assert!(
            arm.contains("self.file_viewer") || arm.contains("self.file_viewer_expand()"),
            "every file-viewer key is expected to write the pane's view state: {arm}"
        );
    }
    // And the one arm that does more than that reaches an editor process, which is
    // the `no-process-launch` row's subject.
    assert!(
        method_body(&root, "src/app/key_handlers.rs", "fn file_viewer_expand")
            .contains("open_file_in_editor"),
        "expanding a file is expected to launch the editor; if it stopped, `no-process-launch` \
         is about something else now"
    );
}

/// The three facts behind `the-module-is-the-model-and-the-window`, named one at a
/// time.
///
/// The row's probe answers yes-or-no; this says *which* of the three moved, which is
/// what a reader of the failure needs. It was a standalone observation about cost
/// until ADR-54 made it one of the three things deciding the verdict.
#[test]
fn the_module_a_handover_deletes_is_the_panes_model() {
    let root = repo_root();
    assert!(
        source(&root, "src/ui/file_viewer.rs").contains("pub struct FileViewerState"),
        "the pane's state machine is expected to live in the module the teardown deletes"
    );
    assert!(
        source(&root, "src/app/mod.rs").contains("crate::ui::file_viewer::FileViewerState"),
        "`App` is expected to own that state, which is what the published section reads"
    );
    assert!(
        source(&root, "src/ui/plugin_pane.rs").contains("file_viewer::visible_window"),
        "every plugin list is expected to be windowed by a helper in that same module, so \
         deleting it would stop *plugin* panes scrolling"
    );
}

/// The consequence of the missing view write that this change made visible: a
/// plugin pane's scroll track is an indicator, not a control.
///
/// The thumb reports a cursor the plugin does not own, so a drag would have to
/// write view state — the first row's wall. Asserted on the absence of a recorded
/// drag target rather than in prose, so a later change that records one meets the
/// argument here.
#[test]
fn a_plugin_panes_scroll_track_is_not_draggable() {
    let root = repo_root();
    assert!(
        !variant_names(&block(
            &root,
            "src/app/mod.rs",
            "pub(crate) enum ScrollTarget"
        ))
        .iter()
        .any(|v| v.contains("Plugin")),
        "no scroll target names a plugin pane: a drag would have to move a cursor the plugin does \
         not own"
    );
    // Both halves of the plugin-pane paint: the loop that places each pane, and
    // the painter both placements share since ADR-46.
    for f in ["fn render_plugin_panes", "fn paint_plugin_pane"] {
        assert!(
            !method_body(&root, "src/app/view.rs", f).contains("record_scrollbar"),
            "a plugin pane is expected to record no scrollbar drag target, and `{f}` does"
        );
    }
}

/// The verdict and the shipped plugin must not disagree: a pane that declared keys
/// it could not act with would be the failure this gate records.
#[test]
fn the_bundled_plugin_takes_no_keys() {
    let root = repo_root();
    let manifest = source(&root, "src/plugin/bundled/file-viewer/plugin.toml");
    assert!(
        !manifest.contains("input"),
        "the bundled file-viewer plugin must not declare the input capability while its keys have \
         nowhere to land: {manifest}"
    );
    assert!(
        !manifest.contains("[[keybindings]]"),
        "the bundled file-viewer plugin must declare no keybinding: {manifest}"
    );
}

/// The pane is reproduced, not replaced — so the thing a user presses `j` in is
/// still the native renderer, and the teardown inventory's answer for it is
/// unchanged.
#[test]
fn the_native_pane_is_still_what_thurbox_draws() {
    let root = repo_root();
    assert!(
        root.join("src/ui/file_viewer.rs").is_file(),
        "the native file viewer's renderer must still exist"
    );
    assert!(
        source(&root, "src/app/view.rs").contains("file_viewer"),
        "the interface must still draw the native file viewer: a plugin that cannot take the \
         pane's keys cannot replace it"
    );
}
