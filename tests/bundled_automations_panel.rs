//! The bundled automations plugin renders — and **drives** — the native pane.
//!
//! The claim `tests/bundled_tasks_panel.rs` makes for the tasks pane, for the
//! sixth ported pane and the last native pane that had no plugin at all: the Luau
//! plugin's view tree must **equal** the one
//! `ui::automations_panel::automations_tree` builds from the same rows, and paint
//! the same frame at a height where the pane scrolls.
//!
//! What is new here, and is why this file is longer than its four predecessors:
//! this is the first bundled plugin that takes **input** and **writes**. So the
//! claim is not only "it draws the same pane" but "its keys do what the native
//! keys do", asserted through the database the native keys write rather than by
//! reading the plugin's own state. Three properties are pinned that no previous
//! port could state:
//!
//! - the cursor a key moves is the **plugin's own**, and thurbox's is untouched
//!   ([`the_keys_move_the_plugins_own_cursor`]);
//! - `runAutomation` only marks an automation **due** — the kernel fires it, so no
//!   plugin thread runs the shell command a user's `Exec` automation names
//!   ([`running_marks_the_automation_due_and_executes_nothing`]);
//! - the left column's circular wrap stays **kernel-owned**: the plugin declines
//!   the key at its edge and nothing completes it
//!   ([`the_wrap_out_of_the_pane_stays_kernel_owned`]).
//!
//! **No divergence is enumerated any more**, and both of the two this file used to
//! carry are kept pointing the other way rather than deleted. The **placement** went
//! first (ADR-46): the reproduction declares the seat its native counterpart occupies,
//! so showing it compares the two panes in one rect
//! ([`the_pane_is_placed_where_the_native_one_sits`]). The **fitted name** went with
//! ADR-55: neither side cuts a name any more — both declare that the name's runs yield
//! their width and the kernel cuts them — so the assertion is now frame equality at a
//! width where the fit fires, which is the pane's real size
//! ([`the_two_panes_paint_the_same_frame_when_a_name_overflows`]).
//!
//! It lives in `tests/` for its predecessors' reason: this is the one place that
//! must see both `ui::automations_panel` and `plugin::PluginHost`, and an
//! integration test is not part of the library's module graph, so
//! `tests/architecture_rules.rs` stays untouched.
//!
//! ## Two edges, because a handover deletes one of them
//!
//! The tree equality is **differential** — it names `automations_tree`, which
//! lives in `src/ui/automations_panel.rs`, the module a handover removes. On that
//! day the comparison has nothing on its right-hand side, and the repair that
//! compiles is to drop it: what survives is a test that the plugin renders without
//! erroring, satisfied equally by a pane drawing one wrong row and by one drawing
//! twenty. So each case asserts **both** edges while both sides exist (ADR-42):
//! the recording in `tests/snapshots/` equals the *native* tree — the edge that
//! gives it provenance, establishable only now — and the plugin equals the native
//! tree, the port's original proof. Their conjunction is what a handover inherits.
//!
//! The keys are not recorded this way and cannot be: their oracle is the database
//! a native key writes, which no handover deletes.

#![cfg(feature = "plugins")]

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

use thurbox::plugin::{discovery, ExecutionBounds, PluginHost};
use thurbox::session::automation::{AutomationAction, AutomationSchedule};
use thurbox::session::motion::FrameTable;
use thurbox::session::pane_context::{AutomationRowSnapshot, AutomationsSnapshot, PaneContext};
use thurbox::session::plugin_manifest::Capability;
use thurbox::session::plugin_mutations::KernelWriter;
use thurbox::session::view_tree::ViewNode;
use thurbox::storage::automations::NewAutomation;
use thurbox::storage::plugins::DbKernelWriter;
use thurbox::storage::Database;
use thurbox::ui::automations_panel::{
    automations_tree, resolve_rows, row_summary, AutomationPaneEntry, AutomationRow,
    AutomationsPaneState,
};
use thurbox::ui::FocusLevel;

/// The recorder that turns a view tree into the checked-in expectation, shared
/// with the other panes that record one (see its module note for why this is
/// shared while the input gates' source-reading helpers are copied).
mod view_tree_record;

/// The bound the UI thread waits on plugin input, so a wedged plugin costs a
/// dropped key rather than a hang. Generous here: a test machine under load is not
/// a wedged plugin.
const KEY_TIMEOUT: Duration = Duration::from_secs(5);

/// The process-wide snapshot slot is global, so every case here runs one at a
/// time — otherwise one case's publication would answer another's reader.
static SERIALIZE: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// One row of a fixture, in the form the *model* has it.
struct Row {
    name: &'static str,
    schedule: &'static str,
    action: &'static str,
    enabled: bool,
    due: Option<u64>,
    matches: Vec<usize>,
    dimmed: bool,
}

fn row(name: &'static str) -> Row {
    Row {
        name,
        schedule: "daily 09:00",
        action: "spawn",
        enabled: true,
        due: Some(120),
        matches: Vec::new(),
        dimmed: false,
    }
}

/// One comparison case.
///
/// Both sides are derived from `rows`, so an equality failure is a statement about
/// the plugin rather than about two hand-written fixtures drifting apart.
struct Case {
    name: &'static str,
    rows: Vec<Row>,
    selected: usize,
    focus: FocusLevel,
    preview: bool,
}

impl Case {
    fn entries(&self) -> Vec<AutomationPaneEntry> {
        self.rows
            .iter()
            .enumerate()
            .map(|(i, r)| AutomationPaneEntry {
                id: i as i64 + 1,
                name: r.name.to_string(),
                schedule: r.schedule.to_string(),
                action: r.action,
                enabled: r.enabled,
                due_in_secs: r.due,
                match_positions: r.matches.clone(),
                dimmed: r.dimmed,
            })
            .collect()
    }

    /// The rows the native pane resolves. Every row, in the model's order: the
    /// window is the renderer's, resolved from the cursor below — and since ADR-55
    /// so is the fit, so this takes no width at all.
    fn native_rows(&self) -> Vec<AutomationRow> {
        let entries = self.entries();
        resolve_rows(&AutomationsPaneState {
            entries: &entries,
            selected: self.selected,
            focus: self.focus,
            preview_selected: self.preview,
        })
    }

    /// The cursor's row, clamped — the one rule `cursor_row` and
    /// `App::build_automations_snapshot` share, since the host refuses a list whose
    /// cursor is not an index into its children.
    fn cursor(&self) -> Option<usize> {
        (!self.rows.is_empty()).then(|| self.selected.min(self.rows.len() - 1))
    }

    fn native_tree(&self) -> ViewNode {
        automations_tree(&self.native_rows(), self.cursor(), self.is_focused())
    }

    /// The recording's name: the case name, slugified.
    ///
    /// Derived from the name rather than written twice, so a renamed case cannot
    /// keep asserting against another case's recording.
    fn snapshot_name(&self) -> String {
        self.name.replace([' ', ',', '\''], "-")
    }

    fn is_focused(&self) -> bool {
        matches!(self.focus, FocusLevel::Focused)
    }

    /// Whether the native pane draws its cursor, which is what the section's
    /// `cursor_visible` publishes — mirroring `App::build_automations_snapshot`.
    fn cursor_visible(&self) -> bool {
        self.is_focused() || self.preview
    }

    /// The same rows as the snapshot a plugin reads.
    ///
    /// Built from the model's rows, which since ADR-55 is what the *resolved* rows
    /// carry too: the published name is unfitted because a width belongs to a frame
    /// and the plugin's pane is a different rect, and the native pane no longer fits
    /// one either.
    fn context(&self) -> PaneContext {
        PaneContext {
            automations: AutomationsSnapshot {
                entries: self
                    .rows
                    .iter()
                    .enumerate()
                    .map(|(i, r)| AutomationRowSnapshot {
                        id: i as i64 + 1,
                        name: r.name.to_string(),
                        enabled: r.enabled,
                        action: r.action,
                        schedule: r.schedule.to_string(),
                        due_in_secs: r.due,
                        dimmed: r.dimmed,
                        match_positions: r.matches.clone(),
                    })
                    .collect(),
                cursor: self.cursor(),
                cursor_visible: self.cursor_visible(),
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
            "none, unfocused",
            Vec::new(),
            0,
            FocusLevel::Inactive,
            false,
        ),
        case("none, focused", Vec::new(), 0, FocusLevel::Focused, false),
        // Enabled and disabled: two markers, two colour roles, and the `when`
        // precedence that puts `disabled` ahead of a countdown.
        Case {
            name: "enabled and disabled rows",
            rows: vec![
                row("nightly sync"),
                Row {
                    enabled: false,
                    ..row("paused one")
                },
                Row {
                    // Disabled *and* still carrying a next run: the precedence
                    // decides, and a plugin reading `dueInSecs` first would differ
                    // here.
                    enabled: false,
                    due: Some(30),
                    ..row("disabled but scheduled")
                },
            ],
            selected: 0,
            focus: FocusLevel::Inactive,
            preview: false,
        },
        // Every schedule shape and every action name, so the summary's parts are
        // exercised rather than one representative.
        Case {
            name: "each schedule and action shape",
            rows: vec![
                Row {
                    schedule: "once",
                    action: "send",
                    ..row("one shot")
                },
                Row {
                    schedule: "hourly :15",
                    action: "exec",
                    ..row("hourly job")
                },
                Row {
                    schedule: "weekdays 08:30",
                    action: "spawn",
                    ..row("standup")
                },
                Row {
                    schedule: "Mondays 07:00",
                    action: "send",
                    ..row("weekly nudge")
                },
                Row {
                    // A cron that maps to no preset: the raw expression crosses.
                    schedule: "*/7 3 * * *",
                    action: "exec",
                    ..row("power user")
                },
            ],
            selected: 2,
            focus: FocusLevel::Focused,
            preview: false,
        },
        // The three `when` shapes at once.
        Case {
            name: "each countdown shape",
            rows: vec![
                Row {
                    due: Some(0),
                    ..row("due now")
                },
                Row {
                    due: Some(45),
                    ..row("seconds")
                },
                Row {
                    due: Some(150),
                    ..row("minutes and seconds")
                },
                Row {
                    due: Some(300),
                    ..row("whole minutes")
                },
                Row {
                    due: Some(7_200),
                    ..row("whole hours")
                },
                Row {
                    due: Some(12_000),
                    ..row("hours and minutes")
                },
                Row {
                    due: None,
                    ..row("no next run")
                },
            ],
            selected: 0,
            focus: FocusLevel::Focused,
            preview: false,
        },
        // The cursor is past the end of a shortened list: the kernel clamps the
        // window and the plugin must clamp with it.
        case(
            "a cursor past the last row",
            vec![row("only")],
            4,
            FocusLevel::Focused,
            false,
        ),
        // An unfocused pane with the cursor below the fold: the anchor is published
        // and the appearance is not, which is this pane's whole reason for two
        // fields.
        case(
            "an anchor without a drawn cursor",
            (0..6).map(|_| row("row")).collect(),
            5,
            FocusLevel::Inactive,
            false,
        ),
        // A global-search preview marks the row while focus is in the search box:
        // the one case where an unfocused pane still shows a cursor.
        case(
            "previewed by a global search",
            vec![row("host tests"), row("other")],
            0,
            FocusLevel::Inactive,
            true,
        ),
        // The central-pane editor holds focus: the native pane stays "active" so
        // the row being worked on keeps its mark, which is `cursor_visible` without
        // `focused`.
        case(
            "active while the editor is open",
            vec![row("being edited"), row("other")],
            0,
            FocusLevel::Active,
            true,
        ),
        // A running search: a matched row, a dimmed one, and a row that is both
        // matched and selected — where emphasis layers over a base that already has
        // its own.
        Case {
            name: "a running search",
            rows: vec![
                Row {
                    matches: vec![0, 5],
                    ..row("host tests")
                },
                Row {
                    dimmed: true,
                    ..row("nothing matched here")
                },
                Row {
                    matches: vec![0, 3, 9],
                    ..row("selected and matched")
                },
                Row {
                    // Dimmed *and* disabled: two style inputs whose precedence the
                    // plugin has to get right.
                    dimmed: true,
                    enabled: false,
                    ..row("dim and disabled")
                },
            ],
            selected: 2,
            focus: FocusLevel::Focused,
            preview: false,
        },
        // Multi-byte names with matches inside them: the byte-offset walk is the
        // part of this plugin most likely to be subtly wrong.
        Case {
            name: "a multi-byte name",
            rows: vec![
                Row {
                    matches: vec![0, 7],
                    ..row("héllo — wörld")
                },
                Row {
                    matches: vec![0],
                    ..row("日本語の自動化")
                },
            ],
            selected: 0,
            focus: FocusLevel::Focused,
            preview: false,
        },
        // An offset landing *inside* a multi-byte character, which the host's
        // segmentation skips rather than slicing there. `"abc… def"`: the ellipsis
        // occupies bytes 3..6, so 4 is inside it.
        Case {
            name: "an offset inside a character",
            rows: vec![Row {
                matches: vec![0, 4, 6],
                ..row("abc… def")
            }],
            selected: 0,
            focus: FocusLevel::Inactive,
            preview: false,
        },
        // Degenerate names: the segmentation's edge inputs.
        Case {
            name: "degenerate names",
            rows: vec![row(""), row("x")],
            selected: 0,
            focus: FocusLevel::Focused,
            preview: false,
        },
    ]
}

fn plugin_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/plugin/bundled/automations")
}

/// Start the bundled plugin from the source that ships.
fn host() -> PluginHost {
    let dir = plugin_dir();
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

/// The same host, able to write — the shape `main.rs` wires for the write seam.
fn host_with_writer(db_path: &Path) -> PluginHost {
    let path = db_path.to_path_buf();
    let factory: thurbox::session::plugin_mutations::KernelWriterFactory = Arc::new(move || {
        Database::open(&path)
            .ok()
            .map(|db| Box::new(DbKernelWriter::from_db(db)) as Box<dyn KernelWriter>)
    });
    let outcome = discovery::discover_in(&[plugin_dir()], None);
    let mut host =
        PluginHost::from_discovery(outcome, ExecutionBounds::default()).with_kernel_writer(factory);
    assert_eq!(host.start_all(), 1, "the plugin must reach Running");
    host
}

fn render(host: &PluginHost) -> ViewNode {
    host.render_pane("automations", "automations")
        .expect("the pane renders")
}

fn press(host: &PluginHost, key: &str, binding: Option<&str>) -> bool {
    host.send_key("automations", "automations", key, binding, KEY_TIMEOUT)
        .expect("the plugin is granted input")
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

/// The glyphs of a painted buffer's first row, so a frame assertion can name what it
/// expected instead of printing two buffers.
fn row_text(buf: &Buffer, width: u16) -> String {
    (0..width).map(|x| buf[(x, 0)].symbol()).collect()
}

/// Which row a tree's list names as its cursor.
fn tree_cursor(tree: &ViewNode) -> Option<usize> {
    match tree {
        ViewNode::List { selected, .. } => *selected,
        other => panic!("expected a list, got {}", other.kind_name()),
    }
}

/// Every text run in a tree, concatenated — for asserting on what a pane shows
/// without depending on how its runs are split.
fn text_of(tree: &ViewNode) -> String {
    fn walk(node: &ViewNode, out: &mut String) {
        if let ViewNode::Text { content, .. } = node {
            out.push_str(content);
        }
        node.children().iter().for_each(|c| walk(c, out));
    }
    let mut out = String::new();
    walk(tree, &mut out);
    out
}

/// The headline claim: for every case, the plugin's tree equals the native pane's.
/// Equal trees paint identically, so this is byte-identity of the pane's rows.
///
/// The recorded edge is asserted **first**, so a run in which both edges break
/// reports the recording — the fact about the pane — rather than only that two
/// implementations disagree.
#[test]
fn the_plugin_builds_the_native_panes_view_tree() {
    let _guard = SERIALIZE.lock().unwrap_or_else(|e| e.into_inner());
    let host = host();

    for case in cases() {
        thurbox::session::pane_context::publish(case.context());
        let plugin = render(&host);
        let native = case.native_tree();
        // Edge one: the checked-in expectation is the native pane's tree — the
        // edge that outlives `automations_tree`, and one only establishable while
        // that builder is here to generate it.
        insta::assert_snapshot!(case.snapshot_name(), view_tree_record::tree(&native));
        // Edge two: the plugin reproduces that recording — asserted through the
        // records, so a failure names the line that moved instead of printing two
        // structural dumps to diff by eye.
        view_tree_record::assert_matches(case.name, &plugin, &native);
        // Edge three: the port's original claim, exact rather than legible.
        assert_eq!(
            plugin, native,
            "the plugin's tree diverges from the native pane for `{}`",
            case.name
        );
    }
}

/// The equality above holds at **every** width, because neither side resolves one.
///
/// It used to be a statement about a chosen comparison size: `resolve_rows` fitted
/// the name, so the trees could only be equal where the fit was a no-op, and the
/// narrow case was an enumerated divergence. ADR-55 took the fit out of the tree, so
/// this now pins the stronger fact — no row's name is cut before the renderer sees
/// it — which is what makes
/// [`the_two_panes_paint_the_same_frame_when_a_name_overflows`] a comparison of
/// panes rather than of paddings.
#[test]
fn the_tree_carries_no_fitted_name() {
    for case in cases() {
        let rows = case.native_rows();
        assert_eq!(
            rows.len(),
            case.rows.len(),
            "`{}` lost rows before the tree, which windows nothing",
            case.name
        );
        for (resolved, original) in rows.iter().zip(&case.rows) {
            assert_eq!(
                resolved.name, original.name,
                "`{}` had a name fitted before the tree",
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
    let tree = case.native_tree();
    let (rows, selected) = match &tree {
        ViewNode::List {
            children, selected, ..
        } => (children, selected),
        other => panic!("expected a list, got {}", other.kind_name()),
    };
    assert_eq!(rows.len(), 4);
    assert_eq!(*selected, Some(2), "the list carries its cursor");
    // Every row is a line of several styled runs — a marker, one run per matched
    // and unmatched span of the name, and the summary tail.
    let runs: usize = rows.iter().map(|r| r.children().len()).sum();
    assert!(runs >= 12, "{tree:#?}");
}

/// The summary the plugin composes equals thurbox's rule, for every combination of
/// the three parts — not only the ones a fixture happens to contain.
///
/// The comparison is against `ui::automations_panel::row_summary` rather than
/// against a string written here, which is the point: the test cannot agree with
/// the plugin about a rule thurbox does not follow.
#[test]
fn the_plugin_composes_the_summary_thurbox_composes() {
    let _guard = SERIALIZE.lock().unwrap_or_else(|e| e.into_inner());
    let host = host();

    let schedules = ["once", "daily 09:00", "hourly :15", "*/7 3 * * *"];
    let actions = ["send", "spawn", "exec"];
    let whens = [
        None,
        Some(0),
        Some(1),
        Some(59),
        Some(60),
        Some(90),
        Some(3_600),
        Some(12_000),
    ];
    let mut compared = 0;

    for schedule in schedules {
        for action in actions {
            for enabled in [true, false] {
                for due in whens {
                    thurbox::session::pane_context::publish(PaneContext {
                        automations: AutomationsSnapshot {
                            entries: vec![AutomationRowSnapshot {
                                id: 1,
                                name: "n".to_string(),
                                enabled,
                                action,
                                schedule: schedule.to_string(),
                                due_in_secs: due,
                                dimmed: false,
                                match_positions: Vec::new(),
                            }],
                            cursor: Some(0),
                            cursor_visible: false,
                            focused: false,
                        },
                        ..PaneContext::default()
                    });
                    let drawn = text_of(&render(&host));
                    let expected = row_summary(schedule, action, enabled, due);
                    assert!(
                        drawn.contains(&expected),
                        "the plugin's summary is not thurbox's for \
                         ({schedule}, {action}, enabled={enabled}, due={due:?}): \
                         drew {drawn:?}, expected to contain {expected:?}"
                    );
                    compared += 1;
                }
            }
        }
    }
    assert_eq!(compared, 192, "every combination is compared");
}

/// **The gap the file viewer closed, held here.** The plugin names the row its
/// cursor is on and the *renderer* resolves the window, so both panes scroll
/// through one implementation. Tree equality alone could not say it — the two
/// trees are equal at every height — so only a paint shows the window was resolved
/// the same way.
#[test]
fn the_plugin_paints_the_native_frame_when_the_pane_scrolls() {
    let _guard = SERIALIZE.lock().unwrap_or_else(|e| e.into_inner());
    let host = host();
    const NAMES: [&str; 12] = [
        "automation 00",
        "automation 01",
        "automation 02",
        "automation 03",
        "automation 04",
        "automation 05",
        "automation 06",
        "automation 07",
        "automation 08",
        "automation 09",
        "automation 10",
        "automation 11",
    ];
    // The cursor at the bottom of a list four times the pane's height: the case
    // where a copy that could not scroll shows none of what matters.
    let case = Case {
        name: "tall",
        rows: NAMES.iter().map(|n| row(n)).collect(),
        selected: NAMES.len() - 1,
        focus: FocusLevel::Focused,
        preview: false,
    };
    thurbox::session::pane_context::publish(case.context());

    // Wide enough that no name is cut: the marker takes 3 columns, these rows'
    // summary tails take 31 and their names 13, so anything below 47 would fit the
    // name instead — and the assertions below would then be about the *fit* rather
    // than about the window, which is
    // `the_two_panes_paint_the_same_frame_when_a_name_overflows`' subject.
    const WIDTH: u16 = 60;
    const HEIGHT: u16 = 3;
    let plugin = paint(&render(&host), WIDTH, HEIGHT);
    let native = paint(&case.native_tree(), WIDTH, HEIGHT);
    assert_eq!(plugin, native, "the plugin's frame is not the native frame");

    // And that frame is the *scrolled* one — otherwise the two could agree by both
    // being wrong.
    let text: String = (0..HEIGHT)
        .flat_map(|y| (0..WIDTH).map(move |x| (x, y)))
        .map(|(x, y)| plugin[(x, y)].symbol().to_string())
        .collect();
    assert!(
        text.contains("automation 11"),
        "the cursor's row is drawn: {text:?}"
    );
    assert!(
        !text.contains("automation 00"),
        "the top scrolled off: {text:?}"
    );
}

/// **The retired divergence: a name wider than the column.** This was the pane's
/// last enumerated difference, and it is now its opposite.
///
/// The native pane used to cut the name itself, reserving the marker's and the whole
/// summary's room from a width the plugin is never told; the plugin drew the name
/// whole and the renderer clipped it at the pane's edge, taking the schedule, the
/// action and the countdown with it. Since ADR-55 **neither** side fits: both trees
/// mark the name's runs as yielding their width, and the kernel cuts them with the
/// same `ui::truncate_ellipsis`. So the claim is frame equality at a width where the
/// fit actually fires — the pane's real size, where the equality above could
/// previously say nothing.
///
/// Asserted as a paint rather than as tree equality, which the case above already
/// covers: the fit happens in the renderer, so only a painted frame shows the two
/// panes cut in the same column.
#[test]
fn the_two_panes_paint_the_same_frame_when_a_name_overflows() {
    let _guard = SERIALIZE.lock().unwrap_or_else(|e| e.into_inner());
    let host = host();
    let case = Case {
        name: "narrow",
        rows: vec![row("an automation name far wider than the column")],
        selected: 0,
        focus: FocusLevel::Inactive,
        preview: false,
    };
    // Narrow enough that the name is cut, wide enough that something is left of it:
    // the marker takes 3 columns and this row's summary tail takes 31.
    const NARROW: u16 = 44;

    thurbox::session::pane_context::publish(case.context());
    let plugin = paint(&render(&host), NARROW, 1);
    let native = paint(&case.native_tree(), NARROW, 1);
    assert_eq!(
        plugin, native,
        "the two panes must cut the name in the same column"
    );

    // And that frame is the *fitted* one — otherwise the two could agree by both
    // clipping. The marker leads, the name ends in an ellipsis, and the whole
    // summary tail survives, which is the half a clip at the pane edge lost.
    let drawn = row_text(&plugin, NARROW);
    assert!(drawn.contains('…'), "the name is ellipsized: {drawn:?}");
    assert!(
        drawn.contains("daily 09:00 · spawn · in 2m"),
        "the summary tail keeps its columns: {drawn:?}"
    );

    // At a width where the marker and the tail leave the name no columns at all both
    // panes drop it — `truncate_ellipsis` returns nothing rather than a lone `…` that
    // carries no information — and they still agree, which is the edge the enumerated
    // divergence used to be sharpest at.
    const CRAMPED: u16 = 30;
    let plugin = paint(&render(&host), CRAMPED, 1);
    let native = paint(&case.native_tree(), CRAMPED, 1);
    assert_eq!(
        plugin, native,
        "they agree where the name does not fit at all"
    );
    let drawn = row_text(&plugin, CRAMPED);
    assert!(
        !drawn.contains("an automation"),
        "a name with no columns is dropped: {drawn:?}"
    );
}

/// **The retired divergence: the pane's placement.** The reproduction used to be
/// placeable *as a pane* and not placeable *where this pane is* — `PaneSlot` named
/// only the right-hand column, while the native pane is the band beneath the
/// session list. ADR-46 gave that band a slot, and this pane declares it.
///
/// Kept rather than deleted, pointing the other way: it now pins that the shipped
/// manifest names the seat its native counterpart occupies, so the two panes are
/// compared in the same rect. A regression to the right column would make every
/// equality claim above one about content alone again.
#[test]
fn the_pane_is_placed_where_the_native_one_sits() {
    use thurbox::session::plugin_manifest::{PaneSlot, PluginManifest};

    let outcome = discovery::discover_in(&[plugin_dir()], None);
    let plugin = outcome.get("automations").expect("discovered");
    assert_eq!(
        plugin.manifest.panes[0].slot,
        PaneSlot::LeftBottom,
        "the pane should name the seat the native automations pane occupies"
    );
    // And that seat is the native pane's region rather than a lookalike: the slot
    // resolves to the region `ui::layout` places beneath the session list.
    assert_eq!(
        PaneSlot::LeftBottom.seat(),
        Some(thurbox::session::workspace_tree::RegionId::Automations)
    );
    // The vocabulary is still closed, so a typo in that seat's name is an error
    // rather than a silent default into the right column.
    let manifest = |slot: &str| {
        format!(
            "name = \"copy\"\napi_version = 1\ncapabilities = [\"render\"]\n\
             [[panes]]\nid = \"a\"\nslot = \"{slot}\"\n"
        )
    };
    assert!(
        PluginManifest::from_toml(Path::new("/p/copy/plugin.toml"), &manifest("left-bottom"))
            .is_ok(),
        "the seat this pane names must parse, or this test proves nothing"
    );
    assert!(
        PluginManifest::from_toml(Path::new("/p/copy/plugin.toml"), &manifest("bottom-left"))
            .is_err(),
        "an unrecognized slot must be refused rather than defaulted"
    );
}

/// The plugin must hold no surface a user's plugin could not declare: its reach is
/// exactly its manifest's capability list, and it is four things — two more than
/// any previous bundled pane, because this is the first that takes input and
/// writes.
#[test]
fn the_plugin_declares_every_power_it_uses() {
    let outcome = discovery::discover_in(&[plugin_dir()], None);
    let plugin = outcome.get("automations").expect("discovered");
    let declared = &plugin.manifest.capabilities;
    for required in [
        Capability::Render,
        Capability::Automations,
        Capability::Input,
        Capability::AutomationsWrite,
    ] {
        assert!(declared.contains(&required), "{required} must be declared");
    }
    assert_eq!(
        declared.len(),
        4,
        "a bundled pane that quietly asked for more would stop being evidence \
         about what a third party can build: {declared:?}"
    );
    // Notably absent: `tasks-write`, `sessions`, `spawn`, `state-*`. The pane
    // changes automations and reads automations; nothing else.
    for absent in [
        Capability::TasksWrite,
        Capability::Sessions,
        Capability::Spawn,
        Capability::StateWrite,
    ] {
        assert!(!declared.contains(&absent), "{absent} must not be declared");
    }
    // Additive port: the pane must not appear on anyone's screen unasked, since
    // the native pane is still the one thurbox draws.
    assert!(
        !plugin.manifest.panes[0].default_visible,
        "the reproduction must be hidden by default"
    );
    // One binding per ported key, each with a chord — a binding shipped without
    // one would be a key the pane advertises and nobody can press.
    let bindings = &plugin.manifest.keybindings;
    let ids: Vec<&str> = bindings.iter().map(|b| b.id.as_str()).collect();
    assert_eq!(ids, ["next", "prev", "toggle", "run", "delete"]);
    for b in bindings {
        assert_eq!(b.pane, "automations");
        assert!(b.chord.is_some(), "`{}` declares no chord", b.id);
    }
    // And the two keys that cannot be ported are *not* declared, because the host
    // refuses to register a binding it cannot deliver and a plugin shipping a key
    // that does nothing is the same failure one layer up.
    for unportable in ["new", "edit", "open"] {
        assert!(
            !ids.contains(&unportable),
            "`{unportable}` needs a host power the vocabulary does not define"
        );
    }
}

/// The keys move the **plugin's own** cursor, and thurbox's is untouched.
///
/// This is the property ADR-38 could not state, and the reason this pane's keys are
/// portable where the tasks pane's were not: the row the user is looking at is the
/// row the *pane receiving the key* draws its cursor on, and a VM persists across
/// renders, so that cursor is ordinary plugin state needing no view write.
#[test]
fn the_keys_move_the_plugins_own_cursor() {
    let _guard = SERIALIZE.lock().unwrap_or_else(|e| e.into_inner());
    let host = host();
    let case = Case {
        name: "driven",
        rows: (0..5).map(|_| row("an automation")).collect(),
        selected: 1,
        // Unfocused, which is the real situation: while the *plugin's* pane holds
        // focus, thurbox's does not — so the published cursor is not drawn, and a
        // plugin acting on it would act on a row nobody is looking at.
        focus: FocusLevel::Inactive,
        preview: false,
    };
    let published = case.context();
    thurbox::session::pane_context::publish(published.clone());

    // Before any key: the plugin follows the host, which is what keeps the
    // reproduction exact.
    assert_eq!(tree_cursor(&render(&host)), Some(1));

    assert!(press(&host, "j", Some("next")), "the key is consumed");
    assert_eq!(
        tree_cursor(&render(&host)),
        Some(2),
        "the plugin's own cursor moved"
    );
    // The publication is untouched: the plugin wrote no kernel state.
    assert_eq!(
        thurbox::session::pane_context::published()
            .unwrap()
            .automations,
        published.automations,
        "a plugin's cursor is its own; thurbox's did not move"
    );

    assert!(press(&host, "k", Some("prev")));
    assert_eq!(tree_cursor(&render(&host)), Some(1));

    // The arrows work too, by raw key name, because a manifest binding declares one
    // chord where thurbox's action declares a list.
    assert!(press(&host, "down", None));
    assert!(press(&host, "down", None));
    assert_eq!(tree_cursor(&render(&host)), Some(3));
    assert!(press(&host, "up", None));
    assert_eq!(tree_cursor(&render(&host)), Some(2));

    // A click selects the row it landed on, in the same numbering `ui.list` uses.
    assert!(host
        .send_click("automations", "automations", 5, KEY_TIMEOUT)
        .expect("granted input"));
    // Row 5 in the plugin's one-based numbering is index 4 in the converted tree.
    assert_eq!(tree_cursor(&render(&host)), Some(4));
    // A click past the last row is declined rather than inventing a cursor.
    assert!(!host
        .send_click("automations", "automations", 9, KEY_TIMEOUT)
        .expect("granted input"));

    // Once driven, the cursor is *drawn* even though the host says its own pane's
    // is not — the enumerated consequence of a pane not being told about its own
    // focus. Asserted so that publishing that fact fails here.
    let tree = render(&host);
    assert!(
        !published.automations.cursor_visible,
        "the fixture's native pane draws no cursor"
    );
    let selected_runs = match &tree {
        ViewNode::List { children, .. } => children[4]
            .children()
            .iter()
            .filter(|r| matches!(r, ViewNode::Text { style, .. } if style.bold))
            .count(),
        other => panic!("expected a list, got {}", other.kind_name()),
    };
    assert!(
        selected_runs > 0,
        "a driven pane draws its own cursor: {tree:#?}"
    );
}

/// The left column's circular wrap stays **kernel-owned**.
///
/// In thurbox's pane, `k` on the first row hands focus to the session list and `j`
/// past the last loops to its top. Both are `App` writing `self.focus` — view
/// state, which no capability writes. The plugin does the half it can: it
/// **declines** the key at its edge. This asserts both that, and that nothing
/// completes it, so adding either half fails here and the finding is revisited.
#[test]
fn the_wrap_out_of_the_pane_stays_kernel_owned() {
    let _guard = SERIALIZE.lock().unwrap_or_else(|e| e.into_inner());
    let host = host();
    let case = Case {
        name: "edges",
        rows: (0..3).map(|_| row("an automation")).collect(),
        selected: 0,
        focus: FocusLevel::Inactive,
        preview: false,
    };
    thurbox::session::pane_context::publish(case.context());

    // At the first row, "previous" is declined.
    assert!(
        !press(&host, "k", Some("prev")),
        "the plugin declines at its first row rather than wrapping its own cursor, \
         which would be a behaviour the native pane does not have"
    );
    // Walk to the last row, where "next" is declined.
    assert!(press(&host, "j", Some("next")));
    assert!(press(&host, "j", Some("next")));
    assert_eq!(
        tree_cursor(&render(&host)),
        Some(2),
        "the last of three rows"
    );
    assert!(!press(&host, "j", Some("next")), "declined at the last row");

    // An empty pane declines both, since there is nowhere to be.
    thurbox::session::pane_context::publish(PaneContext::default());
    assert!(!press(&host, "j", Some("next")));
    assert!(!press(&host, "k", Some("prev")));

    // The kernel's half: an unconsumed key from a focused plugin pane resolves in
    // the *global* context, where no single-letter action lives, so it does
    // nothing. Two probes, because either alone would be the wrong claim.
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let keybindings = std::fs::read_to_string(root.join("src/session/keybindings.rs")).unwrap();
    let contexts = keybindings
        .split_once("pub enum KeyContext {")
        .expect("KeyContext is declared")
        .1
        .split_once('}')
        .expect("and closed")
        .0;
    assert!(
        !contexts.to_lowercase().contains("plugin"),
        "a plugin pane has no key context of its own — if it gained one, a key a \
         pane declined can now resolve to a scoped action, and the wrap should be \
         completed here rather than left declining"
    );
    let handlers = std::fs::read_to_string(root.join("src/app/key_handlers.rs")).unwrap();
    let resolve = handlers
        .split_once("pub(crate) fn focus_key_context(")
        .expect("focus_key_context exists")
        .1
        .split_once("\n    }")
        .expect("and closes")
        .0;
    assert!(
        !resolve.contains("PluginPane"),
        "the plugin-pane focus still falls through to the global context, so a \
         declined movement key does nothing — completing the wrap means mapping it \
         somewhere, which is the change that should revisit this"
    );
}

/// Fixture for the write half: a real database with three automations in it.
struct WriteFixture {
    _tmp: tempfile::TempDir,
    db_path: PathBuf,
    ids: Vec<i64>,
}

impl WriteFixture {
    fn new() -> Self {
        let tmp = tempfile::tempdir().expect("tempdir");
        let db_path = tmp.path().join("thurbox.db");
        let db = Database::open(&db_path).expect("database");
        let ids = ["first", "second", "third"]
            .iter()
            .map(|name| {
                db.create_automation(&NewAutomation {
                    name: (*name).to_string(),
                    enabled: true,
                    schedule: AutomationSchedule::Cron {
                        expr: "0 9 * * *".to_string(),
                    },
                    action: AutomationAction::Exec {
                        command: "true".to_string(),
                    },
                    prompt: String::new(),
                    timezone: None,
                    next_run_at: None,
                })
                .expect("automation")
            })
            .collect();
        Self {
            _tmp: tmp,
            db_path,
            ids,
        }
    }

    fn db(&self) -> Database {
        Database::open(&self.db_path).expect("database")
    }

    /// Publish the fixture's rows so the plugin's cursor has something to name.
    fn publish(&self, enabled: &[bool]) {
        thurbox::session::pane_context::publish(PaneContext {
            automations: AutomationsSnapshot {
                entries: self
                    .ids
                    .iter()
                    .zip(enabled)
                    .map(|(id, enabled)| AutomationRowSnapshot {
                        id: *id,
                        name: format!("automation {id}"),
                        enabled: *enabled,
                        action: "exec",
                        schedule: "daily 09:00".to_string(),
                        due_in_secs: None,
                        dimmed: false,
                        match_positions: Vec::new(),
                    })
                    .collect(),
                cursor: Some(0),
                cursor_visible: false,
                focused: false,
            },
            ..PaneContext::default()
        });
    }
}

/// The keys reach the database — the same rows the native keys write, through the
/// write seam, addressed by the id the published row carried.
///
/// Asserted against storage rather than against the plugin's own state: a pane that
/// moved a cursor and called nothing would pass a test of its cursor.
#[test]
fn the_write_keys_change_the_rows_the_native_keys_change() {
    let _guard = SERIALIZE.lock().unwrap_or_else(|e| e.into_inner());
    let f = WriteFixture::new();
    let host = host_with_writer(&f.db_path);
    // A freshly created automation is enabled.
    let enabled: Vec<bool> = f.ids.iter().map(|_| true).collect();
    f.publish(&enabled);

    // A write key **before** any movement is refused: the fixture publishes
    // `cursor_visible = false` (which is the real state while the plugin's pane
    // holds focus and thurbox's does not), so the pane draws no cursor and there is
    // no row the user can see to act on. Asserted first, because acting here would
    // toggle whichever row thurbox's cursor was left on.
    assert!(
        !press(&host, " ", Some("toggle")),
        "a write with no drawn cursor is declined rather than guessing at a row"
    );
    assert!(
        f.db()
            .get_automation(f.ids[0])
            .unwrap()
            .expect("there")
            .enabled,
        "and nothing was written"
    );

    // A movement key places a cursor the pane draws; from the published row 1, one
    // step down is row 2.
    assert!(press(&host, "j", Some("next")));
    assert!(press(&host, " ", Some("toggle")));
    let db = f.db();
    assert!(
        !db.get_automation(f.ids[1]).unwrap().expect("there").enabled,
        "the toggle key disabled the row the cursor was on"
    );
    assert!(
        db.get_automation(f.ids[0]).unwrap().expect("there").enabled,
        "and only that row"
    );
    drop(db);

    // Move to the third and delete it, which must not touch its neighbours.
    f.publish(&[true, false, true]);
    assert!(press(&host, "j", Some("next")));
    assert!(press(&host, "d", Some("delete")));
    let db = f.db();
    assert!(
        db.get_automation(f.ids[2]).unwrap().is_none(),
        "the delete key removed the row the cursor was on"
    );
    assert!(db.get_automation(f.ids[0]).unwrap().is_some());
    assert!(db.get_automation(f.ids[1]).unwrap().is_some());
}

/// `runAutomation` marks the automation **due** and executes nothing.
///
/// The property that makes this key safe to hand a plugin at all: an automation the
/// *user* authored may run a shell command (this fixture's action is `Exec`), and a
/// plugin can neither author nor edit one — so the set of programs reachable is
/// exactly the set already scheduled, and the kernel is what fires them.
#[test]
fn running_marks_the_automation_due_and_executes_nothing() {
    let _guard = SERIALIZE.lock().unwrap_or_else(|e| e.into_inner());
    let f = WriteFixture::new();
    let host = host_with_writer(&f.db_path);
    f.publish(&[true, true, true]);

    // A movement key first, for `the_write_keys_change_the_rows_the_native_keys_change`'s
    // reason: a write with no drawn cursor is refused. One step down from the
    // published row 1 lands on row 2, so the run target is `ids[1]`.
    assert!(press(&host, "j", Some("next")));
    let target = f.ids[1];

    let before = f.db().get_automation(target).unwrap().expect("there");
    assert!(
        matches!(before.action, AutomationAction::Exec { .. }),
        "the fixture's action is the one that runs a shell command"
    );

    assert!(press(&host, "r", Some("run")));

    let db = f.db();
    let after = db.get_automation(target).unwrap().expect("there");
    let now = thurbox::sync::current_time_millis();
    assert!(
        after.next_run_at.is_some_and(|at| at <= now),
        "the automation is marked due, so the kernel's next pass fires it: {:?}",
        after.next_run_at
    );
    // Nothing ran: firing records a run, and only the kernel's scheduler does that.
    assert!(
        db.list_automation_runs(target, 10).unwrap().is_empty(),
        "no run was recorded, so no plugin thread executed the command"
    );
}

/// A write is refused until the pane draws a cursor of its own — the one place the
/// missing pane-focus fact costs *behaviour*, not only appearance.
///
/// While the plugin's pane holds focus thurbox's does not, so the published
/// `cursor_visible` is false and the pane draws nothing. A write then would act on
/// whichever row thurbox's cursor was left on, which the user cannot see. The
/// plugin refuses. Pinned separately from the write test because the property is
/// "never guess at a row nobody is looking at", and it would be easy to "fix" the
/// refusal into a guess.
#[test]
fn a_write_is_refused_until_the_pane_draws_its_own_cursor() {
    let _guard = SERIALIZE.lock().unwrap_or_else(|e| e.into_inner());
    let f = WriteFixture::new();
    let host = host_with_writer(&f.db_path);
    f.publish(&[true, true, true]);

    for (key, binding) in [(" ", "toggle"), ("r", "run"), ("d", "delete")] {
        assert!(
            !press(&host, key, Some(binding)),
            "`{binding}` must be declined while no cursor is drawn"
        );
    }
    let db = f.db();
    for id in &f.ids {
        let row = db.get_automation(*id).unwrap().expect("still there");
        assert!(row.enabled, "nothing was toggled");
        assert!(row.next_run_at.is_none(), "nothing was marked due");
    }
    drop(db);

    // The counterpart: when the *host* is drawing the cursor — a global-search
    // preview, the one case where an unfocused native pane shows one — the write is
    // permitted without the plugin having moved anything, because there is a row the
    // user can see.
    thurbox::session::pane_context::publish(PaneContext {
        automations: AutomationsSnapshot {
            entries: vec![AutomationRowSnapshot {
                id: f.ids[0],
                name: "previewed".to_string(),
                enabled: true,
                action: "exec",
                schedule: "daily 09:00".to_string(),
                due_in_secs: None,
                dimmed: false,
                match_positions: Vec::new(),
            }],
            cursor: Some(0),
            cursor_visible: true,
            focused: false,
        },
        ..PaneContext::default()
    });
    assert!(press(&host, " ", Some("toggle")));
    assert!(
        !f.db()
            .get_automation(f.ids[0])
            .unwrap()
            .expect("there")
            .enabled
    );
}

/// Before the host has published anything the reader answers "no automations", so
/// the plugin's first render must produce the empty-state pane rather than an
/// error.
#[test]
fn the_first_render_before_any_publication_succeeds() {
    let _guard = SERIALIZE.lock().unwrap_or_else(|e| e.into_inner());
    let host = host();
    thurbox::session::pane_context::publish(PaneContext::default());
    assert_eq!(
        render(&host),
        automations_tree(&[], None, false),
        "an empty publication draws the same pane an empty automation list does"
    );
}
