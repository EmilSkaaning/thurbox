//! The bundled session-list plugin renders the native session list.
//!
//! The same claim the four earlier ports make, for the pane ADR-V1 hinges on: the
//! Luau plugin's view tree must **equal** the one `ui::project_list` builds from
//! the same rows. Since the same renderer paints both
//! (`ui::plugin_pane::render_tree`), an equal tree is a byte-identical pane
//! without comparing a frame — and a failure localises to a node rather than to a
//! cell.
//!
//! Two things are different here, and both are the point:
//!
//! - **The animation is inside the comparison.** A working session's glyph is a
//!   declared motion node in *both* trees, so this port cannot pass by reproducing
//!   everything except the one part of the pane that moves. `ui::project_list`'s
//!   own tests separately pin that node to the frames and rate thurbox's spinner
//!   has always run at.
//! - **The rows are the pane's, the list is not.** The native pane keeps its
//!   ratatui list widget (for its scroll offsets, its two-line items and its click
//!   hitboxes), so it windows by a different rule than `ViewNode::List` does. The
//!   comparison therefore runs at a size where neither windows anything, asserted
//!   by [`the_comparison_size_adjusts_nothing`], and the divergence is enumerated
//!   below rather than absorbed by weakening the equality.
//!
//! It lives in `tests/` for the earlier ports' reason: this is the one place that
//! must see both `ui::project_list` and `plugin::PluginHost`, and an integration
//! test is not part of the library's module graph, so
//! `tests/architecture_rules.rs` stays untouched.
//!
//! ## Two edges, because a handover deletes one of them
//!
//! The equality is **differential** — it names `session_list_tree`, which lives in
//! `src/ui/project_list.rs`, the module a handover removes. On that day the
//! comparison has nothing on its right-hand side, and the repair that compiles is
//! to drop it: what remains is a test that the plugin renders without erroring,
//! satisfied equally by a pane drawing one wrong row and by one drawing twenty.
//! The acceptance snapshots do not cover the gap either — every one is captured
//! with no active session, so none holds a row of this pane.
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
//! The three enumerated divergences below record **inequality**, so no recording
//! is attached to them: an expectation states what the pane should be, and a case
//! that exists to differ has nothing for one to say.
//!
//! Each of the three is **also** a row in `tests/session_list_pane_handover_gap.rs`
//! (ADR-57), and the pair is deliberate: these assertions fail when a divergence
//! *closes*, forcing this port to be revisited; those rows fail when the tree stops
//! matching the recorded handover verdict. A divergence documented only here would be a
//! verdict written in prose, which is the expiry that gate exists to prevent.

#![cfg(feature = "plugins")]

use std::path::PathBuf;

use thurbox::plugin::{discovery, ExecutionBounds, PluginHost};
use thurbox::session::pane_context::{
    PaneContext, SessionListSnapshot, SessionRowSnapshot, StatusSnapshot,
};
use thurbox::session::session_list::{
    compute_session_order, resolve_rows, OrderedSessions, RowInputs, SessionMatch,
};
use thurbox::session::view_tree::ViewNode;
use thurbox::session::{SessionId, SessionInfo, SessionStatus, WorktreeInfo};
use thurbox::ui::project_list::{session_list_tree, SessionListItem};

/// A column wide enough that the geometry step fits nothing, and a height that
/// windows nothing. Both are checked rather than trusted.
const WIDE: usize = 80;

/// The recorder that turns a view tree into the checked-in expectation, shared
/// with the other panes that record one (see its module note for why this is
/// shared while the input gates' source-reading helpers are copied).
mod view_tree_record;

/// The process-wide snapshot slot is global, so every case here runs one at a
/// time — otherwise one case's publication would answer another's reader.
static SERIALIZE: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// One fixture session, in the form the *model* has it.
struct Row {
    name: &'static str,
    status: SessionStatus,
    repo: &'static str,
    /// Index into the case's rows of this session's parent, if it has one.
    parent: Option<usize>,
    remote: bool,
    worktree: bool,
    activity: Option<&'static str>,
    notification: Option<&'static str>,
    /// Byte offsets a search matched in the name; `None` means the search missed
    /// this row, which is what dims it.
    matches: Option<Vec<usize>>,
}

fn row(name: &'static str, status: SessionStatus, repo: &'static str) -> Row {
    Row {
        name,
        status,
        repo,
        parent: None,
        remote: false,
        worktree: false,
        activity: None,
        notification: None,
        matches: None,
    }
}

/// One comparison case.
///
/// Both sides are derived from `rows` by [`Case::inputs`], so an equality failure
/// is a statement about the plugin rather than about two hand-written fixtures
/// drifting apart.
struct Case {
    name: &'static str,
    rows: Vec<Row>,
    /// Which session the cursor is on.
    active: usize,
    /// Whether the cursor is shown at all.
    show_selection: bool,
    /// Whether a search is running (which is what dims a non-matching row).
    searching: bool,
}

fn case(name: &'static str, rows: Vec<Row>, active: usize) -> Case {
    Case {
        name,
        rows,
        active,
        show_selection: true,
        searching: false,
    }
}

/// The sessions a case describes, with ids assigned so parent links resolve.
fn sessions_of(rows: &[Row]) -> Vec<SessionInfo> {
    let ids: Vec<SessionId> = rows.iter().map(|_| SessionId::default()).collect();
    rows.iter()
        .enumerate()
        .map(|(i, r)| {
            let mut info = SessionInfo::new(r.name.to_string());
            info.id = ids[i];
            info.status = r.status;
            info.repo_display_names = vec![r.repo.to_string()];
            info.parent_session_id = r.parent.map(|p| ids[p]);
            info.remote_host = r.remote.then(|| "devbox".to_string());
            if r.worktree {
                info.worktrees.push(WorktreeInfo {
                    repo_path: PathBuf::from("/repos").join(r.repo),
                    worktree_path: PathBuf::from("/tmp/wt").join(r.name),
                    branch: format!("feat/{}", r.name),
                });
            }
            info.agent_activity = r.activity.map(str::to_string);
            info.notification = r.notification.map(str::to_string);
            // Manual order, so the rendered order is the fixture's order rather
            // than a creation-time one a test cannot see.
            info.display_order = Some(i as i64);
            info
        })
        .collect()
}

/// The native pane's resolved items, and the snapshot a plugin reads — both from
/// the same inputs, which is what makes the equality a claim about the plugin.
struct Resolved {
    items: Vec<SessionListItem>,
    snapshot: SessionListSnapshot,
}

impl Case {
    fn resolve(&self, inner_width: usize) -> Resolved {
        let sessions = sessions_of(&self.rows);
        let refs: Vec<&SessionInfo> = sessions.iter().collect();
        let order = compute_session_order(&refs);
        let matches: Vec<Option<SessionMatch>> = if self.searching {
            self.rows
                .iter()
                .map(|r| {
                    r.matches
                        .clone()
                        .and_then(|m| SessionMatch::from_matches(Some(m), None, None, None, None))
                })
                .collect()
        } else {
            Vec::new()
        };
        let ordered = OrderedSessions::from_order(&refs, &order, &matches, self.active);
        let inputs = RowInputs {
            sessions: &ordered.sessions,
            active_index: ordered.active_index,
            show_selection: self.show_selection,
            match_positions: &ordered.match_positions,
            search_active: self.searching,
            headers: ordered.headers,
            depths: ordered.depths,
        };

        // The published rows come from the *same* `resolve_rows` the pane's
        // geometry step builds on, mirroring `App::build_session_list_snapshot`:
        // the kernel is what knows which row opens a group and which the cursor is
        // on, so publishing that per row is what stops the plugin having to
        // reconstruct a rule it cannot see the inputs to.
        let rows = resolve_rows(&inputs)
            .into_iter()
            .zip(&ordered.sessions)
            .map(|((group, row), info)| SessionRowSnapshot {
                name: row.name,
                status: StatusSnapshot::of(row.status),
                group,
                depth: row.depth,
                cross_group_child: row.cross_group_child,
                remote: row.remote,
                worktree: row.worktree,
                selected: row.selected,
                dimmed: row.dimmed,
                match_positions: row.name_matches,
                activity: info.agent_activity.clone(),
                notification: info.notification.clone(),
            })
            .collect();

        Resolved {
            items: thurbox::ui::project_list::resolve_items(&inputs, inner_width),
            snapshot: SessionListSnapshot { rows },
        }
    }

    fn native_tree(&self, inner_width: usize) -> ViewNode {
        session_list_tree(&self.resolve(inner_width).items)
    }

    fn context(&self, inner_width: usize) -> PaneContext {
        PaneContext {
            session_list: self.resolve(inner_width).snapshot,
            ..PaneContext::default()
        }
    }

    /// The recording's name: the case name, slugified.
    ///
    /// Derived from the name rather than written twice, so a renamed case cannot
    /// keep asserting against another case's recording.
    fn snapshot_name(&self) -> String {
        self.name.replace([' ', '\''], "-")
    }
}

/// The variants. Each isolates one decision the pane makes, so a failure names
/// which part of a row the plugin gets wrong rather than only that it does.
fn cases() -> Vec<Case> {
    vec![
        // One row per status: six glyphs, six colour tokens — and the working one
        // is the animated case.
        case(
            "one row per status",
            vec![
                row("resting", SessionStatus::Idle, "thurbox"),
                row("busy", SessionStatus::Working, "thurbox"),
                row("waiting", SessionStatus::Blocked, "thurbox"),
                row("finished", SessionStatus::Done, "thurbox"),
                row("crashed", SessionStatus::Error, "thurbox"),
                row("offline", SessionStatus::Unreachable, "thurbox"),
            ],
            0,
        ),
        // Several repo groups: a header opens each, and only the first row of each
        // carries one.
        case(
            "three repo groups",
            vec![
                row("a", SessionStatus::Idle, "webapp"),
                row("b", SessionStatus::Idle, "webapp"),
                row("c", SessionStatus::Working, "infra"),
                row("d", SessionStatus::Idle, "docs"),
            ],
            2,
        ),
        // Parent/child nesting inside a group, two levels deep.
        Case {
            rows: vec![
                row("lead", SessionStatus::Working, "thurbox"),
                Row {
                    parent: Some(0),
                    ..row("worker", SessionStatus::Idle, "thurbox")
                },
                Row {
                    parent: Some(1),
                    ..row("grandchild", SessionStatus::Idle, "thurbox")
                },
            ],
            ..case("nested children", Vec::new(), 1)
        },
        // A child whose parent renders in another repo group: marked rather than
        // indented, which is a different prefix from nesting.
        Case {
            rows: vec![
                row("parent-elsewhere", SessionStatus::Idle, "infra"),
                Row {
                    parent: Some(0),
                    ..row("child-here", SessionStatus::Idle, "webapp")
                },
            ],
            ..case("a cross-group child", Vec::new(), 1)
        },
        // The prefix marks, alone and together, including on the selected row
        // where the selection replaces their colours.
        case(
            "remote and worktree marks",
            vec![
                Row {
                    remote: true,
                    ..row("remote-only", SessionStatus::Idle, "thurbox")
                },
                Row {
                    worktree: true,
                    ..row("worktree-only", SessionStatus::Idle, "thurbox")
                },
                Row {
                    remote: true,
                    worktree: true,
                    ..row("both", SessionStatus::Working, "thurbox")
                },
            ],
            2,
        ),
        // The agent's reported text: an activity title, a blocked notification, a
        // blocked session with nothing reported (which falls back to the state's
        // label), and whitespace-only activity (which reports nothing).
        case(
            "reported text",
            vec![
                Row {
                    activity: Some("Compacting"),
                    ..row("streaming", SessionStatus::Working, "thurbox")
                },
                Row {
                    notification: Some("Allow edit to src/main.rs?"),
                    ..row("asking", SessionStatus::Blocked, "thurbox")
                },
                row("blocked-silent", SessionStatus::Blocked, "thurbox"),
                Row {
                    activity: Some("   "),
                    ..row("blank-activity", SessionStatus::Idle, "thurbox")
                },
                Row {
                    activity: Some("  padded  "),
                    ..row("trimmed-activity", SessionStatus::Idle, "thurbox")
                },
            ],
            1,
        ),
        // A running search: matched rows, a dimmed one, and a row that is both
        // matched and selected — where emphasis layers over a base that already
        // has its own.
        Case {
            searching: true,
            rows: vec![
                Row {
                    matches: Some(vec![0, 5]),
                    ..row("host tests", SessionStatus::Idle, "thurbox")
                },
                Row {
                    matches: None,
                    remote: true,
                    worktree: true,
                    activity: Some("Compacting"),
                    ..row("nothing matched here", SessionStatus::Working, "thurbox")
                },
                Row {
                    matches: Some(vec![0, 3, 9]),
                    notification: Some("needs you"),
                    ..row("selected and matched", SessionStatus::Blocked, "thurbox")
                },
            ],
            ..case("a running search", Vec::new(), 2)
        },
        // Multi-byte names with matches inside them: the byte-offset walk is the
        // part of this plugin most likely to be subtly wrong.
        Case {
            searching: true,
            rows: vec![
                Row {
                    matches: Some(vec![0, 7]),
                    ..row("héllo — wörld", SessionStatus::Idle, "thurbox")
                },
                Row {
                    matches: Some(vec![0]),
                    ..row("日本語のセッション", SessionStatus::Working, "thurbox")
                },
                // An offset landing *inside* a multi-byte character, which the
                // host's segmentation skips rather than slicing there: `"abc… def"`
                // puts the ellipsis at bytes 3..6, so 4 is inside it.
                Row {
                    matches: Some(vec![0, 4, 6]),
                    ..row("abc… def", SessionStatus::Idle, "thurbox")
                },
            ],
            ..case("multi-byte names", Vec::new(), 0)
        },
        // The cursor hidden: the automations context makes the active session
        // irrelevant, so no row may be highlighted at all.
        Case {
            show_selection: false,
            ..case(
                "no cursor shown",
                vec![
                    row("one", SessionStatus::Idle, "thurbox"),
                    row("two", SessionStatus::Working, "thurbox"),
                ],
                1,
            )
        },
        // Every feature of a row at once, on the cursor's row — the case where the
        // selection bar has to replace six colour roles and still reach the edge.
        Case {
            searching: true,
            rows: vec![
                row("above", SessionStatus::Idle, "thurbox"),
                Row {
                    parent: Some(0),
                    remote: true,
                    worktree: true,
                    notification: Some("Approve?"),
                    matches: Some(vec![0, 2]),
                    ..row("everything", SessionStatus::Blocked, "thurbox")
                },
            ],
            ..case("everything on the cursor's row", Vec::new(), 1)
        },
        // One session with no repo at all: the `(no repo)` fallback group.
        Case {
            rows: vec![Row {
                repo: "",
                ..row("orphan", SessionStatus::Idle, "")
            }],
            ..case("a session with no repo", Vec::new(), 0)
        },
    ]
}

/// Start the bundled plugin from the source that ships.
fn host() -> PluginHost {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/plugin/bundled/session-list");
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
    host.render_pane("session-list", "sessions")
        .expect("the pane renders")
}

/// The headline claim: for every case, the plugin's tree equals the native pane's.
/// Equal trees paint identically, so this is byte-identity of the pane.
///
/// The recorded edge is asserted **first**, so a run in which both edges break
/// reports the recording — the fact about the pane — rather than only that two
/// implementations disagree.
#[test]
fn the_plugin_builds_the_native_panes_view_tree() {
    let _guard = SERIALIZE.lock().unwrap_or_else(|e| e.into_inner());
    let host = host();

    for case in cases() {
        thurbox::session::pane_context::publish(case.context(WIDE));
        let plugin = render(&host);
        let native = case.native_tree(WIDE);
        // Edge one: the checked-in expectation is the native pane's tree. It is
        // what survives the handover, and the only moment its baseline can be
        // shown to come from the pane rather than from the plugin is while
        // `session_list_tree` is still here to generate it.
        insta::assert_snapshot!(case.snapshot_name(), view_tree_record::tree(&native));
        // Edge two: the plugin reproduces that recording. Asserted through the
        // records rather than the trees, because a failure here has to be
        // readable — the structural equality below says the same thing in two
        // thousand-character dumps a reader has to diff by eye.
        view_tree_record::assert_matches(case.name, &plugin, &native);
        // Edge three: the port's original claim, exact rather than legible. Kept
        // beneath the readable one so a divergence reports itself twice, most
        // useful form first.
        assert_eq!(
            plugin, native,
            "the plugin's tree diverges from the native pane for `{}`",
            case.name
        );
    }
}

/// The equality above is only meaningful at a size where the kernel's geometry
/// step is a no-op, so that is asserted rather than assumed: no reported text
/// fitted, and no row dropped.
#[test]
fn the_comparison_size_adjusts_nothing() {
    for case in cases() {
        let resolved = case.resolve(WIDE);
        let sessions = resolved
            .items
            .iter()
            .filter(|i| matches!(i, SessionListItem::Session(_)))
            .count();
        assert_eq!(
            sessions,
            case.rows.len(),
            "`{}` lost rows at the comparison size",
            case.name
        );
        for item in &resolved.items {
            if let SessionListItem::Session(row) = item {
                if let Some(text) = row.status_text.as_deref() {
                    assert!(
                        !text.ends_with('…'),
                        "`{}` had reported text fitted at the comparison size: {text:?}",
                        case.name
                    );
                }
            }
        }
    }
}

/// The compared tree must be a whole pane, or the equality could pass on two
/// nearly-empty lists — and it must carry the parts that make this pane hard: a
/// header, a nested prefix, marks, a motion, a matched run and a selection fill.
#[test]
fn the_compared_tree_is_a_whole_pane() {
    let case = cases()
        .into_iter()
        .find(|c| c.name == "everything on the cursor's row")
        .expect("the everything case");
    let tree = case.native_tree(WIDE);
    let children = match &tree {
        ViewNode::List {
            children, selected, ..
        } => {
            assert!(
                selected.is_some(),
                "the cursor's row is declared on the list"
            );
            children
        }
        other => panic!("expected a list, got {}", other.kind_name()),
    };
    // Two sessions plus the group header that opens their group.
    assert_eq!(children.len(), 3, "{tree:#?}");

    let mut kinds = Vec::new();
    fn walk(node: &ViewNode, out: &mut Vec<&'static str>) {
        out.push(node.kind_name());
        node.children().iter().for_each(|c| walk(c, out));
    }
    walk(&tree, &mut kinds);
    for expected in ["line", "text", "fill"] {
        assert!(kinds.contains(&expected), "no {expected} in {tree:#?}");
    }
    // Enough runs that a single text node could not have carried the row, which is
    // why a row is a `line` at all.
    let runs: usize = children.iter().map(|r| r.children().len()).sum();
    assert!(runs >= 10, "{tree:#?}");
}

/// The animation is inside the equality, which is the part a weaker comparison
/// would have exempted: the plugin's own motion node — its key, its rate, its
/// frames and its looping — must be the kernel's.
#[test]
fn the_plugins_animation_is_the_kernels_animation() {
    let _guard = SERIALIZE.lock().unwrap_or_else(|e| e.into_inner());
    let host = host();
    let case = case(
        "working",
        vec![row("busy", SessionStatus::Working, "thurbox")],
        0,
    );
    thurbox::session::pane_context::publish(case.context(WIDE));

    let motion_of = |tree: &ViewNode| -> (String, bool, usize, u8, bool) {
        let mut found = None;
        fn walk(node: &ViewNode, found: &mut Option<(String, bool, usize, u8, bool)>) {
            if let ViewNode::Motion {
                key,
                keyed_by_id,
                motion,
            } = node
            {
                *found = Some((
                    key.clone(),
                    *keyed_by_id,
                    motion.frames().len(),
                    motion.fps,
                    motion.repeats(),
                ));
            }
            node.children().iter().for_each(|c| walk(c, found));
        }
        walk(tree, &mut found);
        found.expect("a working row animates")
    };

    let plugin = motion_of(&render(&host));
    let native = motion_of(&case.native_tree(WIDE));
    assert_eq!(plugin, native);
    assert!(
        plugin.1,
        "a structural key would restart the spinner whenever a sibling appeared"
    );
    assert_eq!(plugin.3, thurbox::session::motion::DEFAULT_FPS);
    assert!(plugin.4, "the spinner loops");
}

/// **Enumerated divergence 1: the empty pane.** The native pane leaves the view
/// tree entirely when there are no sessions — its two lines are drawn *centred*
/// by a `Paragraph`, never by a node. The plugin draws the same words
/// left-aligned.
///
/// The vocabulary gap the port named is **closed** since ADR-62: `ViewNode::Center`
/// places a row centrally and `no-centred-line` is met in the handover gate. The
/// divergence is not, and the distinction is the point of keeping this test — a
/// pane can now be centred and neither of these two has been, because changing
/// either moves a recording. This is what fails if one of them changes and the
/// other does not.
#[test]
fn the_empty_pane_is_the_one_place_the_plugin_differs() {
    let _guard = SERIALIZE.lock().unwrap_or_else(|e| e.into_inner());
    let host = host();
    thurbox::session::pane_context::publish(PaneContext::default());
    let plugin = render(&host);

    // The plugin says what the native pane says.
    let mut text = String::new();
    fn walk(node: &ViewNode, out: &mut String) {
        if let ViewNode::Text { content, .. } = node {
            out.push_str(content);
            out.push('\n');
        }
        node.children().iter().for_each(|c| walk(c, out));
    }
    walk(&plugin, &mut text);
    assert!(text.contains("No sessions yet"), "{text:?}");
    assert!(text.contains("Press Ctrl+N to create one"), "{text:?}");

    // And it is *not* the native pane's tree, because the native pane has none
    // here: `session_list_tree` of no items is an empty list.
    assert_ne!(
        plugin,
        session_list_tree(&[]),
        "if these ever agree, the empty state became expressible and this \
         divergence should be retired"
    );
    assert_eq!(session_list_tree(&[]), ViewNode::list(Vec::new()));
}

/// **Enumerated divergence 2: which rows are on screen.** The native pane hands
/// its nodes to a ratatui list, which keeps the cursor visible with its own sticky
/// offset and can draw a two-line item; the plugin declares the cursor's row and
/// the *kernel* windows the list (ADR-30). Both keep the cursor visible; they do
/// not agree on which other rows are beside it, and the plugin's index counts the
/// group headers the native pane folds into the row below.
///
/// Closing it would mean moving the native pane off its list widget, which is what its
/// scroll offsets, its border indicators, its click hitboxes and the pending-spawn
/// placeholder's position are all derived from — Phase 6 work, not a port's. It is
/// `the-window-is-the-list-widgets` in the handover gate, which is where that work is
/// enumerated, and it is one of the two rows deciding that verdict (ADR-57).
#[test]
fn the_two_panes_window_a_long_list_by_different_rules() {
    let _guard = SERIALIZE.lock().unwrap_or_else(|e| e.into_inner());
    let host = host();
    let rows: Vec<Row> = (0..8)
        .map(|i| {
            row(
                ["s0", "s1", "s2", "s3", "s4", "s5", "s6", "s7"][i],
                SessionStatus::Idle,
                "thurbox",
            )
        })
        .collect();
    let case = case("tall", rows, 7);
    thurbox::session::pane_context::publish(case.context(WIDE));

    let (children, selected) = match render(&host) {
        ViewNode::List {
            children, selected, ..
        } => (children.len(), selected),
        other => panic!("expected a list, got {}", other.kind_name()),
    };
    // Eight rows plus the one group header, and the cursor's row is declared as a
    // child index that counts that header.
    assert_eq!(children, 9);
    assert_eq!(
        selected,
        Some(8),
        "the cursor's row is the last child, header included"
    );

    // The native pane's list carries the same nine lines, as eight items — the
    // header travels with the row below it, which is what its hitboxes are built
    // from.
    let items = case.resolve(WIDE).items;
    assert_eq!(items.len(), 9);
    assert_eq!(
        items
            .iter()
            .filter(|i| matches!(i, SessionListItem::Header(_)))
            .count(),
        1
    );
}

/// **Enumerated divergence 3: whitespace outside ASCII.** The kernel trims the
/// agent's activity text with Rust's `str::trim`, which is Unicode-aware; the
/// plugin trims with Luau's `%s`, which is not. A no-break space around an
/// activity title therefore survives in the plugin's copy.
///
/// Left open rather than closed by publishing the trimmed text: the trim is a
/// presentation decision about the pane's own row, and the port's rule is that the
/// kernel publishes no rendering. Recorded as
/// `non-ascii-whitespace-is-the-kernels-trim` in the handover gate.
#[test]
fn non_ascii_whitespace_is_trimmed_by_the_kernel_only() {
    let _guard = SERIALIZE.lock().unwrap_or_else(|e| e.into_inner());
    let host = host();
    let case = case(
        "nbsp",
        vec![Row {
            // U+00A0 either side: `char::is_whitespace` says yes, `%s` says no.
            activity: Some("\u{a0}Compacting\u{a0}"),
            ..row("padded", SessionStatus::Idle, "thurbox")
        }],
        0,
    );
    thurbox::session::pane_context::publish(case.context(WIDE));
    assert_ne!(
        render(&host),
        case.native_tree(WIDE),
        "if these agree, either the trims converged or the fixture stopped \
         exercising the difference"
    );
}

/// The plugin must hold no surface a user's plugin could not declare: its reach is
/// exactly its manifest's capability list, and it is two things.
#[test]
fn the_plugin_declares_every_power_it_uses() {
    use thurbox::session::plugin_manifest::Capability;
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/plugin/bundled/session-list");
    let outcome = discovery::discover_in(&[dir], None);
    let plugin = outcome.get("session-list").expect("discovered");
    let declared = &plugin.manifest.capabilities;
    for required in [Capability::Render, Capability::Sessions] {
        assert!(declared.contains(&required), "{required} must be declared");
    }
    assert_eq!(
        declared.len(),
        2,
        "a bundled pane that quietly asked for more would stop being evidence \
         about what a third party can build: {declared:?}"
    );
    // The pane that draws every session holds nothing that *acts* on one: the
    // vocabulary defines no capability that spawns, kills, focuses or writes.
    assert!(
        !plugin.manifest.capabilities.contains(&Capability::Input),
        "the reproduction receives no keys — the cursor it draws is the kernel's"
    );

    // Additive port: the pane must not appear on anyone's screen unasked, since
    // the native pane is still the one thurbox draws.
    assert!(
        !plugin.manifest.panes[0].default_visible,
        "the reproduction must be hidden by default"
    );
}

/// The result this port is really about: the host surface needed **nothing new**.
///
/// Asserted rather than described, because "the API sufficed" is the claim ADR-V1
/// rests on and a claim in a document expires without telling anyone. Every node
/// kind and style field this pane uses was already in the catalogue before the
/// port — four of them added for panes that were each easier than this one.
#[test]
fn the_host_surface_needed_no_new_node() {
    let case = cases()
        .into_iter()
        .find(|c| c.name == "everything on the cursor's row")
        .expect("the everything case");
    let tree = case.native_tree(WIDE);

    let mut kinds = std::collections::BTreeSet::new();
    fn walk(node: &ViewNode, out: &mut std::collections::BTreeSet<&'static str>) {
        out.insert(node.kind_name());
        node.children().iter().for_each(|c| walk(c, out));
    }
    walk(&tree, &mut kinds);
    // The whole pane is four node kinds, every one of which predates this change.
    assert_eq!(
        kinds.into_iter().collect::<Vec<_>>(),
        vec!["fill", "line", "list", "text"],
        "a new node kind here means the catalogue was widened for this pane, \
         which is the opposite of the result this test records"
    );

    // And no new capability: the reader this pane needs is gated on the grant that
    // already read a session.
    use thurbox::session::plugin_manifest::Capability;
    assert!(
        Capability::all()
            .iter()
            .all(|c| c.as_str() != "session-list"),
        "the session list is read under the `sessions` grant, not a new one"
    );
}

/// Before the host has published anything the reader answers "no sessions", so the
/// plugin's first render must produce its empty-state pane rather than an error.
#[test]
fn the_first_render_before_any_publication_succeeds() {
    let _guard = SERIALIZE.lock().unwrap_or_else(|e| e.into_inner());
    let host = host();
    // `clear_for_test` is crate-internal, so an integration test publishes the
    // default instead: the reader answers from an empty section either way, which
    // is the state this asserts about.
    thurbox::session::pane_context::publish(PaneContext::default());
    let tree = render(&host);
    assert!(
        matches!(&tree, ViewNode::List { children, .. } if children.len() == 2),
        "an unpublished snapshot draws the empty state: {tree:#?}"
    );
}
