//! The bundled file-viewer plugin **is** thurbox's file viewer.
//!
//! `src/ui/file_viewer.rs` is deleted (ADR-58), so this is no longer a claim about a
//! copy: the tree a user navigates with `j`/`k` is the one this plugin builds. What
//! holds it to the pane it replaced is the ten checked-in recordings under
//! `tests/snapshots/`, every one generated from `ui::file_viewer::file_tree` while that
//! builder existed — so they are a statement about the **pane**, not about this plugin,
//! and they could not have been produced after the deletion.
//!
//! ## What the handover took from this file, and what it could not
//!
//! Each case used to assert two edges (ADR-42): `native == snapshot`, establishable only
//! while the native builder was here, and `plugin == native`, the port's original proof.
//! Their conjunction is what survives — `plugin == snapshot` — with a baseline that was
//! proven rather than assumed. Recording from the plugin instead would have inverted it:
//! a plugin defect would have become the expectation, and the test could never fail for
//! the reason it exists.
//!
//! The two **frame** tests keep their claims and lose their comparison. What they assert
//! is that the *kernel* resolves the window and reserves the track — facts a tree cannot
//! show, since the trees are equal at every size — so each now paints the plugin's tree
//! and asserts the resulting cells directly: the cursor's row is on screen and the top
//! has scrolled off; the track's column carries a thumb and no row text, and a list that
//! declares no track paints differently.
//!
//! The **search bar** is no longer an enumerated divergence. It is kernel chrome drawn
//! into the seat below this pane's frame (ADR-58), and what this file still asserts is
//! that the plugin does not imitate it.
//!
//! It lives in `tests/` because it must see both `session::pane_context` and
//! `plugin::PluginHost`, and an integration test is not part of the library's module
//! graph, so `tests/architecture_rules.rs` stays untouched.

#![cfg(feature = "plugins")]

use std::path::PathBuf;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

use thurbox::plugin::{discovery, ExecutionBounds, PluginHost};
use thurbox::session::motion::FrameTable;
use thurbox::session::pane_context::{FileNodeSnapshot, FilesSnapshot, PaneContext};
use thurbox::session::view_tree::ViewNode;

/// The process-wide snapshot slot is global, so every case here runs one at a
/// time — otherwise one case's publication would answer another's reader.
static SERIALIZE: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// The recorder that turns a view tree into the checked-in expectation.
///
/// Shared across every recorded pane rather than copied: its exhaustiveness over
/// the view tree is what stops a compact recording from omitting a fact, and that
/// property is worth as much as the number of copies of it. See the module's own
/// note.
mod view_tree_record;

/// One case: the rows the kernel publishes, and what the pane must draw for them.
///
/// The rows are the **published** type since the handover — `FileViewerState::rows`
/// yields it directly, so a case is built out of exactly what the kernel would put on
/// the wire rather than out of a fixture converted to it.
struct Case {
    name: &'static str,
    rows: Vec<FileNodeSnapshot>,
    selected: Option<usize>,
    nerd_font: bool,
}

fn row(name: &str, depth: usize, is_dir: bool, expanded: bool, matched: bool) -> FileNodeSnapshot {
    FileNodeSnapshot {
        name: name.to_string(),
        depth,
        is_dir,
        expanded,
        matched,
    }
}

fn dir(name: &str, depth: usize, expanded: bool) -> FileNodeSnapshot {
    row(name, depth, true, expanded, true)
}

fn file(name: &str, depth: usize) -> FileNodeSnapshot {
    row(name, depth, false, false, true)
}

impl Case {
    /// The publication a plugin reads.
    ///
    /// Mirrors `App::build_files_snapshot`: the kernel publishes a basename, a
    /// depth, the expansion state, the search's verdict and the cursor's index —
    /// and no path, no glyph and no colour.
    fn context(&self) -> PaneContext {
        PaneContext {
            files: FilesSnapshot {
                nodes: self.rows.clone(),
                selected: self.selected,
                nerd_font: self.nerd_font,
            },
            ..PaneContext::default()
        }
    }

    /// The recording's name: the case name, slugified.
    ///
    /// Derived from the name rather than written twice, so a renamed case cannot
    /// keep asserting against another case's recording.
    fn snapshot_name(&self) -> String {
        self.name.replace(' ', "-")
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

/// The headline claim: for every case, the pane's tree is the recorded one. Equal
/// trees paint identically, so this is byte-identity of the pane.
///
/// The recordings were taken from `ui::file_viewer::file_tree` before the handover
/// deleted it, so what this asserts is that the pane still draws what the *native* pane
/// drew — a baseline that was proven rather than assumed, and one this file could not
/// re-establish now.
#[test]
fn the_plugin_builds_the_native_panes_view_tree() {
    let _guard = SERIALIZE.lock().unwrap_or_else(|e| e.into_inner());
    let host = host();

    for case in cases() {
        thurbox::session::pane_context::publish(case.context());
        insta::assert_snapshot!(case.snapshot_name(), view_tree_record::tree(&render(&host)));
    }
}

/// The claim the previous port could not make, and the one the recordings cannot
/// carry: the pane scrolls. The tasks pane's plugin copy drew from the first row
/// because the window lived in the kernel and a plugin has no height; this pane's list
/// declares its cursor's row instead, so the **kernel** windows it — which only a paint
/// can show, since the tree is the same tree at every size.
#[test]
fn the_plugin_paints_the_native_frame_when_the_pane_scrolls() {
    let _guard = SERIALIZE.lock().unwrap_or_else(|e| e.into_inner());
    let host = host();

    let rows: Vec<FileNodeSnapshot> = std::iter::once(dir("repo", 0, true))
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

    // The frame is the scrolled one: the cursor's row is on screen and the top of the
    // list is not. Asserted on the cells rather than against a second builder, which
    // the handover took away.
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
    let _guard = SERIALIZE.lock().unwrap_or_else(|e| e.into_inner());
    let host = host();
    thurbox::session::pane_context::publish(case.context());
    let tree = render(&host);
    let (children, selected, scrollbar) = match &tree {
        ViewNode::List {
            children,
            selected,
            scrollbar,
        } => (children, selected, scrollbar),
        other => panic!("expected a list, got {}", other.kind_name()),
    };
    assert_eq!(children.len(), 6);
    assert_eq!(*selected, Some(0), "the list carries its cursor");
    assert!(*scrollbar, "the list asks for its scroll track");
    // Every row is a line of two runs — the prefix and the name — which one text
    // node could not have styled apart.
    assert!(
        children.iter().all(|row| row.children().len() == 2),
        "{tree:#?}"
    );
}

/// **The search bar is the kernel's, and the pane must not imitate it.** The bar is a
/// three-row bordered ` Search (2/5) ` block with a `/ ` prefix, the query scrolled to
/// its end, and a block cursor. It was an enumerated divergence while this was a
/// reproduction; the handover made it **seat chrome** (ADR-58), drawn by the kernel into
/// the rows below this pane's frame, because the query, the caret and the counter are
/// kernel state that `Capability::Files` publishes none of.
///
/// So what is asserted here is the boundary: the search's effect on the *rows* is the
/// pane's (and is recorded by the `a-running-search` case above), and the bar itself is
/// not drawn here at all.
#[test]
fn the_search_bar_is_the_kernels_and_the_pane_does_not_imitate_it() {
    let _guard = SERIALIZE.lock().unwrap_or_else(|e| e.into_inner());
    let host = host();
    let case = cases()
        .into_iter()
        .find(|c| c.name == "a running search")
        .expect("the search case");
    thurbox::session::pane_context::publish(case.context());

    // The plugin draws nothing resembling the bar: no query, no counter, no
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

/// **Divergence 2 is closed: the plugin's copy grows the scroll track.**
///
/// It used to be chrome the native pane reserved *around* its tree, so a plugin
/// pane had no track at all. The list node now declares that it scrolls and the
/// renderer reserves the column, which means the two panes agree about the
/// thumb's column as well as about the rows — the whole frame, not the rows in it.
///
/// The track a plugin draws is an **indicator**, not a control: the painter reports row
/// hitboxes and not the track's rect, so the kernel records no drag target for a plugin
/// pane and the thumb cannot be dragged. The native pane's `ScrollTarget::FileViewer` went
/// with it (ADR-58); wheel scrolling over the column, which is resolved from the layout,
/// did not.
#[test]
fn the_plugin_paints_the_native_frame_including_the_scroll_track() {
    let _guard = SERIALIZE.lock().unwrap_or_else(|e| e.into_inner());
    let host = host();
    // A cursor a third of the way down a list five times the pane's height, so the
    // thumb is neither at the top nor at the bottom — a track drawn at the wrong
    // position would still be a track.
    let rows: Vec<FileNodeSnapshot> = (0..20).map(|i| file(&format!("f{i}.rs"), 0)).collect();
    let case = Case {
        name: "overflowing",
        selected: Some(7),
        rows,
        nerd_font: false,
    };
    thurbox::session::pane_context::publish(case.context());

    const WIDTH: u16 = 16;
    const HEIGHT: u16 = 4;
    let tree = render(&host);
    let plugin = paint(&tree, WIDTH, HEIGHT);

    // The declaration is what draws it: the same rows *without* it paint a different
    // frame, so the assertions below are evidence that a track was drawn rather than
    // that nothing was.
    let untracked = ViewNode::selectable_list(
        match tree {
            ViewNode::List { children, .. } => children,
            other => panic!("expected a list, got {}", other.kind_name()),
        },
        case.selected,
    );
    assert_ne!(
        plugin,
        paint(&untracked, WIDTH, HEIGHT),
        "a pane with no track must not paint the same frame as one with it"
    );

    // The track is where the pane's rows are not: the rightmost column carries no
    // row content, and it is not blank either.
    let column: String = (0..HEIGHT)
        .map(|y| plugin[(WIDTH - 1, y)].symbol().to_string())
        .collect();
    assert!(!column.contains('f'), "row text in the track: {column:?}");
    assert!(
        column.chars().any(|c| c != ' '),
        "a thumb is drawn: {column:?}"
    );
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
/// plugin's first render must produce the empty-state pane rather than an error — and
/// the *same* empty state a session with no folders gets, which is the recorded
/// `no-folders` case.
#[test]
fn the_first_render_before_any_publication_succeeds() {
    let _guard = SERIALIZE.lock().unwrap_or_else(|e| e.into_inner());
    let host = host();
    let empty = cases()
        .into_iter()
        .find(|c| c.name == "no folders")
        .expect("the empty case");
    thurbox::session::pane_context::publish(empty.context());
    let no_folders = render(&host);

    thurbox::session::pane_context::publish(PaneContext::default());
    assert_eq!(
        render(&host),
        no_folders,
        "an empty publication draws the same pane a session with no folders does"
    );
}
