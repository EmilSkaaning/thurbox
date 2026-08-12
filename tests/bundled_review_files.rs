//! The bundled code-review plugin renders the review's **changed-files list**.
//!
//! The code review is drawn as two panes, not one: the diff stream in the central
//! pane (`tests/bundled_code_review.rs`) and this list in the file-viewer column,
//! with its own focus, its own keys and a cursor that follows the diff's. This is
//! the port of the second one.
//!
//! The equality chain has the shape the diff's does, one link shorter. Since
//! ADR-65 the native list is itself painted from a view tree —
//! `ui::code_review::files_list_tree` — so tree equality *is* frame equality for
//! this pane, and there is no windowing arithmetic on either side to reconcile:
//! both name the cursor's row and the kernel resolves the slice.
//!
//! 1. here: the plugin's tree equals `ui::code_review::files_list_tree`;
//! 2. in `ui::code_review`'s own tests: that builder paints what the pane's
//!    pre-port spans painted, cell for cell.
//!
//! Without the second link the first would be two functions written in the same
//! change agreeing about a format neither is obliged to match.
//!
//! And a **recorded** edge as well, for `tests/bundled_code_review.rs`'s reason: a
//! handover deletes `src/ui/code_review.rs`, taking both links with it, and what is
//! left is a test that the plugin renders without erroring — satisfied equally by a
//! pane drawing one wrong row and by one drawing twelve. The recording is taken from
//! the **kernel's** builder while it is here, which is the only moment its
//! provenance can be established (ADR-48).
//!
//! Both sides are handed the **same published rows**, so a failure is a statement
//! about the plugin rather than about two fixtures drifting apart.

#![cfg(feature = "plugins")]

use std::path::PathBuf;

use thurbox::plugin::{discovery, ExecutionBounds, PluginHost};
use thurbox::session::pane_context::{PaneContext, ReviewFileRowSnapshot, ReviewSnapshot};
use thurbox::session::view_tree::ViewNode;
use thurbox::ui::code_review::files_list_tree;

/// The process-wide snapshot slot is global, so every case here runs one at a
/// time — otherwise one case's publication would answer another's reader.
static SERIALIZE: std::sync::Mutex<()> = std::sync::Mutex::new(());

mod view_tree_record;

/// One comparison case: the published tree, and which of its rows the diff's
/// cursor is in.
struct Case {
    name: &'static str,
    rows: Vec<ReviewFileRowSnapshot>,
    cursor: Option<usize>,
}

fn folder(depth: usize, name: &str) -> ReviewFileRowSnapshot {
    ReviewFileRowSnapshot::Folder {
        depth,
        name: name.to_string(),
    }
}

fn file(
    depth: usize,
    path: &str,
    status: &'static str,
    added: usize,
    removed: usize,
    reviewed: bool,
) -> ReviewFileRowSnapshot {
    ReviewFileRowSnapshot::File {
        depth,
        path: path.to_string(),
        status,
        added,
        removed,
        reviewed,
    }
}

impl Case {
    fn native_tree(&self) -> ViewNode {
        files_list_tree(&self.rows, self.cursor)
    }

    /// The rows as the snapshot a plugin reads. Mirrors
    /// `App::build_review_snapshot`: the kernel publishes the tree and the cursor's
    /// row, and no glyph, no basename, no window.
    fn context(&self) -> PaneContext {
        PaneContext {
            review: ReviewSnapshot {
                file_rows: self.rows.clone(),
                file_cursor: self.cursor,
                rows: Vec::new(),
                cursor: None,
                number_width: 0,
            },
            ..PaneContext::default()
        }
    }

    fn snapshot_name(&self) -> String {
        self.name.replace(", ", " ").replace(' ', "-")
    }
}

/// The variants. Each isolates one decision a row makes, so a failure names which
/// part of the list the plugin gets wrong rather than only that it does.
fn cases() -> Vec<Case> {
    vec![
        // No review open, and what a plugin sees before one has ever been opened.
        Case {
            name: "no changes",
            rows: Vec::new(),
            cursor: None,
        },
        // The shape the tree exists for: files grouped under nested directories,
        // with a top-level file after them (the sort is by path segment).
        Case {
            name: "a nested tree",
            rows: vec![
                folder(0, "src"),
                file(1, "src/b.rs", "modified", 3, 1, false),
                folder(1, "ui"),
                file(2, "src/ui/a.rs", "added", 12, 0, false),
                file(2, "src/ui/z.rs", "deleted", 0, 40, false),
                file(0, "top.rs", "renamed", 1, 1, false),
            ],
            cursor: Some(1),
        },
        // The cursor on a file the user has marked reviewed: the mark is drawn and
        // the selection appearance still owns the whole row.
        Case {
            name: "the cursor on a reviewed file",
            rows: vec![
                folder(0, "docs"),
                file(1, "docs/GUIDE.md", "modified", 2, 2, true),
                file(1, "docs/OTHER.md", "modified", 1, 0, true),
            ],
            cursor: Some(1),
        },
        // No cursor at all, which is what the diff's summary section publishes:
        // nothing is highlighted and the window opens at the top.
        Case {
            name: "no cursor",
            rows: vec![file(0, "a.rs", "modified", 1, 1, false)],
            cursor: None,
        },
        // A status name this build does not know: the row still draws, with the
        // residual glyph both panes derive.
        Case {
            name: "an unknown status",
            rows: vec![file(0, "a.rs", "copied", 1, 0, false)],
            cursor: Some(0),
        },
        // A directory deep enough that its name is what a narrow column would cut,
        // which is the one row of this list that yields its width.
        Case {
            name: "a deep directory",
            rows: vec![
                folder(0, "a-very-long-directory-name-indeed"),
                file(
                    1,
                    "a-very-long-directory-name-indeed/x.rs",
                    "modified",
                    9,
                    9,
                    false,
                ),
            ],
            cursor: Some(1),
        },
        // Multi-byte names: the basename is split on `/` and nothing here counts
        // bytes, so a plugin scanning bytes would cut a character in half.
        Case {
            name: "multi-byte names",
            rows: vec![
                folder(0, "文档"),
                file(1, "文档/naïve.md", "modified", 1, 1, false),
            ],
            cursor: Some(1),
        },
    ]
}

/// Start the bundled plugin from the source that ships.
fn host() -> PluginHost {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/plugin/bundled/code-review");
    let outcome = discovery::discover_in(&[dir], None);
    assert!(
        outcome.problems.is_empty(),
        "the bundled plugin must load cleanly: {:?}",
        outcome.problems
    );
    let mut host = PluginHost::from_discovery(outcome, ExecutionBounds::default());
    assert_eq!(host.start_all(), 1, "the plugin must reach Running");
    host
}

fn render(host: &PluginHost) -> ViewNode {
    host.render_pane("code-review", "files")
        .expect("the pane renders")
}

/// The claim: the plugin's changed-files list is the kernel's, row for row.
///
/// The recorded edge is asserted **first**, so a run in which both edges break
/// reports the recording — the fact about the pane — rather than only that two
/// implementations disagree.
#[test]
fn the_plugin_builds_the_native_changed_files_tree() {
    let _guard = SERIALIZE.lock().unwrap_or_else(|e| e.into_inner());
    let host = host();

    for case in cases() {
        thurbox::session::pane_context::publish(case.context());
        let plugin = render(&host);
        let native = case.native_tree();
        insta::assert_snapshot!(case.snapshot_name(), view_tree_record::tree(&native));
        view_tree_record::assert_matches(case.name, &plugin, &native);
        assert_eq!(
            plugin, native,
            "the plugin's tree diverges from the native list for `{}`",
            case.name
        );
    }
}

/// Both row kinds occur, so the equality cannot be passing on file rows alone
/// while a directory header quietly diverges.
#[test]
fn both_row_kinds_are_compared() {
    let mut seen: Vec<&str> = cases()
        .iter()
        .flat_map(|c| c.rows.iter())
        .map(ReviewFileRowSnapshot::wire_name)
        .collect();
    seen.sort_unstable();
    seen.dedup();
    for kind in ["folder", "file"] {
        assert!(seen.contains(&kind), "no case publishes a {kind} row");
    }
}

/// Every status the review defines is drawn by some case, since the glyph and its
/// colour are the pane's derivation from a name and a missed one is a wrong letter.
#[test]
fn every_file_status_is_compared() {
    let statuses: Vec<&str> = cases()
        .iter()
        .flat_map(|c| c.rows.iter())
        .filter_map(|r| match r {
            ReviewFileRowSnapshot::File { status, .. } => Some(*status),
            ReviewFileRowSnapshot::Folder { .. } => None,
        })
        .collect();
    for status in ["modified", "added", "deleted", "renamed"] {
        assert!(
            statuses.contains(&status),
            "no case publishes a {status} file"
        );
    }
}

/// The pane is a **reproduction**, and its handover is refused on the seat it
/// belongs in — so the manifest must not claim that seat or the keyboard that goes
/// with it while the native list is what thurbox draws.
#[test]
fn the_reproduction_claims_neither_the_seat_nor_the_keyboard() {
    let manifest = std::fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("src/plugin/bundled/code-review/plugin.toml"),
    )
    .expect("the bundled manifest is readable");
    assert!(
        !manifest.contains("key_context"),
        "the reproduction must not declare a keyboard while the native pane draws"
    );
    assert!(
        !manifest.contains("slot = \"file-viewer\""),
        "the reproduction must not claim the seat its handover is refused on"
    );
    assert!(
        manifest.contains("capabilities = [\"render\", \"review\"]"),
        "the second pane needs no capability of its own: the tree crosses in the \
         `review` section the diff already reads"
    );
}
