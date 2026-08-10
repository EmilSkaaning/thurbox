//! The bundled file-viewer plugin renders the native file tree.
//!
//! The same claim `tests/bundled_info_panel.rs` and `tests/bundled_tasks_panel.rs`
//! make for the first two ported panes, plus one the tasks pane could not make.
//!
//! The tasks port had to record an open gap: the kernel windowed its list around
//! the selection and a plugin, knowing no height, drew from the first row — so a
//! selection below the fold was invisible in its copy. That gap is closed here by
//! a list node that carries the row its cursor is on, which means this pane's
//! equality is not only *tree* equality but **frame** equality at a size where the
//! pane scrolls ([`the_plugin_paints_the_native_frame_when_the_pane_scrolls`]).
//! That is the property a file tree could not do without.
//!
//! What is *not* reproduced is stated rather than hidden: the search **bar** below
//! the tree, and the scrollbar. Both are pinned as enumerated divergences with the
//! host features that would close them named.
//!
//! It lives in `tests/` for the same reason as the others: this is the one place
//! that must see both `ui::file_viewer` and `plugin::PluginHost`, and an
//! integration test is not part of the library's module graph, so
//! `tests/architecture_rules.rs` stays untouched.

#![cfg(feature = "plugins")]

use std::path::PathBuf;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

use thurbox::plugin::{discovery, ExecutionBounds, PluginHost};
use thurbox::session::motion::FrameTable;
use thurbox::session::pane_context::{FileNodeSnapshot, FilesSnapshot, PaneContext};
use thurbox::session::view_tree::ViewNode;
use thurbox::ui::file_viewer::{file_tree, FileRow};

/// The process-wide snapshot slot is global, so every case here runs one at a
/// time — otherwise one case's publication would answer another's reader.
static SERIALIZE: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// One comparison case. Both sides are derived from the same rows, so an equality
/// failure is a statement about the plugin rather than about two hand-written
/// fixtures drifting apart.
struct Case {
    name: &'static str,
    rows: Vec<FileRow>,
    selected: Option<usize>,
    nerd_font: bool,
}

fn row(name: &str, depth: usize, is_dir: bool, expanded: bool, matched: bool) -> FileRow {
    FileRow {
        name: name.to_string(),
        depth,
        is_dir,
        expanded,
        matched,
    }
}

fn dir(name: &str, depth: usize, expanded: bool) -> FileRow {
    row(name, depth, true, expanded, true)
}

fn file(name: &str, depth: usize) -> FileRow {
    row(name, depth, false, false, true)
}

impl Case {
    fn native_tree(&self) -> ViewNode {
        file_tree(&self.rows, self.selected, self.nerd_font)
    }

    /// The same rows as the snapshot a plugin reads.
    ///
    /// Mirrors `App::build_files_snapshot`: the kernel publishes a basename, a
    /// depth, the expansion state, the search's verdict and the cursor's index —
    /// and no path, no glyph and no colour.
    fn context(&self) -> PaneContext {
        PaneContext {
            files: FilesSnapshot {
                nodes: self
                    .rows
                    .iter()
                    .map(|r| FileNodeSnapshot {
                        name: r.name.clone(),
                        depth: r.depth,
                        is_dir: r.is_dir,
                        expanded: r.expanded,
                        matched: r.matched,
                    })
                    .collect(),
                selected: self.selected,
                nerd_font: self.nerd_font,
            },
            ..PaneContext::default()
        }
    }
}

/// The variants. Each isolates one decision the pane makes, so a failure names
/// which part of a row the plugin gets wrong rather than only that it does.
fn cases() -> Vec<Case> {
    let case = |name, rows, selected, nerd_font| Case {
        name,
        rows,
        selected,
        nerd_font,
    };
    vec![
        // The empty pane: a session with no directories, and what a plugin sees
        // before the native viewer has been opened at all.
        case("no folders", Vec::new(), None, false),
        // A whole tree: an expanded root, an expanded and a collapsed child
        // directory, and files at two depths. Six markers between them.
        case(
            "an expanded tree with a collapsed directory",
            vec![
                dir("repo", 0, true),
                dir("src", 1, true),
                file("main.rs", 2),
                file("lib.rs", 2),
                dir("target", 1, false),
                file("README.md", 1),
            ],
            Some(0),
            false,
        ),
        // The same shape with the other glyph set, which is the only thing the
        // published `nerdFont` flag exists to choose.
        case(
            "the nerd-font marker set",
            vec![
                dir("repo", 0, true),
                dir("src", 1, false),
                file("main.rs", 1),
            ],
            Some(1),
            true,
        ),
        // The cursor on a directory and on a file: the selection has to beat both
        // the accent a matched directory takes and the primary text a file takes.
        case(
            "the cursor on a directory",
            vec![dir("repo", 0, true), file("main.rs", 1)],
            Some(0),
            false,
        ),
        case(
            "the cursor on a file",
            vec![dir("repo", 0, true), file("main.rs", 1)],
            Some(1),
            false,
        ),
        // A running search: matched rows keep their colours, excluded ones recede
        // to muted — including an excluded *directory*, which loses its accent and
        // its bold.
        case(
            "a running search",
            vec![
                dir("host", 0, true),
                file("hosts.toml", 1),
                row("unrelated.rs", 1, false, false, false),
                row("also-out", 1, true, false, false),
            ],
            Some(1),
            false,
        ),
        // The cursor on a row the search excluded: selection wins over the
        // exclusion, which is the layering most likely to be got wrong.
        case(
            "a selected row the search excluded",
            vec![dir("repo", 0, true), row("nope.rs", 1, false, false, false)],
            Some(1),
            false,
        ),
        // Deep nesting, so the indentation is exercised past one level.
        case(
            "deep nesting",
            vec![
                dir("a", 0, true),
                dir("b", 1, true),
                dir("c", 2, true),
                file("d.rs", 3),
            ],
            Some(3),
            false,
        ),
        // Multi-byte and double-width names: the runs after them must not drift,
        // and a plugin composing strings byte-wise would show it here.
        case(
            "multi-byte and wide names",
            vec![dir("répertoire", 0, true), file("日本語.md", 1)],
            Some(1),
            false,
        ),
        // A tree with no cursor at all, which is not the same as a cursor on row
        // zero: the list must carry no selection, so it neither highlights nor
        // scrolls.
        case(
            "a tree with no cursor",
            vec![dir("repo", 0, true), file("main.rs", 1)],
            None,
            false,
        ),
    ]
}

/// Start the bundled plugin from the source that ships.
fn host() -> PluginHost {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/plugin/bundled/file-viewer");
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
    host.render_pane("file-viewer", "files")
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

/// The headline claim: for every case, the plugin's tree equals the native pane's.
/// Equal trees paint identically, so this is byte-identity of the pane.
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
            "the plugin's tree diverges from the native pane for `{}`",
            case.name
        );
    }
}

/// The claim the previous port could not make. The tasks pane's plugin copy drew
/// from the first row because the window lived in the kernel and a plugin has no
/// height; this pane's list declares its cursor's row instead, so the *kernel*
/// windows the plugin's list exactly as it windows the native one — and the two
/// panes paint the same cells at a height that forces a scroll.
#[test]
fn the_plugin_paints_the_native_frame_when_the_pane_scrolls() {
    let _guard = SERIALIZE.lock().unwrap_or_else(|e| e.into_inner());
    let host = host();

    let rows: Vec<FileRow> = std::iter::once(dir("repo", 0, true))
        .chain((0..15).map(|i| file(&format!("f{i}.rs"), 1)))
        .collect();
    // The cursor at the very bottom of a list four times the pane's height: the
    // case where a copy that could not scroll shows none of what matters.
    let case = Case {
        name: "a tall tree",
        selected: Some(rows.len() - 1),
        rows,
        nerd_font: false,
    };
    thurbox::session::pane_context::publish(case.context());

    const WIDTH: u16 = 24;
    const HEIGHT: u16 = 4;
    let plugin = paint(&render(&host), WIDTH, HEIGHT);
    let native = paint(&case.native_tree(), WIDTH, HEIGHT);
    assert_eq!(plugin, native, "the plugin's frame is not the native frame");

    // And that frame is actually the scrolled one — otherwise the two could agree
    // by both being wrong.
    let text: String = (0..HEIGHT)
        .flat_map(|y| (0..WIDTH).map(move |x| (x, y)))
        .map(|(x, y)| plugin[(x, y)].symbol().to_string())
        .collect();
    assert!(
        text.contains("f14.rs"),
        "the cursor's row is drawn: {text:?}"
    );
    assert!(!text.contains("repo"), "the top scrolled off: {text:?}");
}

/// The compared tree must be a whole pane, or the equality could pass on two
/// nearly-empty lists.
#[test]
fn the_compared_tree_is_a_whole_pane() {
    let case = cases()
        .into_iter()
        .find(|c| c.name == "an expanded tree with a collapsed directory")
        .expect("the tree case");
    let tree = case.native_tree();
    let (children, selected) = match &tree {
        ViewNode::List { children, selected } => (children, selected),
        other => panic!("expected a list, got {}", other.kind_name()),
    };
    assert_eq!(children.len(), 6);
    assert_eq!(*selected, Some(0), "the list carries its cursor");
    // Every row is a line of two runs — the prefix and the name — which one text
    // node could not have styled apart.
    assert!(
        children.iter().all(|row| row.children().len() == 2),
        "{tree:#?}"
    );
}

/// **Enumerated divergence 1: the search bar.** The native pane draws a three-row
/// bordered ` Search (2/5) ` block below the tree, with a `/ ` prefix, the query
/// scrolled to its end, and a block cursor. The host surface can describe none of
/// the three things that needs — a bordered container node, a cursor appearance,
/// and a fixed-height region anchored to the bottom of a pane — and the match
/// counter would need the query text, which the `files` capability deliberately
/// does not publish.
///
/// So the bar is out of scope. What is *not* out of scope, and is asserted here,
/// is the search's effect on the rows: a plugin reproduces the matched and
/// excluded appearances exactly.
#[test]
fn the_search_bar_is_out_of_scope_but_its_effect_on_the_rows_is_not() {
    let _guard = SERIALIZE.lock().unwrap_or_else(|e| e.into_inner());
    let host = host();
    let case = cases()
        .into_iter()
        .find(|c| c.name == "a running search")
        .expect("the search case");
    thurbox::session::pane_context::publish(case.context());

    // The rows agree, including which of them the search excluded.
    assert_eq!(render(&host), case.native_tree());

    // And the plugin draws nothing resembling the bar: no query, no counter, no
    // border. Asserted on the text of every run, so a later attempt to fake one
    // fails here rather than shipping half a search.
    let mut text = String::new();
    fn walk(node: &ViewNode, out: &mut String) {
        if let ViewNode::Text { content, .. } = node {
            out.push_str(content);
        }
        node.children().iter().for_each(|c| walk(c, out));
    }
    walk(&render(&host), &mut text);
    for absent in ["Search", "/ ", "(", "─", "│"] {
        assert!(
            !text.contains(absent),
            "the plugin's pane must not imitate the search bar: found {absent:?} in {text:?}"
        );
    }
}

/// **Enumerated divergence 2: the scrollbar.** The native pane reserves its
/// rightmost column for a draggable track *before* the tree is painted, so the
/// track is chrome outside the tree — like the pane border. A plugin pane has no
/// scrollbar.
///
/// Closing it would be a `scrollbar` field on the list node: the renderer already
/// resolves the window, so it has the numbers. It is deliberately not added,
/// because the native pane reserves its track outside the tree and moving the
/// reservation inside changes the native pane's layout — Phase 6's business, not a
/// reproduction's.
#[test]
fn the_scrollbar_is_the_native_panes_chrome_only() {
    let _guard = SERIALIZE.lock().unwrap_or_else(|e| e.into_inner());
    let host = host();
    let rows: Vec<FileRow> = (0..20).map(|i| file(&format!("f{i}.rs"), 0)).collect();
    let case = Case {
        name: "overflowing",
        selected: Some(0),
        rows,
        nerd_font: false,
    };
    thurbox::session::pane_context::publish(case.context());

    // The tree the plugin returns is the tree the native pane builds — the track
    // is not in either of them, because the native pane draws it around the tree
    // rather than inside it.
    assert_eq!(render(&host), case.native_tree());
    let mut text = String::new();
    fn walk(node: &ViewNode, out: &mut String) {
        if let ViewNode::Text { content, .. } = node {
            out.push_str(content);
        }
        node.children().iter().for_each(|c| walk(c, out));
    }
    walk(&render(&host), &mut text);
    for glyph in ["█", "░", "▐", "│"] {
        assert!(!text.contains(glyph), "no track in the tree: {text:?}");
    }
}

/// The plugin must hold no surface a user's plugin could not declare: its reach is
/// exactly its manifest's capability list, and it is two things — neither of which
/// is a filesystem capability.
#[test]
fn the_plugin_declares_every_power_it_uses() {
    use thurbox::session::plugin_manifest::Capability;
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/plugin/bundled/file-viewer");
    let outcome = discovery::discover_in(&[dir], None);
    let plugin = outcome.get("file-viewer").expect("discovered");
    let declared = &plugin.manifest.capabilities;
    for required in [Capability::Render, Capability::Files] {
        assert!(declared.contains(&required), "{required} must be declared");
    }
    assert_eq!(
        declared.len(),
        2,
        "a bundled pane that quietly asked for more would stop being evidence \
         about what a third party can build: {declared:?}"
    );
    // The pane that draws a file tree does not hold the filesystem: `files`
    // publishes the tree the user opened, and the host defines no capability that
    // reaches a directory or a file at all.
    assert!(
        !Capability::all().iter().any(|c| c.as_str() == "fs"),
        "the vocabulary must define no filesystem capability"
    );

    // Additive port: the pane must not appear on anyone's screen unasked, since
    // the native pane is still the one thurbox draws.
    assert!(
        !plugin.manifest.panes[0].default_visible,
        "the reproduction must be hidden by default"
    );
}

/// Before the host has published anything the reader answers "no tree", so the
/// plugin's first render must produce the empty-state pane rather than an error.
#[test]
fn the_first_render_before_any_publication_succeeds() {
    let _guard = SERIALIZE.lock().unwrap_or_else(|e| e.into_inner());
    let host = host();
    thurbox::session::pane_context::publish(PaneContext::default());
    assert_eq!(
        render(&host),
        file_tree(&[], None, false),
        "an empty publication draws the same pane a session with no folders does"
    );
}
