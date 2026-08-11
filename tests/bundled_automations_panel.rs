//! The bundled automations plugin **is** thurbox's automations pane.
//!
//! `src/ui/automations_panel.rs` is deleted (ADR-56), so the claim this file makes is
//! the one `tests/bundled_tasks_panel.rs` makes: the plugin's view tree must equal the
//! **recording** of the native pane's tree, and the recordings were generated from
//! `ui::automations_panel::automations_tree` while it existed (ADR-42/48). They are the
//! reason a handover has evidence at all — a comparison against the builder the
//! deletion removes could not fail afterwards.
//!
//! ## What this file lost, and the one thing it kept
//!
//! Before the handover the pane held its **own** keys — `input`, `automations-write`,
//! five bindings and a cursor of its own across renders — and five tests here measured
//! them against the database. Those keys are gone: the pane declares
//! `key_context = "Automations"` and the *kernel* performs all seven scoped actions, so
//! there is nothing here to send a key to. The tests went with them, and what they
//! proved is preserved in ADR-56, because it is still the answer for a pane that wants
//! keys of its own.
//!
//! What is **kept**, and would have been wrong to drop with the rest, is
//! [`the_plugin_composes_the_summary_thurbox_composes`]. Its right-hand side is
//! `ui::automations_list_modal::row_summary`, which survives the deletion because the
//! `Ctrl+P` list modal composes it too — so that edge is not differential, and it is the
//! only assertion here that holds the plugin to a *rule* rather than to a fixed set of
//! cases (192 combinations of schedule, action, enabled and countdown; no snapshot set
//! covers that). Deciding what a handover deletes means asking of each edge whether its
//! right-hand side is going, not whether the change is a handover.
//!
//! Two enumerated divergences were retired before the handover and both are kept
//! pointing the other way: the **placement** (ADR-46 — the pane declares the seat the
//! native band occupied) and the **fitted name** (ADR-55 — neither side cuts a name; the
//! runs declare that they yield their width and the kernel cuts them).
//!
//! It lives in `tests/` for its predecessors' reason: it must see both `plugin::PluginHost`
//! and `ui`, and an integration test is not part of the library's module graph, so
//! `tests/architecture_rules.rs` stays untouched.

#![cfg(feature = "plugins")]

use std::path::{Path, PathBuf};

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

use thurbox::plugin::{discovery, ExecutionBounds, PluginHost};
use thurbox::session::motion::FrameTable;
use thurbox::session::pane_context::{AutomationRowSnapshot, AutomationsSnapshot, PaneContext};
use thurbox::session::plugin_manifest::Capability;
use thurbox::session::view_tree::ViewNode;
use thurbox::ui::automations_list_modal::row_summary;
use thurbox::ui::FocusLevel;

/// The recorder that turns a view tree into the checked-in expectation, shared
/// with the other panes that record one (see its module note for why this is
/// shared while the input gates' source-reading helpers are copied).
mod view_tree_record;

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
    /// The cursor's row, clamped — the one rule `cursor_row` and
    /// `App::build_automations_snapshot` share, since the host refuses a list whose
    /// cursor is not an index into its children.
    fn cursor(&self) -> Option<usize> {
        (!self.rows.is_empty()).then(|| self.selected.min(self.rows.len() - 1))
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

fn render(host: &PluginHost) -> ViewNode {
    host.render_pane("automations", "automations")
        .expect("the pane renders")
}

/// Paint a tree into a bare buffer of the given size — the renderer the host paints a
/// seated pane's tree through, so a frame assertion here is a statement about what the
/// interface draws.
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

/// The headline claim: for every case, the pane's tree is the one recorded from the
/// native builder before it was deleted.
///
/// The recording is the whole evidence now (ADR-42): `automations_tree` is gone, so a
/// differential comparison has no right-hand side, and the recordings were deliberately
/// **not** regenerated by the handover — byte-identical after the deletion is the payoff.
/// Equal trees paint identically, so this is byte-identity of the pane's rows.
#[test]
fn the_pane_draws_the_recorded_tree() {
    let _guard = SERIALIZE.lock().unwrap_or_else(|e| e.into_inner());
    let host = host();

    for case in cases() {
        thurbox::session::pane_context::publish(case.context());
        insta::assert_snapshot!(case.snapshot_name(), view_tree_record::tree(&render(&host)));
    }
}

/// The compared tree must be a whole pane, or the recording could be satisfied by a
/// nearly-empty list.
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
/// The comparison is against `ui::automations_list_modal::row_summary` rather than
/// against a string written here, which is the point: the test cannot agree with the
/// plugin about a rule thurbox does not follow.
///
/// **The one edge the handover kept.** That function survives the deletion because the
/// `Ctrl+P` list modal composes it too, so this comparison is not differential — and it
/// is the only assertion in this file that holds the pane to a rule rather than to a
/// fixed set of recorded cases.
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

/// **The gap the file viewer closed, held here.** The pane names the row its cursor is
/// on and the *renderer* resolves the window, so a pane that knows no height still
/// scrolls. The recordings cannot say it — a tree is the same tree at every height — so
/// only a painted frame shows the window was resolved from the declared cursor.
#[test]
fn the_pane_scrolls_to_its_cursor_from_a_height_it_is_never_told() {
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
    // `an_overflowing_name_is_cut_and_keeps_its_summary`' subject.
    const WIDTH: u16 = 60;
    const HEIGHT: u16 = 3;
    let painted = paint(&render(&host), WIDTH, HEIGHT);

    let text: String = (0..HEIGHT)
        .flat_map(|y| (0..WIDTH).map(move |x| (x, y)))
        .map(|(x, y)| painted[(x, y)].symbol().to_string())
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

/// **The retired divergence: a name wider than the column.** This was the pane's last
/// enumerated difference and it became its opposite before the handover (ADR-55).
///
/// The native pane cut the name itself from a width the plugin is never told, and the
/// plugin drew it whole and let the renderer clip at the pane's edge — which took the
/// schedule, the action and the countdown with it. Now the name's runs declare that they
/// yield their width and the kernel cuts them with `ui::truncate_ellipsis`, keeping every
/// other run's columns. The recordings cannot say it, since the tree carries the whole
/// name; only a painted frame at a width where the fit fires can.
#[test]
fn an_overflowing_name_is_cut_and_keeps_its_summary() {
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
    // The marker leads, the name ends in an ellipsis, and the whole summary tail
    // survives — which is the half a clip at the pane's edge lost.
    let drawn = row_text(&paint(&render(&host), NARROW, 1), NARROW);
    assert!(drawn.contains('…'), "the name is ellipsized: {drawn:?}");
    assert!(
        drawn.contains("daily 09:00 · spawn · in 2m"),
        "the summary tail keeps its columns: {drawn:?}"
    );

    // At a width where the marker and the tail leave the name no columns at all it is
    // dropped entirely — `truncate_ellipsis` returns nothing rather than a lone `…` that
    // carries no information, which is the native pane's rule and the renderer applies
    // it. This is where the retired divergence used to be sharpest: the two panes showed
    // different *information*, not different punctuation.
    const CRAMPED: u16 = 30;
    let drawn = row_text(&paint(&render(&host), CRAMPED, 1), CRAMPED);
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
/// Kept rather than deleted, pointing the other way: it pins that the shipped manifest
/// names the seat its native counterpart occupied, which after the handover is not a
/// comparison aid but the pane's *position*. A regression to the right column would move
/// the automations band into the tasks/file-viewer column.
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

/// The pane must hold no surface a user's plugin could not declare: its reach is exactly
/// its manifest's capability list, and it is **two** things.
///
/// It was four. The port held `input` and `automations-write` and answered five of the
/// pane's seven keys with a cursor of its own; the handover took ADR-51's route instead,
/// so the kernel performs all seven and the plugin sees no key. That the pane with the
/// most keys in thurbox ends up with the *fewest* capabilities is the finding, not a
/// side effect — so this test asserts the absence of the two it gave up as firmly as the
/// presence of the two it kept.
#[test]
fn the_pane_declares_every_power_it_uses_and_only_those() {
    let outcome = discovery::discover_in(&[plugin_dir()], None);
    let plugin = outcome.get("automations").expect("discovered");
    let declared = &plugin.manifest.capabilities;
    for required in [Capability::Render, Capability::Automations] {
        assert!(declared.contains(&required), "{required} must be declared");
    }
    assert_eq!(
        declared.len(),
        2,
        "a bundled pane that quietly asked for more would stop being evidence \
         about what a third party can build: {declared:?}"
    );
    // The two the handover gave up, and the ones no version of this pane ever had.
    // Answering a key needs none of them: the kernel performs the action (ADR-51).
    for absent in [
        Capability::Input,
        Capability::AutomationsWrite,
        Capability::TasksWrite,
        Capability::Sessions,
        Capability::Spawn,
        Capability::StateWrite,
    ] {
        assert!(!declared.contains(&absent), "{absent} must not be declared");
    }
    // And no bindings of its own: a pane on the kernel-keyboard route may declare
    // none, and one that declared both routes is refused at validation.
    assert!(
        plugin.manifest.keybindings.is_empty(),
        "the pane declares bindings: {:?}",
        plugin.manifest.keybindings
    );
    assert_eq!(
        plugin.manifest.panes[0].key_context,
        Some(thurbox::session::KeyContext::Automations),
        "the pane must declare that it *is* thurbox's automations pane"
    );
    // Visible, because the band it replaced always was — the first bundled pane of
    // which that is true, and the reason it binds no toggle action.
    assert!(
        plugin.manifest.panes[0].default_visible,
        "the pane replaces an always-visible band, so it seeds visible"
    );
    assert!(
        plugin.manifest.panes[0].toggle_action.is_none(),
        "the native band had no toggle, so there is no key for this pane to take over"
    );
}

/// Before the host has published anything the reader answers "no automations", so the
/// pane's first render must produce the empty-state pane rather than an error.
///
/// Asserted against the recording of the unfocused empty case rather than against the
/// deleted builder: "an empty publication and an empty automation list draw the same
/// pane" is the claim, and the recording is what says what that pane is.
#[test]
fn the_first_render_before_any_publication_succeeds() {
    let _guard = SERIALIZE.lock().unwrap_or_else(|e| e.into_inner());
    let host = host();
    let empty = cases()
        .into_iter()
        .find(|c| c.name == "none, unfocused")
        .expect("the unfocused empty case");
    thurbox::session::pane_context::publish(PaneContext::default());
    let before_publication = view_tree_record::tree(&render(&host));
    thurbox::session::pane_context::publish(empty.context());
    assert_eq!(
        before_publication,
        view_tree_record::tree(&render(&host)),
        "an empty publication draws the same pane an empty automation list does"
    );
}
