//! The file viewer's **input** verdict, enforced as a test.
//!
//! `docs/PHASE4-PANE-READINESS.md` §16 records that the file viewer's *rendering*
//! is reproduced by a bundled plugin — including, since this change, its scroll
//! track — while its **keys** cannot be. This file is that half of the verdict in
//! executable form: [`BLOCKERS`] records one host power the pane's keys need and
//! do not have, per row, and re-derives each from the source.
//!
//! Why a test rather than only the document, in `tests/global_search_pane_gap.rs`'s
//! words: a verdict written in markdown is a fact about a build that expires
//! without telling anyone.
//!
//! ## Which route this gate measures, since ADR-51 there are two
//!
//! Every row below is about a pane whose **keys are the plugin's**: `input`, a
//! binding per chord (ADR-34), and a capability per effect — which for this pane
//! would mean a filesystem read and a process launch, the two grants ADR-39 refused.
//! Each row is still a true statement about that route; nothing here has been
//! granted.
//!
//! ADR-51 added a second: a pane may declare that it **is** thurbox's file viewer,
//! and the kernel then resolves `KeyContext::FileViewer` and performs those actions
//! itself while the pane holds focus. On that route the two dangerous grants are
//! not needed — the kernel reads the directory and launches the editor, as it always
//! did — so a blocked row here says "this pane's keys cannot be ported **to a
//! plugin**", never "this pane cannot be handed over". What the second route leaves
//! untouched is the last two rows and the structural fact below: the search bar is
//! still drawn outside any pane's tree, and the module a handover deletes is still
//! this pane's model.
//!
//! **What is different about this pane**, and why it needed its own gate rather
//! than a line in the tasks pane's (`tests/tasks_pane_input_gap.rs`): the tasks
//! pane had two keys that needed no new host power and failed for a second reason.
//! This pane has none. All seven of its `KeyContext::FileViewer` actions write
//! view state, and two of them need powers the vocabulary does not define at all —
//! expanding a directory **reads the filesystem**, and expanding a file **launches
//! a process**. On top of that its `/` sub-mode cannot meet the parity bar even in
//! principle: while it is active the pane's scoped key context is abandoned so
//! every character types into the query, which is the opposite of "rebindable, and
//! in the F1 editor".
//!
//! And one structural fact this pane is the first to have: **the module a handover
//! would delete is the pane's model, not only its renderer.** `FileViewerState`
//! lives in `src/ui/file_viewer.rs`, `App` owns one, and the published section is
//! derived from it — as is `visible_window`, the rule every *plugin* list is
//! scrolled by. [`the_module_a_handover_deletes_is_the_panes_model`] pins that.
//!
//! Three things this gate is deliberately not:
//!
//! - it is **not** the teardown gate, which answers whether
//!   `src/ui/file_viewer.rs` may be deleted — already no, for ADR-37's reason, and
//!   no either way;
//! - it is **not** a claim that the pane's rendering is inexpressible. The plugin
//!   builds the native pane's view tree and paints its frame, track included
//!   (`tests/bundled_file_viewer.rs`);
//! - it is **not** a copy of the tasks pane's verdict. The rows differ, and the
//!   difference is the finding.
//!
//! Its probes read the source the way a human auditor would, so this gate runs,
//! and means the same thing, with or without the `plugins` Cargo feature. The
//! helpers below duplicate the other two gates', because an integration test
//! cannot import another one — the alternative is a shared crate for three
//! readers, which is more machinery than the duplication costs.

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
        id: "no-view-write",
        needs: "every one of the pane's seven keys — moving the cursor (`j`/`k`), collapsing \
                (`h`), expanding (`l`/`Enter`), starting a search (`/`) and stepping matches \
                (`n`/`N`) all write the cursor, the expansion set, or both",
        stands: "no binding writes *view* state — a plugin may change records it was granted \
                 (ADR-35), and nothing it holds moves a cursor or expands a row",
        gap: Gap::Structural,
        blocked: true,
        probe: |root| !a_view_write_binding_exists(root),
    },
    Blocker {
        id: "no-filesystem-read",
        needs: "filling a directory the first time it is expanded (`l`/`Enter` on a folder), \
                which the kernel does by reading it",
        stands: "the vocabulary defines no filesystem capability — `Capability::Fs` is reserved \
                 by the teardown inventory for v1's \"place a file in an agent's own config \
                 dir\" power — no binding lists a directory or reads a file, and the published \
                 rows carry no path to read",
        gap: Gap::Structural,
        blocked: true,
        probe: |root| {
            // Three independent halves, because closing any one alone would not
            // give a plugin the power: a capability to hold, a binding to call,
            // and a path to name.
            let no_capability = !capability_names(root)
                .iter()
                .any(|c| c == "Fs" || c == "FileSystem");
            // Needles long enough to be the name of a filesystem reader and no
            // shorter: `stat` would have matched `stateRead`, which is the
            // plugin's own key/value store and the opposite of a filesystem
            // grant.
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
            // Scoped to the published row's *fields*: the type documents that a
            // basename is "never a path", so a probe over its prose would read
            // the sentence stating the rule as the rule being broken.
            let no_path = !field_names(&block(
                root,
                "src/session/pane_context.rs",
                "pub struct FileNodeSnapshot",
            ))
            .iter()
            .any(|f| f.contains("path"));
            no_capability && no_binding && no_path
        },
    },
    Blocker {
        id: "no-process-launch",
        needs: "opening the selected file (`l`/`Enter` on a file), which launches the \
                configured editor — detached, or in a tmux popup",
        stands: "the write seam names five operations over tasks and automations, and even \
                 running an automation is a *request* the kernel fulfils. `Capability::Spawn` \
                 adds environment to spawns thurbox already makes; it is not the power to start \
                 one",
        gap: Gap::Structural,
        blocked: true,
        probe: |root| {
            // Signatures only, not the trait body: its prose says plainly that a
            // plugin thread never "spawns a process, or opens a session", and a
            // probe fooled by the sentence stating the rule would report the rule
            // broken.
            let seam_reaches_a_process = writer_methods(root).iter().any(|m| {
                ["editor", "launch", "spawn", "exec", "open"]
                    .iter()
                    .any(|n| m.contains(n))
            });
            let bound = |needle: &str| {
                module_bindings(root)
                    .iter()
                    .any(|b| b.to_lowercase().contains(needle))
            };
            !seam_reaches_a_process && !bound("editor") && !bound("exec") && !bound("launch")
        },
    },
    Blocker {
        id: "sub-mode-keys-are-not-rebindable",
        needs: "the `/` sub-mode, whose keys a ported pane would have to expose in the \
                keybinding editor like any other",
        stands: "while a search is active the pane's scoped key context is abandoned for the \
                 global one so that every character types into the query, and the sub-mode's \
                 keys are matched literally rather than resolved through an action — so there \
                 is nothing to rebind",
        gap: Gap::Structural,
        blocked: true,
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
            context_falls_back && keys_are_literal
        },
    },
    Blocker {
        id: "no-query-write",
        needs: "a query a plugin collected doing anything — the search's effect is revealing \
                matches by expanding directories, moving the cursor between them, and marking \
                which rows matched",
        stands: "a plugin declaring `input` receives the keystrokes, and nothing carries a query \
                 it collected into the pane's search: no binding names one, and the published \
                 section carries the search's *verdict* per row and not its text",
        gap: Gap::Structural,
        blocked: true,
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
            no_binding && no_query_published
        },
    },
    Blocker {
        id: "no-frame-node",
        needs: "the search bar's own bordered block, its caret, and its place pinned to the \
                bottom of the pane",
        stands: "a pane's frame is the host's, drawn around whatever the plugin returned; no \
                 node describes a border, a cursor cell, or a region anchored to the bottom of \
                 an area",
        gap: Gap::Vocabulary,
        blocked: true,
        probe: |root| {
            let kinds = view_node_kinds(root);
            let no_frame = !kinds
                .iter()
                .any(|k| k == "Block" || k == "Frame" || k == "Border" || k == "Panel");
            // A caret is an appearance, so it would be a style field rather than a
            // node kind. Fields, not prose: `selected`'s documentation is *about*
            // the row the user's cursor is on.
            let no_caret = !field_names(&block(
                root,
                "src/session/view_tree.rs",
                "pub struct TextStyle",
            ))
            .iter()
            .any(|f| f.contains("cursor") || f.contains("caret"));
            no_frame && no_caret
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

/// The node kinds the view tree defines.
fn view_node_kinds(root: &Path) -> Vec<String> {
    variant_names(&block(
        root,
        "src/session/view_tree.rs",
        "pub enum ViewNode",
    ))
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
        "the file viewer's input verdict disagrees with the source tree.\n\
         A row that became available may unblock part of the pane's key surface: re-verdict it \
         here and revisit docs/PHASE4-PANE-READINESS.md §16 in the same change.{failures}"
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

    // The load-bearing half: the reason is structural. Closing the vocabulary row
    // would draw the search *bar* and change nothing about a single key.
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

/// The structural fact this pane is the first to have: the module a handover would
/// delete is the pane's **model**, and it also owns the rule every plugin list is
/// scrolled by.
///
/// Not a key blocker — it is what makes "delete the native renderer" mean
/// something different here — so it is asserted rather than tabled. If any of the
/// three facts stops holding, the handover's cost has changed and §16 should say
/// so.
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
