//! The bundled code-review plugin renders the native review's whole document.
//!
//! The fourth Phase 4 port, and the one done in two steps. The native view
//! (`src/ui/code_review.rs`) is the largest pane in thurbox, and the first step
//! reproduced only its diff *lines*. What is reproduced now is its **document**:
//! every row kind the pane lists — file headers with their rule, fold chevron,
//! status letter, insertion and deletion counts and reviewed mark; hunk headers
//! with their `@@` ranges and mark; the diff lines with their gutter,
//! syntax-coloured bodies and row tints; comments with their classification badges;
//! the review summary's heading and its comments; informational rows — each in the
//! selection appearance when the cursor is on it.
//!
//! What is *not* reproduced is the pane's behaviour, and the split is the finding
//! rather than a scoping note: the rows were missing because the published section
//! carried lines instead of rows, while every remaining entry needs a host power
//! this surface does not have.
//! [`the_out_of_scope_surface_is_absent_rather_than_approximated`] pins what is
//! still absent; `tests/code_review_pane_handover_gap.rs` gates why.
//!
//! The equality claim is built differently from the three earlier ports, and the
//! difference matters. Those refactored the native pane to *draw* the view tree,
//! so tree equality was frame equality by construction. This pane's renderer
//! cannot be a geometry-free tree — it windows a body by character count against
//! a resolved width — so the native paint path is untouched and the chain has two
//! links instead of one:
//!
//! 1. here: the plugin's tree equals `ui::code_review::review_stream_tree`;
//! 2. in `ui::code_review`'s own tests: that builder and the untouched
//!    `row_visual_lines` **paint the same row**, for every row kind.
//!
//! Without the second link the first would be two functions written in the same
//! change agreeing about a format neither is obliged to match.
//!
//! It lives in `tests/` for the same reason as its siblings: this is the one place
//! that must see both `ui::code_review` and `plugin::PluginHost`, and an
//! integration test is not part of the library's module graph, so
//! `tests/architecture_rules.rs` stays untouched.

#![cfg(feature = "plugins")]

use std::path::PathBuf;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

use thurbox::plugin::{discovery, ExecutionBounds, PluginHost};
use thurbox::session::motion::FrameTable;
use thurbox::session::pane_context::{
    PaneContext, ReviewCommentSnapshot, ReviewFileSnapshot, ReviewHunkSnapshot, ReviewLineSnapshot,
    ReviewRowSnapshot, ReviewSnapshot,
};
use thurbox::session::view_tree::{ViewNode, MAX_NODES};
use thurbox::ui::code_review::review_stream_tree;

/// The process-wide snapshot slot is global, so every case here runs one at a
/// time — otherwise one case's publication would answer another's reader.
static SERIALIZE: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// One comparison case. Both sides are derived from the **same published rows**, so
/// an equality failure is a statement about the plugin rather than about two
/// hand-written fixtures drifting apart. The rows are exactly what
/// `CodeReviewState::snapshot_rows` produces, so neither side re-maps anything.
struct Case {
    name: &'static str,
    rows: Vec<ReviewRowSnapshot>,
    cursor: Option<usize>,
    number_width: usize,
}

fn line(
    path: &str,
    kind: &'static str,
    old_no: Option<u32>,
    new_no: Option<u32>,
    text: &str,
) -> ReviewRowSnapshot {
    ReviewRowSnapshot::Line(ReviewLineSnapshot {
        path: path.to_string(),
        old_no,
        new_no,
        kind,
        text: text.to_string(),
    })
}

fn context(old: u32, new: u32, text: &str) -> ReviewRowSnapshot {
    line("src/a.rs", "context", Some(old), Some(new), text)
}

fn added(new: u32, text: &str) -> ReviewRowSnapshot {
    line("src/a.rs", "add", None, Some(new), text)
}

fn removed(old: u32, text: &str) -> ReviewRowSnapshot {
    line("src/a.rs", "del", Some(old), None, text)
}

/// Re-path a run of line rows, since a line's comment marker follows its file.
fn in_file(path: &str, rows: Vec<ReviewRowSnapshot>) -> Vec<ReviewRowSnapshot> {
    rows.into_iter()
        .map(|row| match row {
            ReviewRowSnapshot::Line(l) => ReviewRowSnapshot::Line(ReviewLineSnapshot {
                path: path.to_string(),
                ..l
            }),
            other => other,
        })
        .collect()
}

fn file_row(path: &str, status: &'static str, folded: bool, reviewed: bool) -> ReviewRowSnapshot {
    ReviewRowSnapshot::File(ReviewFileSnapshot {
        path: path.to_string(),
        status,
        added: 3,
        removed: 1,
        folded,
        reviewed,
    })
}

fn hunk_row(heading: &str, reviewed: bool) -> ReviewRowSnapshot {
    ReviewRowSnapshot::Hunk(ReviewHunkSnapshot {
        old_start: 11,
        old_span: 4,
        new_start: 11,
        new_span: 5,
        heading: heading.to_string(),
        reviewed,
    })
}

fn comment_row(classification: &'static str, text: &str, more: bool) -> ReviewRowSnapshot {
    ReviewRowSnapshot::Comment(ReviewCommentSnapshot {
        classification,
        text: text.to_string(),
        more,
    })
}

impl Case {
    fn native_tree(&self) -> ViewNode {
        review_stream_tree(&self.rows, self.cursor, self.number_width)
    }

    /// The rows as the snapshot a plugin reads.
    ///
    /// Mirrors `App::build_review_snapshot`: the kernel publishes the rows, the
    /// cursor and the gutter width — and no colouring, no glyph, no window and no
    /// padding.
    fn context(&self) -> PaneContext {
        PaneContext {
            review: ReviewSnapshot {
                rows: self.rows.clone(),
                cursor: self.cursor,
                number_width: self.number_width,
            },
            ..PaneContext::default()
        }
    }
}

/// The variants. Each isolates one decision a row makes, so a failure names which
/// part of the document the plugin gets wrong rather than only that it does.
fn cases() -> Vec<Case> {
    let case = |name, rows, cursor, number_width| Case {
        name,
        rows,
        cursor,
        number_width,
    };
    vec![
        // No review open, and what a plugin sees before one has ever been opened.
        case("no changes", Vec::new(), None, 0),
        // The three line kinds together, which is what a hunk is.
        case(
            "an insertion, a deletion and context",
            in_file(
                "src/build.rs",
                vec![
                    context(11, 11, "fn build() {"),
                    removed(12, "    let total = sum;"),
                    added(12, "    let total = sum + 1;"),
                    context(13, 13, "}"),
                ],
            ),
            Some(2),
            2,
        ),
        // Every colour the highlighter assigns. A wrong token here is a wrong run
        // in the tree, which is exactly what the equality catches.
        case(
            "every token kind",
            in_file(
                "src/tokens.rs",
                vec![
                    context(1, 1, "pub struct Row {"),
                    context(2, 2, "    label: Vec<String>,"),
                    added(3, "    count: 0x1F_u32,"),
                    added(4, "    name: \"a string\","),
                    context(5, 5, "// why, not what"),
                    context(6, 6, "}"),
                ],
            ),
            Some(0),
            2,
        ),
        // The comment marker follows the extension, and a `#` outside a hash
        // language is not a comment.
        case(
            "comment markers per language",
            vec![
                line(
                    "conf/app.toml",
                    "context",
                    Some(1),
                    Some(1),
                    "# a hash comment",
                ),
                line(
                    "db/schema.sql",
                    "context",
                    Some(1),
                    Some(1),
                    "-- a dash one",
                ),
                line(
                    "src/main.rs",
                    "context",
                    Some(1),
                    Some(1),
                    "#[derive(Debug)]",
                ),
                line("run.sh", "add", None, Some(2), "# shell"),
            ],
            None,
            3,
        ),
        // The cursor on each kind: the selection has to beat the tint, and on a
        // context row there is no tint to beat.
        case(
            "the cursor on an insertion",
            vec![context(1, 1, "a"), added(2, "b")],
            Some(1),
            2,
        ),
        case(
            "the cursor on a deletion",
            vec![removed(1, "a"), context(1, 2, "b")],
            Some(0),
            2,
        ),
        // A four-digit gutter: the number columns widen together, and an absent
        // number is an empty column rather than a zero.
        case(
            "a wide gutter",
            vec![
                line("src/a.rs", "context", Some(1201), Some(1198), "x"),
                added(1199, "y"),
                removed(1202, "z"),
            ],
            Some(0),
            4,
        ),
        // An empty body still draws its gutter and its tint.
        case(
            "empty bodies",
            vec![added(1, ""), removed(1, ""), context(2, 2, "")],
            None,
            2,
        ),
        // Multi-byte and double-width text: the lexer walks characters, and a
        // plugin scanning bytes would drift after the first one.
        case(
            "multi-byte bodies",
            in_file(
                "src/i18n.rs",
                vec![
                    added(1, "let s = \"日本語のテキスト\";"),
                    context(2, 2, "let répertoire = 1;"),
                    context(3, 3, "// commentaire accentué"),
                ],
            ),
            Some(1),
            2,
        ),
        // A tab in the body: both sides sanitize it identically, since both build
        // nodes. (The native *span* renderer does not — pinned in
        // `ui::code_review`'s own tests as an enumerated divergence.)
        case(
            "a tab in the body",
            in_file("src/main.go", vec![added(1, "\tfmt.Println(x)")]),
            None,
            2,
        ),
        // No cursor at all, which is not the same as a cursor on the first row:
        // the list must carry none, so it neither highlights nor scrolls.
        case(
            "a stream with no cursor",
            vec![context(1, 1, "a"), context(2, 2, "b")],
            None,
            2,
        ),
        // ── the rows the second step added ──────────────────────────────────
        //
        // Each file status, since the letter *and* its colour role differ per
        // status and a near-miss token would pass on the default palette.
        case(
            "each file status",
            vec![
                file_row("src/modified.rs", "modified", false, false),
                file_row("src/new.rs", "added", false, false),
                file_row("src/gone.rs", "deleted", false, false),
                file_row("src/moved.rs", "renamed", false, false),
            ],
            None,
            2,
        ),
        // A folded file is one row, and a reviewed one carries its mark: the
        // chevron follows `folded` rather than `reviewed`, which is why both
        // cross.
        case(
            "folded and reviewed files",
            vec![
                file_row("src/peeked.rs", "modified", false, true),
                file_row("src/done.rs", "modified", true, true),
                file_row("src/collapsed.rs", "modified", true, false),
            ],
            Some(1),
            2,
        ),
        // A hunk header with and without its mark, and one with no heading at all
        // (a new file's first hunk).
        case(
            "hunk headers",
            vec![
                hunk_row("fn build", false),
                hunk_row("fn build", true),
                hunk_row("", false),
            ],
            Some(2),
            2,
        ),
        // Each classification, since the badge's label and colour both follow it.
        case(
            "each comment classification",
            vec![
                comment_row("issue", "this is wrong", false),
                comment_row("suggestion", "extract a helper", false),
                comment_row("note", "for later", false),
                comment_row("praise", "nicely done", false),
                // A multi-line body, marked with an ellipsis by the pane.
                comment_row("note", "overall:", true),
            ],
            Some(0),
            2,
        ),
        // The summary section, whose heading's text is the kernel's because it
        // names a keystroke.
        case(
            "the summary section",
            vec![
                ReviewRowSnapshot::SummaryHeader {
                    label: "── Review summary (s to add) ──".to_string(),
                },
                comment_row("note", "the shape is right", false),
            ],
            Some(0),
            2,
        ),
        // A whole file's worth of rows in the order the pane lists them, which is
        // the arrangement none of the single-kind cases exercises.
        case(
            "a whole file",
            vec![
                file_row("src/build.rs", "modified", false, false),
                comment_row("issue", "this file does two things", false),
                hunk_row("fn build", false),
                line(
                    "src/build.rs",
                    "context",
                    Some(11),
                    Some(11),
                    "fn build() {",
                ),
                line(
                    "src/build.rs",
                    "add",
                    None,
                    Some(12),
                    "    let total = sum + 1;",
                ),
                comment_row("suggestion", "extract a helper", false),
                ReviewRowSnapshot::SummaryHeader {
                    label: "── Review summary (s to add) ──".to_string(),
                },
            ],
            Some(3),
            2,
        ),
        // An informational row, which the native pane draws unselected however the
        // cursor moved — it is not selectable.
        case(
            "an informational row",
            vec![ReviewRowSnapshot::Info {
                text: "No changes to show for this target.".to_string(),
            }],
            None,
            0,
        ),
    ]
}

/// Start the bundled plugin from the source that ships.
fn host() -> PluginHost {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/plugin/bundled/code-review");
    assert!(
        dir.join("plugin.toml").is_file(),
        "the bundled plugin must live at {}",
        dir.display()
    );
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
    host.render_pane("code-review", "diff")
        .expect("the pane renders")
}

/// Paint a tree the way any pane is painted, so two frames can be compared.
fn paint(tree: &ViewNode, width: u16, height: u16) -> Buffer {
    let area = Rect::new(0, 0, width, height);
    let mut buf = Buffer::empty(area);
    thurbox::ui::plugin_pane::render_tree(
        tree,
        area,
        &thurbox::ui::theme::current(),
        &FrameTable::default(),
        &mut buf,
    );
    buf
}

/// Every text run in a tree, concatenated.
fn text_of(node: &ViewNode) -> String {
    let mut out = String::new();
    fn walk(node: &ViewNode, out: &mut String) {
        if let ViewNode::Text { content, .. } = node {
            out.push_str(content);
        }
        node.children().iter().for_each(|c| walk(c, out));
    }
    walk(node, &mut out);
    out
}

/// The headline claim: for every case, the plugin's tree equals the kernel's.
///
/// Equal trees paint identically, and the kernel's builder is pinned to the
/// untouched native renderer in `ui::code_review`'s own tests — so this is the
/// review's document, reproduced from two declared capabilities.
#[test]
fn the_plugin_builds_the_native_panes_view_tree() {
    let _guard = SERIALIZE.lock().unwrap_or_else(|e| e.into_inner());
    let host = host();

    for case in cases() {
        thurbox::session::pane_context::publish(case.context());
        let plugin = render(&host);
        let native = case.native_tree();
        assert_eq!(
            plugin, native,
            "the plugin's tree diverges from the native stream for `{}`",
            case.name
        );
    }
}

/// Every row kind occurs in the cases above, so the equality cannot be passing on
/// diff lines alone while a header quietly diverges.
#[test]
fn every_row_kind_is_compared() {
    let mut seen: Vec<&str> = cases()
        .iter()
        .flat_map(|c| c.rows.iter())
        .map(|r| r.wire_name())
        .collect();
    seen.sort_unstable();
    seen.dedup();
    for kind in ["file", "hunk", "line", "comment", "summaryHeader", "info"] {
        assert!(seen.contains(&kind), "no case publishes a {kind} row");
    }
}

/// The claim a diff pane cannot do without: the window is the kernel's, so the
/// two panes paint the same cells at a height that forces a scroll.
///
/// A diff is the case where this matters most — a review is almost always taller
/// than its pane — and the fill is exercised here too, since the row tint has to
/// reach the right edge of a *narrow* pane.
#[test]
fn the_plugin_paints_the_native_frame_when_the_pane_scrolls() {
    let _guard = SERIALIZE.lock().unwrap_or_else(|e| e.into_inner());
    let host = host();

    let rows: Vec<ReviewRowSnapshot> = (0..20)
        .map(|i| {
            if i % 3 == 0 {
                added(i + 1, &format!("let v{i} = {i};"))
            } else {
                context(i + 1, i + 1, &format!("let v{i} = {i};"))
            }
        })
        .collect();
    let case = Case {
        name: "a tall diff",
        cursor: Some(rows.len() - 1),
        rows,
        number_width: 2,
    };
    thurbox::session::pane_context::publish(case.context());

    const WIDTH: u16 = 30;
    const HEIGHT: u16 = 5;
    let plugin = paint(&render(&host), WIDTH, HEIGHT);
    let native = paint(&case.native_tree(), WIDTH, HEIGHT);
    assert_eq!(plugin, native, "the plugin's frame is not the native frame");

    // And that frame is the scrolled one — otherwise the two could agree by both
    // being wrong.
    let text: String = (0..HEIGHT)
        .flat_map(|y| (0..WIDTH).map(move |x| (x, y)))
        .map(|(x, y)| plugin[(x, y)].symbol().to_string())
        .collect();
    assert!(text.contains("v19"), "the cursor's row is drawn: {text:?}");
    assert!(!text.contains("v0 "), "the top scrolled off: {text:?}");
}

/// The compared tree must be a whole pane, or the equality could pass on two
/// nearly-empty lists.
#[test]
fn the_compared_tree_is_a_whole_pane() {
    let case = cases()
        .into_iter()
        .find(|c| c.name == "every token kind")
        .expect("the token case");
    let tree = case.native_tree();
    let (children, selected) = match &tree {
        ViewNode::List {
            children, selected, ..
        } => (children, selected),
        other => panic!("expected a list, got {}", other.kind_name()),
    };
    assert_eq!(children.len(), 6);
    assert_eq!(*selected, Some(0), "the list carries its cursor");
    // Every row is a line of a gutter, its token runs, and a fill — which one
    // text node could not have styled apart, and which is the point of the pane.
    for row in children {
        let runs = row.children();
        assert!(runs.len() >= 3, "{row:#?}");
        assert!(
            matches!(runs.last(), Some(ViewNode::Fill { .. })),
            "every row ends in the fill that carries its tint to the edge: {row:#?}"
        );
    }

    // A file header ends in a fill too, and that one is a **rule** rather than a
    // tint: it is the reason the row is expressible without a width at all.
    let file = cases()
        .into_iter()
        .find(|c| c.name == "each file status")
        .expect("the status case");
    match file.native_tree().children()[0].children().last() {
        Some(ViewNode::Fill { glyph, .. }) => assert_eq!(*glyph, '─'),
        other => panic!("a file header must end in its rule: {other:?}"),
    }
}

/// **The host's bounds, measured against a real diff.** This is the pane that
/// finds them, and what it finds is not what the brief predicted.
///
/// `MAX_NODES` bounds a whole converted tree while this pane's cost is per row
/// *and* per token inside a row, so it is the first pane the model cannot bound by
/// publishing fewer rows alone. But the node budget is **not the bound that bites
/// first**: building the rows costs Luau instructions, and the interrupt budget
/// stops the plugin *before* its tree is ever converted. So the effective limit on
/// a plugin diff pane is the execution budget, and the node budget is the second
/// wall behind it.
///
/// Four facts, in the order they matter:
///
/// 1. a representative row costs many nodes, not a fixed handful;
/// 2. a full publication (`MAX_REVIEW_ROWS` of them) renders, so the cap works;
/// 3. the node budget would allow only about `MAX_NODES / per-row` rows — a diff
///    of a few hundred lines is past it;
/// 4. and at that size the plugin is refused for its **execution** budget, not its
///    node budget: the row-building loop runs out of instructions first.
#[test]
fn the_hosts_bounds_are_reached_by_an_ordinary_diff() {
    let _guard = SERIALIZE.lock().unwrap_or_else(|e| e.into_inner());
    let host = host();

    let body = "    let total: Vec<Row> = rows.iter().map(|r| r.id).collect();";
    let dense = |n: usize| -> Vec<ReviewRowSnapshot> {
        (0..n)
            .map(|i| context(i as u32 + 1, i as u32 + 1, body))
            .collect()
    };

    // 1. One row's cost, from the kernel's own builder: a gutter, a fill, and one
    //    node per syntax token.
    let one = review_stream_tree(&dense(1), None, 2);
    let per_row = one.node_count() - 1; // less the list itself
    assert!(
        per_row > 10,
        "a row of real code costs many nodes, not a handful: {per_row}"
    );

    // 2. A full publication fits, which is what the cap is chosen for.
    let cap = thurbox::session::pane_context::MAX_REVIEW_ROWS;
    let full = Case {
        name: "a full publication",
        rows: dense(cap),
        cursor: Some(0),
        number_width: 2,
    };
    assert!(
        full.native_tree().node_count() < MAX_NODES,
        "{cap} rows at {per_row} nodes each must fit in {MAX_NODES}"
    );
    thurbox::session::pane_context::publish(full.context());
    assert_eq!(render(&host), full.native_tree());

    // 3. The node budget alone would allow only this many rows, which is a small
    //    fraction of a real review.
    let node_limit = MAX_NODES / per_row;
    assert!(
        node_limit < 300,
        "a syntax-coloured diff pane fits only {node_limit} rows in {MAX_NODES} nodes"
    );

    // 4. And at that size the refusal is the *execution* budget, reached while the
    //    plugin is still building rows — so the tree the node budget would have
    //    refused is never even returned. Asserted on the message because which
    //    bound stopped it is the finding.
    let rows = node_limit + 10;
    let over = Case {
        name: "an ordinary diff",
        rows: dense(rows),
        cursor: Some(0),
        number_width: 2,
    };
    assert!(
        over.native_tree().node_count() > MAX_NODES,
        "the tree would also have exceeded the node budget"
    );
    thurbox::session::pane_context::publish(over.context());
    let message = host
        .render_pane("code-review", "diff")
        .expect_err("a diff past the host's bounds must be refused")
        .to_string();
    assert!(
        message.contains("execution budget"),
        "the instruction budget is the wall a diff pane hits first: {message}"
    );
}

/// **The document is drawn; the behaviour is absent rather than approximated.**
///
/// The first half is what the second step closed: the rows a reader needs to tell
/// which file a line belongs to, that a file is folded, that a hunk is reviewed and
/// what a comment says are all drawn, and asserted **present** so a regression that
/// dropped them again would fail here.
///
/// The second half is what remains, and none of it is approximated: a pane that
/// drew a plausible find bar or a target picker it cannot operate would agree with
/// nothing and make the record a claim about a pane that does not exist.
#[test]
fn the_out_of_scope_surface_is_absent_rather_than_approximated() {
    let _guard = SERIALIZE.lock().unwrap_or_else(|e| e.into_inner());
    let host = host();
    let case = cases()
        .into_iter()
        .find(|c| c.name == "a whole file")
        .expect("the whole-file case");
    thurbox::session::pane_context::publish(case.context());

    let text = text_of(&render(&host));
    // The document it draws, row kind by row kind.
    for present in [
        "fn build() {", // a diff body
        "@@ -11,4 +11,5 @@",
        "▾ ",        // a file header's fold chevron
        "M ",        // its status letter
        "+3",        // its insertion count
        "▸ [Issue]", // a comment's badge
        "▸ [Suggestion]",
        "Review summary",
    ] {
        assert!(
            !present.is_empty() && text.contains(present),
            "the plugin's pane must draw {present:?}: {text:?}"
        );
    }
    // The rule is a fill rather than a text run, so it is asserted on the node and
    // not on the text — a pane that drew a fixed-width run of `─` would look right
    // at one width and wrong at every other.
    fn has_rule(node: &ViewNode) -> bool {
        matches!(node, ViewNode::Fill { glyph: '─', .. }) || node.children().iter().any(has_rule)
    }
    assert!(
        has_rule(&render(&host)),
        "a file header's rule must be a fill the kernel resolves"
    );

    // And the marks, from the case that carries them.
    let marks = cases()
        .into_iter()
        .find(|c| c.name == "folded and reviewed files")
        .expect("the marks case");
    thurbox::session::pane_context::publish(marks.context());
    let marked = text_of(&render(&host));
    assert!(marked.contains('✓'), "a reviewed file's mark: {marked:?}");
    assert!(marked.contains('▸'), "a folded file's chevron: {marked:?}");

    // What is still behaviour, and therefore still absent.
    thurbox::session::pane_context::publish(case.context());
    let text = text_of(&render(&host));
    for absent in [
        "Search",  // the find bar
        "Target",  // the footer's target button
        "Comment", // the footer's compose button
        "Send→Agent",
        "Wrap",   // the wrap toggle's pill
        "NoWrap", //
        "Old",    // a side-by-side column heading
    ] {
        assert!(
            !text.contains(absent),
            "the plugin's pane must not imitate {absent:?}: {text:?}"
        );
    }
}

/// The plugin must hold no surface a user's plugin could not declare: its reach is
/// exactly its manifest's capability list, and it is two things — neither of which
/// is a version-control capability.
#[test]
fn the_plugin_declares_every_power_it_uses() {
    use thurbox::session::plugin_manifest::Capability;
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/plugin/bundled/code-review");
    let outcome = discovery::discover_in(&[dir], None);
    let plugin = outcome.get("code-review").expect("discovered");
    let declared = &plugin.manifest.capabilities;
    for required in [Capability::Render, Capability::Review] {
        assert!(declared.contains(&required), "{required} must be declared");
    }
    assert_eq!(
        declared.len(),
        2,
        "a bundled pane that quietly asked for more would stop being evidence \
         about what a third party can build: {declared:?}"
    );
    // The pane that draws a diff holds no repository: `review` publishes the diff
    // the user opened, and the host defines no capability that runs git at all.
    for absent in ["git", "vcs", "fs", "exec"] {
        assert!(
            !Capability::all().iter().any(|c| c.as_str() == absent),
            "the vocabulary must define no `{absent}` capability"
        );
    }
    // Drawing the whole document did not widen the reach: no write capability, and
    // no input — the rows it gained are read, and the behaviour it still lacks is
    // what a write would be for.
    for refused in [
        Capability::TasksWrite,
        Capability::AutomationsWrite,
        Capability::Input,
    ] {
        assert!(
            !declared.contains(&refused),
            "{refused} is not what this pane's missing behaviour needs, and \
             declaring it would claim a parity the pane does not have"
        );
    }

    // Additive port: the pane must not appear on anyone's screen unasked, since
    // the native view is still the one thurbox draws.
    assert!(
        !plugin.manifest.panes[0].default_visible,
        "the reproduction must be hidden by default"
    );
}

/// Before the host has published anything the reader answers "no review", so the
/// plugin's first render must produce the empty-state pane rather than an error.
#[test]
fn the_first_render_before_any_publication_succeeds() {
    let _guard = SERIALIZE.lock().unwrap_or_else(|e| e.into_inner());
    let host = host();
    thurbox::session::pane_context::publish(PaneContext::default());
    assert_eq!(
        render(&host),
        review_stream_tree(&[], None, 0),
        "an empty publication draws the same pane a target with no changes does"
    );
}
