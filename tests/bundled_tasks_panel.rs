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
//! each title to the column and windows the list around the selection, and a
//! plugin has neither a width nor a height. So the comparison is run at a size
//! where geometry adjusts nothing — asserted, not assumed, by
//! [`the_comparison_size_adjusts_nothing`] — and the two cases where geometry
//! *does* bite are pinned as enumerated divergences rather than absorbed by
//! weakening the equality.
//!
//! It lives in `tests/` for the same reason as the info panel's: this is the one
//! place that must see both `ui::tasks_panel` and `plugin::PluginHost`, and an
//! integration test is not part of the library's module graph, so
//! `tests/architecture_rules.rs` stays untouched.

#![cfg(feature = "plugins")]

use std::path::PathBuf;

use thurbox::plugin::{discovery, ExecutionBounds, PluginHost};
use thurbox::session::pane_context::{PaneContext, TaskSnapshot, TasksSnapshot};
use thurbox::session::view_tree::ViewNode;
use thurbox::session::TaskStatus;
use thurbox::ui::tasks_panel::{tasks_tree, visible_rows, TaskPaneEntry, TaskPaneState, TaskRow};
use thurbox::ui::FocusLevel;

/// A column and a height with room to spare, so `visible_rows` fits no title and
/// windows no row. Both are checked rather than trusted.
const WIDE: u16 = 60;
const TALL: u16 = 40;

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

/// One comparison case.
///
/// Both sides are derived from `rows` by [`Case::native_rows`], so an equality
/// failure is a statement about the plugin rather than about two hand-written
/// fixtures drifting apart.
struct Case {
    name: &'static str,
    rows: Vec<Row>,
    selected: usize,
    focus: FocusLevel,
    preview: bool,
}

impl Case {
    fn entries(&self) -> Vec<TaskPaneEntry> {
        self.rows
            .iter()
            .map(|r| TaskPaneEntry {
                title: r.title.to_string(),
                status: r.status,
                match_positions: r.matches.clone(),
                dimmed: r.dimmed,
                linked: r.linked,
            })
            .collect()
    }

    /// The rows the native pane resolves at `width` × `height`.
    fn native_rows(&self, width: u16, height: u16) -> Vec<TaskRow> {
        let entries = self.entries();
        visible_rows(
            &TaskPaneState {
                entries: &entries,
                selected: self.selected,
                focus: self.focus,
                preview_selected: self.preview,
            },
            width,
            height,
        )
        .rows
    }

    fn native_tree(&self, width: u16, height: u16) -> ViewNode {
        tasks_tree(&self.native_rows(width, height), self.is_focused())
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
    fn context(&self, width: u16, height: u16) -> PaneContext {
        PaneContext {
            tasks: TasksSnapshot {
                entries: self
                    .native_rows(width, height)
                    .into_iter()
                    .map(|r| TaskSnapshot {
                        title: r.title,
                        status: r.status.as_str(),
                        selected: r.selected,
                        dimmed: r.dimmed,
                        linked: r.linked,
                        match_positions: r.match_positions,
                    })
                    .collect(),
                focused: self.is_focused(),
            },
            ..PaneContext::default()
        }
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

/// The headline claim: for every case, the plugin's tree equals the native
/// pane's. Equal trees paint identically, so this is byte-identity of the pane.
#[test]
fn the_plugin_builds_the_native_panes_view_tree() {
    let _guard = SERIALIZE.lock().unwrap_or_else(|e| e.into_inner());
    let host = host();

    for case in cases() {
        thurbox::session::pane_context::publish(case.context(WIDE, TALL));
        let plugin = render(&host);
        let native = case.native_tree(WIDE, TALL);
        assert_eq!(
            plugin, native,
            "the plugin's tree diverges from the native pane for `{}`",
            case.name
        );
    }
}

/// The equality above is only meaningful at a size where the kernel's geometry
/// step is a no-op, so that is asserted rather than assumed: no title fitted, no
/// row windowed away.
#[test]
fn the_comparison_size_adjusts_nothing() {
    for case in cases() {
        let rows = case.native_rows(WIDE, TALL);
        assert_eq!(
            rows.len(),
            case.rows.len(),
            "`{}` lost rows to windowing at the comparison size",
            case.name
        );
        for (resolved, original) in rows.iter().zip(&case.rows) {
            assert_eq!(
                resolved.title, original.title,
                "`{}` had a title fitted at the comparison size",
                case.name
            );
        }
    }
}

/// The compared tree must be a whole pane, or the equality could pass on two
/// nearly-empty lists.
#[test]
fn the_compared_tree_is_a_whole_pane() {
    let case = cases()
        .into_iter()
        .find(|c| c.name == "a running search")
        .expect("the search case");
    let tree = case.native_tree(WIDE, TALL);
    let rows = match &tree {
        ViewNode::List(rows) => rows,
        other => panic!("expected a list, got {}", other.kind_name()),
    };
    assert_eq!(rows.len(), 3);
    // Every row is a line of several styled runs — the shape a single text node
    // could not carry, which is why a row is a `line` at all.
    let runs: usize = rows.iter().map(|r| r.children().len()).sum();
    assert!(runs >= 8, "{tree:#?}");
}

/// **Enumerated divergence 1: a title wider than the column.** The kernel fits it
/// with an ellipsis and reserves room for the trailing marker, using a width the
/// plugin is never told. The plugin's copy draws the whole title and the renderer
/// clips it at the pane edge — so a long title loses its ellipsis and a linked row
/// can lose its marker.
///
/// Closing it needs a `line` that clips with an ellipsis and a flush-right run.
/// The mechanism exists inside the renderer already (a gauge right-aligns its
/// suffix); what is missing is a node that asks for it.
#[test]
fn a_title_wider_than_the_column_is_fitted_by_the_kernel_only() {
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
    const NARROW: u16 = 18;

    // The plugin reads what the kernel publishes, which is not fitted to a width.
    thurbox::session::pane_context::publish(case.context(WIDE, TALL));
    let plugin = render(&host);
    let native = case.native_tree(NARROW, TALL);
    assert_ne!(
        plugin, native,
        "if these ever agree, the pane stopped fitting titles and this \
         divergence should be retired"
    );

    let text_of = |tree: &ViewNode| -> String {
        fn walk(node: &ViewNode, out: &mut String) {
            if let ViewNode::Text { content, .. } = node {
                out.push_str(content);
            }
            node.children().iter().for_each(|c| walk(c, out));
        }
        let mut out = String::new();
        walk(tree, &mut out);
        out
    };
    assert!(
        text_of(&native).contains('…'),
        "the native pane fits the title: {:?}",
        text_of(&native)
    );
    assert!(
        !text_of(&plugin).contains('…'),
        "the plugin draws the whole title and lets the renderer clip: {:?}",
        text_of(&plugin)
    );
    // And the marker is still *in* the plugin's tree — it is the renderer that
    // will clip it, not the plugin that drops it.
    assert!(text_of(&plugin).contains('⇄'));
}

/// **Enumerated divergence 2: more rows than the pane has lines.** The kernel
/// windows them around the selection; the plugin's copy draws every published row
/// from the first and the renderer clips the overflow, so a selection below the
/// fold is not visible in the plugin's pane.
///
/// Closing it needs a list node carrying a selected index, windowed by the kernel
/// from the height it has — the same shape the gauge node took for width. It is
/// the gap the session-list port will have to close, since a session list that
/// cannot scroll to its selection is not one.
#[test]
fn a_list_longer_than_the_pane_is_windowed_by_the_kernel_only() {
    let _guard = SERIALIZE.lock().unwrap_or_else(|e| e.into_inner());
    let host = host();
    let titles: Vec<&'static str> = vec!["t0", "t1", "t2", "t3", "t4", "t5", "t6", "t7"];
    let case = Case {
        name: "tall",
        rows: titles.iter().map(|t| row(t, TaskStatus::Todo)).collect(),
        selected: 7,
        focus: FocusLevel::Focused,
        preview: false,
    };
    const SHORT: u16 = 3;

    thurbox::session::pane_context::publish(case.context(WIDE, TALL));
    let plugin_rows = match render(&host) {
        ViewNode::List(rows) => rows.len(),
        other => panic!("expected a list, got {}", other.kind_name()),
    };
    let native = case.native_rows(WIDE, SHORT);

    assert_eq!(native.len(), SHORT as usize, "the kernel windows");
    assert_eq!(plugin_rows, titles.len(), "the plugin draws them all");
    assert!(
        native.last().expect("a windowed row").selected,
        "the kernel's window keeps the cursor in view"
    );
    // The plugin's pane would show rows 0..2 and not the selected row 7 — which
    // is exactly the cost this divergence names.
    assert!(
        !case
            .context(WIDE, TALL)
            .tasks
            .entries
            .iter()
            .take(SHORT as usize)
            .any(|e| e.selected),
        "the selected row is below the fold of the plugin's copy"
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
    // Additive port: the pane must not appear on anyone's screen unasked, since
    // the native pane is still the one thurbox draws.
    assert!(
        !plugin.manifest.panes[0].default_visible,
        "the reproduction must be hidden by default"
    );
}

/// Before the host has published anything the reader answers "no tasks", so the
/// plugin's first render must produce the empty-state pane rather than an error.
#[test]
fn the_first_render_before_any_publication_succeeds() {
    let _guard = SERIALIZE.lock().unwrap_or_else(|e| e.into_inner());
    let host = host();
    thurbox::session::pane_context::publish(PaneContext::default());
    assert_eq!(
        render(&host),
        tasks_tree(&[], false),
        "an empty publication draws the same pane an empty task list does"
    );
}
