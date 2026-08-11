//! The bundled tasks plugin renders the native tasks pane.
//!
//! The same claim `tests/bundled_info_panel.rs` makes for the info panel, for the
//! second ported pane: the Luau plugin's view tree must **equal** the one
//! `ui::tasks_panel::tasks_tree` builds from the same rows. Since the same
//! renderer paints both (`ui::plugin_pane::render_tree`), an equal tree is a
//! byte-identical pane without comparing a frame — and a failure localises to a
//! node rather than to a cell.
//!
//! What is different here, and is the finding this pane produced: the tasks pane
//! is the first ported pane whose rows depend on its **resolved size**. It fits
//! each title to the column, and a plugin has no width. So the comparison is run
//! at a size where that adjusts nothing — asserted, not assumed, by
//! [`the_comparison_size_adjusts_nothing`] — and the one case where it bites is
//! pinned as an enumerated divergence rather than absorbed by weakening the
//! equality.
//!
//! **Height is no longer one of them.** The plugin declares the row its cursor is
//! on and the kernel windows the list, so the claim here is the file viewer's
//! stronger one: not only equal trees but the same painted **frame** at a size
//! where the pane scrolls ([`the_plugin_paints_the_native_frame_when_the_pane_scrolls`]).
//!
//! It lives in `tests/` for the same reason as the info panel's: this is the one
//! place that must see both `ui::tasks_panel` and `plugin::PluginHost`, and an
//! integration test is not part of the library's module graph, so
//! `tests/architecture_rules.rs` stays untouched.
//!
//! ## Two edges, because a handover deletes one of them
//!
//! The equality is **differential** — it names `tasks_tree`, which lives in
//! `src/ui/tasks_panel.rs`, the module a handover removes. On that day the
//! comparison has nothing on its right-hand side, and the repair that compiles is
//! to drop it: what remains is a test that the plugin renders without erroring,
//! satisfied equally by a pane drawing one wrong row and by one drawing twenty.
//! The acceptance snapshots do not cover the gap either — every one is captured
//! with no active session and no tasks panel open, so none holds a row of this
//! pane.
//!
//! So each case asserts **both** edges while both sides exist (ADR-42):
//!
//! * `native == snapshot` — the recording in `tests/snapshots/` is the *native
//!   pane's* tree. This is the edge that gives it provenance, and it is
//!   establishable only while the native builder is here.
//! * `plugin == native` — the port's original proof, unchanged.
//!
//! Their conjunction is what a handover inherits (`plugin == snapshot`) with a
//! baseline that was proven rather than assumed. Recording from the plugin instead
//! would invert it: a plugin defect would become the expectation, and the test
//! could never fail for the reason it exists.
//!
//! Two of this file's tests carry no recording, and for opposite reasons.
//! [`a_title_wider_than_the_column_is_fitted_by_the_kernel_only`] asserts the two
//! panes **differ**, and an expectation states what a pane should be — a case that
//! exists to differ has nothing for one to say.
//! [`the_plugin_paints_the_native_frame_when_the_pane_scrolls`] compares a painted
//! frame rather than a tree, because the two trees are equal at *every* height and
//! only a paint shows the window was resolved the same way; a recording of the
//! tree would restate the per-case equality and say nothing about the window.

#![cfg(feature = "plugins")]

use std::path::PathBuf;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

use thurbox::plugin::{discovery, ExecutionBounds, PluginHost};
use thurbox::session::motion::FrameTable;
use thurbox::session::pane_context::{PaneContext, TaskSnapshot, TasksSnapshot};
use thurbox::session::view_tree::ViewNode;
use thurbox::session::TaskStatus;
use thurbox::ui::FocusLevel;

/// The recorder that turns a view tree into the checked-in expectation.
///
/// Shared across every recorded pane rather than copied: its exhaustiveness over
/// the view tree is what stops a compact recording from omitting a fact, and that
/// property is worth as much as the number of copies of it. See the module's own
/// note.
mod view_tree_record;

/// The process-wide snapshot slot is global, so every case here runs one at a
/// time — otherwise one case's publication would answer another's reader.
static SERIALIZE: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// One row of a fixture, in the form the *model* has it: an untruncated title and
/// the search's verdict on it.
struct Row {
    title: &'static str,
    status: TaskStatus,
    matches: Vec<usize>,
    dimmed: bool,
    linked: bool,
}

fn row(title: &'static str, status: TaskStatus) -> Row {
    Row {
        title,
        status,
        matches: Vec::new(),
        dimmed: false,
        linked: false,
    }
}

/// One case: the published section a pane reads, plus the recording it must draw.
///
/// `focus` and `preview` are here because the *kernel* is what resolves which row a
/// cursor is drawn on (the pane focused, or a global-search preview moving it) — the
/// rule `App::build_tasks_snapshot` applies, mirrored here so a fixture cannot
/// publish a state the kernel never would.
struct Case {
    name: &'static str,
    rows: Vec<Row>,
    selected: usize,
    focus: FocusLevel,
    preview: bool,
}

impl Case {
    /// Whether the cursor is drawn at all — the kernel's rule, not the pane's: a row
    /// is marked as the cursor's while the pane is focused, or while a global-search
    /// preview is moving the cursor there.
    fn cursor_visible(&self) -> bool {
        self.is_focused() || self.preview
    }

    /// The cursor's row, clamped the way the publisher clamps it.
    fn cursor(&self) -> Option<usize> {
        (!self.rows.is_empty()).then(|| self.selected.min(self.rows.len() - 1))
    }

    fn is_focused(&self) -> bool {
        matches!(self.focus, FocusLevel::Focused)
    }

    /// The same rows as the snapshot a plugin reads.
    ///
    /// Derived from the *resolved* rows, mirroring `App::build_tasks_snapshot`:
    /// the kernel is what knows which row the cursor is visibly on, so publishing
    /// that per row is what stops the plugin having to reconstruct a rule it
    /// cannot see the inputs to.
    fn context(&self) -> PaneContext {
        let visible = self.cursor_visible();
        PaneContext {
            tasks: TasksSnapshot {
                entries: self
                    .rows
                    .iter()
                    .enumerate()
                    .map(|(i, r)| TaskSnapshot {
                        // Not drawn; fixed so the recordings stay comparable.
                        id: 0,
                        title: r.title.to_string(),
                        status: r.status.as_str(),
                        selected: visible && i == self.selected,
                        dimmed: r.dimmed,
                        linked: r.linked,
                        match_positions: r.matches.clone(),
                    })
                    .collect(),
                cursor: self.cursor(),
                focused: self.is_focused(),
            },
            ..PaneContext::default()
        }
    }

    /// The recording's name: the case name, slugified.
    ///
    /// Derived from the name rather than written twice, so a renamed case cannot
    /// keep asserting against another case's recording.
    fn snapshot_name(&self) -> String {
        // The comma goes before the spaces do, so `"no tasks, focused"` records as
        // one hyphen between words rather than two.
        self.name.replace(", ", " ").replace(' ', "-")
    }
}

/// The variants. Each isolates one decision the pane makes, so a failure names
/// which part of a row the plugin gets wrong rather than only that it does.
fn cases() -> Vec<Case> {
    let case = |name, rows, selected, focus, preview| Case {
        name,
        rows,
        selected,
        focus,
        preview,
    };
    vec![
        // The empty pane, in both the states whose text differs.
        case(
            "no tasks, unfocused",
            Vec::new(),
            0,
            FocusLevel::Inactive,
            false,
        ),
        case(
            "no tasks, focused",
            Vec::new(),
            0,
            FocusLevel::Focused,
            false,
        ),
        // One row per status: three glyphs, three colour tokens.
        case(
            "one row per status",
            vec![
                row("write the port", TaskStatus::Todo),
                row("review the diff", TaskStatus::InProgress),
                row("ship it", TaskStatus::Done),
            ],
            0,
            FocusLevel::Inactive,
            false,
        ),
        // The selected row, which must beat its status colour.
        case(
            "a selected row among others",
            vec![
                row("first", TaskStatus::Todo),
                row("second", TaskStatus::Done),
                row("third", TaskStatus::InProgress),
            ],
            1,
            FocusLevel::Focused,
            false,
        ),
        // The pane is focused but the cursor is past the end of a shortened list:
        // the kernel clamps the window and marks no row, and the plugin must not
        // invent one.
        case(
            "a cursor past the last row",
            vec![row("only", TaskStatus::Todo)],
            4,
            FocusLevel::Focused,
            false,
        ),
        // A global-search preview marks the row while focus is in the search box,
        // which is the one case where an unfocused pane still shows a cursor.
        case(
            "previewed by a global search",
            vec![
                row("host tests", TaskStatus::Todo),
                row("other", TaskStatus::Todo),
            ],
            0,
            FocusLevel::Inactive,
            true,
        ),
        // A running search: a matched row, a dimmed one, and a row that is both
        // matched and selected — the case where emphasis layers over a base that
        // already has its own.
        Case {
            name: "a running search",
            rows: vec![
                Row {
                    matches: vec![0, 5],
                    ..row("host tests", TaskStatus::Todo)
                },
                Row {
                    dimmed: true,
                    ..row("nothing matched here", TaskStatus::Done)
                },
                Row {
                    matches: vec![0, 3, 9],
                    ..row("selected and matched", TaskStatus::InProgress)
                },
            ],
            selected: 2,
            focus: FocusLevel::Focused,
            preview: false,
        },
        // The linked marker, including on a dimmed row where it takes the dim
        // tone instead of the accent.
        Case {
            name: "linked rows",
            rows: vec![
                Row {
                    linked: true,
                    ..row("has a session", TaskStatus::InProgress)
                },
                Row {
                    linked: true,
                    dimmed: true,
                    ..row("linked but filtered", TaskStatus::Todo)
                },
                Row {
                    linked: true,
                    matches: vec![0],
                    ..row("linked and matched", TaskStatus::Done)
                },
            ],
            selected: 0,
            focus: FocusLevel::Inactive,
            preview: false,
        },
        // Multi-byte titles with matches inside them: the byte-offset walk is the
        // part of this plugin most likely to be subtly wrong, and a title with an
        // em dash and accented characters is what finds it.
        Case {
            name: "a multi-byte title",
            rows: vec![
                Row {
                    matches: vec![0, 7],
                    ..row("héllo — wörld", TaskStatus::Todo)
                },
                Row {
                    matches: vec![0],
                    ..row("日本語のタスク", TaskStatus::InProgress)
                },
            ],
            selected: 0,
            focus: FocusLevel::Focused,
            preview: false,
        },
        // An offset landing *inside* a multi-byte character, which the host's
        // segmentation skips rather than slicing there. It arises from a title
        // that was shortened after the offsets were computed, and getting it
        // wrong in Luau would either panic-equivalently or split a glyph.
        // `"abc… def"`: the ellipsis occupies bytes 3..6, so 4 is inside it.
        Case {
            name: "an offset inside a character",
            rows: vec![Row {
                matches: vec![0, 4, 6],
                ..row("abc… def", TaskStatus::Todo)
            }],
            selected: 0,
            focus: FocusLevel::Inactive,
            preview: false,
        },
        // A title that is only a marker's worth of text, and an empty one: the
        // segmentation's degenerate inputs.
        Case {
            name: "degenerate titles",
            rows: vec![
                row("", TaskStatus::Todo),
                Row {
                    linked: true,
                    ..row("x", TaskStatus::Done)
                },
            ],
            selected: 0,
            focus: FocusLevel::Focused,
            preview: false,
        },
    ]
}

/// Start the bundled plugin from the source that ships.
fn host() -> PluginHost {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/plugin/bundled/tasks");
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
    host.render_pane("tasks", "tasks")
        .expect("the pane renders")
}

/// Paint a tree into a bare buffer of the given size.
///
/// The renderer both panes use, so a frame comparison is a statement about the
/// trees rather than about two painters.
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

/// The headline claim: for every case, the plugin draws the recorded pane.
///
/// The recordings are the native pane's trees, generated from
/// `ui::tasks_panel::tasks_tree` while it existed and asserted against it in the
/// same test (ADR-42). They are the only thing left holding this plugin to the pane,
/// which is why they must never be regenerated from it: a `cargo insta accept` here
/// would convert twelve statements about the pane into twelve statements about
/// whatever the plugin currently does, and the test could no longer fail for the
/// reason it exists.
#[test]
fn the_plugin_builds_the_native_panes_view_tree() {
    let _guard = SERIALIZE.lock().unwrap_or_else(|e| e.into_inner());
    let host = host();

    for case in cases() {
        thurbox::session::pane_context::publish(case.context());
        let plugin = render(&host);
        insta::assert_snapshot!(case.snapshot_name(), view_tree_record::tree(&plugin));
    }
}

/// The compared tree must be a whole pane, or the assertion above could pass on a
/// nearly-empty list.
///
/// Counted from the **plugin's** tree now: the native builder that used to answer
/// this is gone, and the recording it left is what the test above compares against.
/// The numbers are unchanged, which is the point — they were true of the pane.
#[test]
fn the_compared_tree_is_a_whole_pane() {
    let _guard = SERIALIZE.lock().unwrap_or_else(|e| e.into_inner());
    let host = host();
    let case = cases()
        .into_iter()
        .find(|c| c.name == "a running search")
        .expect("the search case");
    thurbox::session::pane_context::publish(case.context());
    let tree = render(&host);
    let (rows, selected) = match &tree {
        ViewNode::List {
            children, selected, ..
        } => (children, selected),
        other => panic!("expected a list, got {}", other.kind_name()),
    };
    assert_eq!(rows.len(), 3);
    assert_eq!(*selected, Some(2), "the list carries its cursor");
    // Every row is a line of several styled runs — the shape a single text node
    // could not carry, which is why a row is a `line` at all.
    let runs: usize = rows.iter().map(|r| r.children().len()).sum();
    assert!(runs >= 8, "{tree:#?}");
}

/// **The last divergence, closed: a title wider than the column.** It used to be
/// the one enumerated gap left in this port — the kernel fitted the title with an
/// ellipsis and reserved the marker's room using a width the plugin is never told,
/// so the plugin's copy drew the whole title, the renderer clipped it at the pane
/// edge, and a linked row could lose its `⇄` entirely.
///
/// The plugin *declares* that the title run yields its width (ADR-52) and the kernel
/// fits it, so this asserts the frame the recording cannot: an ellipsis at the cut
/// and the marker still on the row.
///
/// A painted frame rather than a tree, deliberately — the tree carries the whole
/// title at every width, so only a paint shows the fit was resolved. Same argument as
/// the scroll window's test below. Neither has a recording: an expectation states
/// what a *tree* is, and these two are about what the renderer did with one.
#[test]
fn the_plugin_paints_the_native_frame_when_a_title_is_too_wide() {
    let _guard = SERIALIZE.lock().unwrap_or_else(|e| e.into_inner());
    let host = host();
    let case = Case {
        name: "narrow",
        rows: vec![Row {
            linked: true,
            ..row("a task title far wider than the column", TaskStatus::Todo)
        }],
        selected: 0,
        focus: FocusLevel::Inactive,
        preview: false,
    };
    /// Narrow enough that the title cannot fit, so the fit is exercised rather
    /// than skipped.
    const NARROW: u16 = 18;

    thurbox::session::pane_context::publish(case.context());
    let plugin = render(&host);
    let painted: String = {
        let buf = paint(&plugin, NARROW, 1);
        (0..NARROW).map(|x| buf[(x, 0)].symbol()).collect()
    };
    assert!(
        painted.contains('…'),
        "the title should be cut with an ellipsis: {painted:?}"
    );
    assert!(
        painted.trim_end().ends_with('⇄'),
        "and the marker should survive, which is the half a clip lost: {painted:?}"
    );
}

/// **The gap that closed: a list longer than the pane.** The previous port left
/// this as an enumerated divergence — the kernel windowed its rows around the
/// selection while the plugin's copy drew from the first row, so a cursor below
/// the fold was invisible in the plugin's pane. The plugin now names the row its
/// cursor is on and the *renderer* resolves the window, so both panes scroll
/// through one implementation.
///
/// The claim is therefore about a painted frame at a height where the pane has to
/// scroll: the recorded tree is equal at *every* height — which is the point of the
/// window being the renderer's — so only a paint can show the window was resolved to
/// the cursor's row.
#[test]
fn the_plugin_paints_the_native_frame_when_the_pane_scrolls() {
    let _guard = SERIALIZE.lock().unwrap_or_else(|e| e.into_inner());
    let host = host();
    // Static titles because a fixture row borrows its title for the test's life,
    // and long enough that a scrolled frame is unmistakable in the assertion below.
    const TITLES: [&str; 12] = [
        "task number 00",
        "task number 01",
        "task number 02",
        "task number 03",
        "task number 04",
        "task number 05",
        "task number 06",
        "task number 07",
        "task number 08",
        "task number 09",
        "task number 10",
        "task number 11",
    ];
    // The cursor at the bottom of a list four times the pane's height: the case
    // where a copy that could not scroll shows none of what matters.
    let case = Case {
        name: "tall",
        rows: TITLES.iter().map(|t| row(t, TaskStatus::Todo)).collect(),
        selected: TITLES.len() - 1,
        focus: FocusLevel::Focused,
        preview: false,
    };
    thurbox::session::pane_context::publish(case.context());

    const WIDTH: u16 = 30;
    const HEIGHT: u16 = 3;
    let plugin = paint(&render(&host), WIDTH, HEIGHT);

    // The frame is the *scrolled* one: the window the renderer resolved from the
    // cursor the plugin declared.
    let text: String = (0..HEIGHT)
        .flat_map(|y| (0..WIDTH).map(move |x| (x, y)))
        .map(|(x, y)| plugin[(x, y)].symbol().to_string())
        .collect();
    assert!(
        text.contains("task number 11"),
        "the cursor's row is drawn: {text:?}"
    );
    assert!(
        !text.contains("task number 00"),
        "the top scrolled off: {text:?}"
    );
}

/// The plugin must hold no surface a user's plugin could not declare: its reach is
/// exactly its manifest's capability list, and it is two things.
#[test]
fn the_plugin_declares_every_power_it_uses() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/plugin/bundled/tasks");
    let outcome = discovery::discover_in(&[dir], None);
    let plugin = outcome.get("tasks").expect("discovered");
    let declared = &plugin.manifest.capabilities;
    use thurbox::session::plugin_manifest::Capability;
    for required in [Capability::Render, Capability::Tasks] {
        assert!(declared.contains(&required), "{required} must be declared");
    }
    assert_eq!(
        declared.len(),
        2,
        "a bundled pane that quietly asked for more would stop being evidence \
         about what a third party can build: {declared:?}"
    );
    // Handed over (ADR-53), and still hidden by default: `show_tasks_panel`
    // initialised to `false` and F5 showed the panel, so the pane that replaced it
    // seeds `false` too — a handover changes which code draws a pane, not whether it
    // is on screen. `tests/bundled_manifests.rs` is where that rule lives; this
    // asserts the manifest the oracle above is about.
    assert!(
        !plugin.manifest.panes[0].default_visible,
        "the pane must not appear on anyone's screen unasked"
    );
    assert_eq!(
        plugin.manifest.panes[0].key_context,
        Some(thurbox::session::KeyContext::Tasks),
        "the pane answers thurbox's tasks keyboard, which is what makes its keys survive \
         the handover"
    );
}

/// Before the host has published anything the reader answers "no tasks", so the
/// plugin's first render must produce the empty-state pane rather than an error.
///
/// Compared against the recording of the equivalent *case* rather than against a
/// call into the deleted builder: an empty publication and an empty task list are the
/// same pane, so the unfocused empty-state recording is exactly the expectation.
#[test]
fn the_first_render_before_any_publication_succeeds() {
    let _guard = SERIALIZE.lock().unwrap_or_else(|e| e.into_inner());
    let host = host();
    thurbox::session::pane_context::publish(PaneContext::default());
    let unpublished = view_tree_record::tree(&render(&host));

    let empty = cases()
        .into_iter()
        .find(|c| c.name == "no tasks, unfocused")
        .expect("the unfocused empty case");
    thurbox::session::pane_context::publish(empty.context());
    assert_eq!(
        unpublished,
        view_tree_record::tree(&render(&host)),
        "an empty publication draws the same pane an empty task list does"
    );
}
