//! The session list's contract with the rest of the interface.
//!
//! Two things every other pane depends on and neither of which is visible in what
//! the list draws: `Enter` goes to the session you picked, and the selection is
//! **steerable** — a pane that writes `store.selected` moves the cursor rather
//! than being overwritten on the next frame. v1 has one `App::select_session` for
//! that; here it is a value two plugins share, so the rule has to be asserted.

use ratatui::backend::TestBackend;
use ratatui::Terminal;

use thurbox::kernel::command::{Command, InFlight, Phase};
use thurbox::kernel::host::{LuaHost, PluginError};
use thurbox::kernel::paint::{self, PlaceholderSurfaces};
use thurbox::kernel::snapshot::{GitState, SessionRow, Snapshot};

mod common;

use common::{ctx, host, index_of, press, publish, publish_with, registry};

const PLUGIN: &str = "sessions";

fn row(id: &str, name: &str) -> SessionRow {
    SessionRow {
        cwd: Some(std::path::PathBuf::from("/src/thurbox")),
        ..common::session_row(id, name)
    }
}

fn snapshot() -> Snapshot {
    Snapshot {
        sessions: vec![row("aaa", "first"), row("bbb", "second")],
        ..Snapshot::default()
    }
}

/// Render the list, which is also what publishes the selection.
fn render(host: &LuaHost) {
    render_in(host, &snapshot());
}

fn render_in(host: &LuaHost, snapshot: &Snapshot) {
    publish(host, snapshot);
    host.render(index_of(host, PLUGIN), ctx(30, 12, true))
        .expect("render");
}

fn press_in(host: &LuaHost, snapshot: &Snapshot, chord: &str) {
    publish(host, snapshot);
    dispatch(host, chord).expect("the pane must survive its own keys");
}

/// Route a chord the way the binary does — registry first, then raw `on_key` —
/// and hand back the failure instead of unwrapping it, so a test can assert
/// that a key does not take the pane down.
fn dispatch(host: &LuaHost, chord: &str) -> Result<(), PluginError> {
    let index = index_of(host, PLUGIN);
    let key = press(chord);
    if let Some(binding) = registry(host).resolve(&key, Some(PLUGIN)) {
        let action = binding.action.clone();
        if host.on_action(index, &action)? {
            return Ok(());
        }
    }
    host.on_key(index, &key).map(|_| ())
}

#[test]
fn enter_opens_the_selected_session() {
    // v1's Enter on a row moves focus to the terminal; the agent pane is what
    // shows a session here, so opening is a focus change.
    let host = host();
    render(&host);
    press_in(&host, &snapshot(), "enter");
    assert_eq!(
        host.drain_commands(),
        vec![Command::Focus {
            plugin: "agent".into(),
            toggle: false,
        }]
    );
}

#[test]
fn enter_is_declared_so_help_lists_it_and_it_can_be_rebound() {
    let host = host();
    let index = host.index_of(PLUGIN).expect("no sessions plugin");
    assert!(
        host.plugins[index]
            .bindings
            .iter()
            .any(|binding| binding.chord == "enter"),
        "a key that only exists inside on_key is invisible to help and unrebindable"
    );
}

#[test]
fn the_list_publishes_the_session_under_its_cursor() {
    let host = host();
    render(&host);
    assert_eq!(host.shared_string("selected").as_deref(), Some("aaa"));
    press_in(&host, &snapshot(), "j");
    render(&host);
    assert_eq!(host.shared_string("selected").as_deref(), Some("bbb"));
}

#[test]
fn another_pane_can_steer_the_selection() {
    // The gap this closes: the list republished its own cursor every frame, so a
    // pane that wrote `store.selected` — a search result, a task opening its
    // session — was undone a frame later and the jump silently did nothing.
    let host = host();
    render(&host);
    assert_eq!(host.shared_string("selected").as_deref(), Some("aaa"));

    host.set_shared_string("selected", "bbb");
    render(&host);
    assert_eq!(
        host.shared_string("selected").as_deref(),
        Some("bbb"),
        "the write survived the next render"
    );

    // And the cursor really moved with it, rather than the value merely sticking:
    // stepping on lands past the steered row, not past the old one.
    press_in(&host, &snapshot(), "k");
    render(&host);
    assert_eq!(host.shared_string("selected").as_deref(), Some("aaa"));
}

#[test]
fn a_session_that_went_away_does_not_freeze_the_selection() {
    // Steering at an id the list cannot show must not strand the cursor: the list
    // keeps its own, and the request is simply not honoured.
    let host = host();
    render(&host);
    host.set_shared_string("selected", "gone");
    render(&host);
    assert_eq!(
        host.shared_string("selected").as_deref(),
        Some("aaa"),
        "the cursor stayed on a row that exists"
    );
}

/// A worktree with nothing in it that a delete could not put back.
fn clean() -> GitState {
    GitState {
        files_changed: 0,
        insertions: 0,
        deletions: 0,
        untracked: 0,
        dirty: false,
        ahead: 0,
        behind: 0,
    }
}

/// Render the confirmation float and return what it drew, so a test can ask
/// whether a question was put at all — and what it itemised.
fn confirm_tree(host: &LuaHost, snapshot: &Snapshot) -> String {
    publish(host, snapshot);
    let rendered = host
        .render(index_of(host, "confirm"), ctx(60, 12, false))
        .expect("render the confirmation");
    format!("{:?}", rendered.node)
}

#[test]
fn force_deleting_a_clean_session_does_not_ask() {
    // v1's rule, in `App::delete_active_session`: `assess_delete_risk` returning
    // `Some(risk)` opened `ConfirmDelete`, `None` deleted on the keystroke. v2
    // asked every time, which is how the answer to a question stops being a
    // decision.
    let host = host();
    let mut snapshot = snapshot();
    snapshot.sessions[0].git = Some(clean());
    render_in(&host, &snapshot);
    press_in(&host, &snapshot, "D");

    assert_eq!(
        host.drain_commands(),
        vec![Command::Delete {
            session: "aaa".into(),
            force: true,
        }],
        "a known-clean session is torn down on the keystroke"
    );
    assert!(
        !confirm_tree(&host, &snapshot).contains("Confirm"),
        "and no question was put: a worktree directory alone is not work at risk"
    );
}

#[test]
fn force_deleting_a_session_with_work_asks_first_and_says_what_is_lost() {
    let host = host();
    let mut snapshot = snapshot();
    snapshot.sessions[0].git = Some(GitState {
        files_changed: 2,
        untracked: 1,
        dirty: true,
        ahead: 3,
        ..clean()
    });
    snapshot.sessions[0].worktree_count = 1;
    render_in(&host, &snapshot);
    press_in(&host, &snapshot, "D");

    assert!(
        host.drain_commands().is_empty(),
        "nothing is torn down until the question is answered"
    );
    let tree = confirm_tree(&host, &snapshot);
    assert!(tree.contains("and its worktree?"), "the question: {tree}");
    assert!(
        tree.contains("3 uncommitted or untracked file(s)"),
        "{tree}"
    );
    assert!(
        tree.contains("3 commit(s) not pushed anywhere else"),
        "{tree}"
    );
    assert!(
        tree.contains("its worktree directory"),
        "listed as what else goes, once a question is owed: {tree}"
    );
}

#[test]
fn a_clean_primary_does_not_speak_for_the_other_worktrees() {
    // The snapshot stats one directory per session, so on a multi-worktree
    // session a clean answer covers the primary and nothing else. v1 assessed
    // every worktree it was about to remove; not being able to is unknown.
    let host = host();
    let mut snapshot = snapshot();
    snapshot.sessions[0].git = Some(clean());
    snapshot.sessions[0].worktree_count = 2;
    render_in(&host, &snapshot);
    press_in(&host, &snapshot, "D");

    assert!(
        host.drain_commands().is_empty(),
        "a checkout nobody read must not be torn down unasked"
    );
    let tree = confirm_tree(&host, &snapshot);
    assert!(
        tree.contains("its other worktrees could not be read"),
        "{tree}"
    );
    assert!(
        tree.contains("its 2 worktree directories"),
        "and what goes is counted, not assumed singular: {tree}"
    );
}

#[test]
fn a_state_that_could_not_be_read_asks_rather_than_assume_clean() {
    // `git` is nil for a stat that has not run, a directory that is not a
    // worktree, and a host that could not be reached. v1 folded all three into
    // `DeleteRisk::unknown()` and confirmed.
    let host = host();
    let snapshot = snapshot();
    assert!(snapshot.sessions[0].git.is_none());
    render_in(&host, &snapshot);
    press_in(&host, &snapshot, "D");

    assert!(host.drain_commands().is_empty(), "it must not delete blind");
    assert!(
        confirm_tree(&host, &snapshot).contains("its state could not be read"),
        "and it says why it is asking"
    );
}

/// A creation in flight for the repo the sessions are already in.
///
/// The pane learns about one from `thurbox.commands`: a `create` names no
/// session yet, so it carries the repository as its `subject` and the list
/// draws a placeholder at the end of that group. Matching the group label is
/// the whole point — a `subject` nothing groups under produces no placeholder,
/// and a test with no placeholder pins nothing.
fn creating(repo: &str) -> Vec<InFlight> {
    vec![InFlight {
        id: 1,
        kind: "create",
        session: String::new(),
        subject: Some(repo.to_string()),
        phase: Phase::Running,
        error: None,
    }]
}

#[test]
fn sorting_a_group_that_holds_a_creation_in_flight_does_not_take_the_pane_down() {
    // The placeholder carries no `session`, and the comparator read
    // `a[1].session.name` unconditionally. With two blocks in the group — the
    // common case, since creating into a repo that already holds a session
    // reuses that group — `table.sort` raised, `draw` cleared the PluginError on
    // the next frame, and the observable behaviour was Shift+S doing nothing at
    // all with no message.
    let host = host();
    let snapshot = Snapshot {
        // Reverse alphabetical, so a sort that runs is a sort that is visible.
        sessions: vec![row("bbb", "second"), row("aaa", "first")],
        ..Snapshot::default()
    };
    publish_with(&host, &snapshot, &Default::default(), &creating("thurbox"));
    host.render(index_of(&host, PLUGIN), ctx(46, 14, true))
        .expect("render");

    dispatch(&host, "S").expect("Shift+S must not error the pane");

    // And it really sorted: the placeholder keeps the end of the group, where
    // the row it will become is already drawn, and carries no id of its own.
    assert_eq!(
        host.drain_commands(),
        vec![Command::Order {
            list: vec!["aaa".into(), "bbb".into()],
        }],
        "the sorted order is persisted, without the placeholder in it"
    );
}

/// The confirmation as it is PAINTED, one string per row.
///
/// Mirrors `draw_floats`: the plugin is probed with the whole screen and its
/// tree painted into the float rect the kernel sizes for it. `confirm_tree`
/// above cannot stand in for this — a text node carries its whole string
/// whatever width it was given, so a slot narrower than its own label reads
/// correct in the tree and clipped on the screen.
fn confirm_paint(host: &LuaHost, snapshot: &Snapshot, screen: (u16, u16)) -> Vec<String> {
    publish(host, snapshot);
    let (screen_w, screen_h) = screen;
    let rendered = host
        .render(index_of(host, "confirm"), ctx(screen_w, screen_h, false))
        .expect("render the confirmation");
    let float = rendered
        .float
        .expect("a question that is up floats: the kernel has no other way to place it");
    // `App::float_rect`'s own sizing — cells when the plugin knows them, else a
    // share of the screen, clamped to what there is. Only the size is needed:
    // the tree is painted at the origin here, and centring moves no cell within
    // it.
    let width = float
        .cols
        .unwrap_or((f64::from(screen_w) * float.width_pct / 100.0) as u16)
        .min(screen_w);
    let height = float
        .rows
        .unwrap_or((f64::from(screen_h) * float.height_pct / 100.0) as u16)
        .min(screen_h);
    let mut terminal = Terminal::new(TestBackend::new(width, height)).expect("terminal");
    terminal
        .draw(|frame| paint::render(frame, frame.area(), &rendered.node, &PlaceholderSurfaces))
        .expect("draw");
    let buffer = terminal.backend().buffer().clone();
    (0..height)
        .map(|y| {
            (0..width)
                .map(|x| buffer[(x, y)].symbol())
                .collect::<String>()
        })
        .collect()
}

#[test]
fn both_pills_are_painted_whole() {
    // The pill's slot is a fixed `len`, so a label longer than the number beside
    // it is clipped on the right: `" [ Cancel ]"` is eleven columns and the slot
    // said ten, which cost every confirmation its closing bracket. Invisible in
    // the tree, hence the paint — and worth pinning because `70_new_session`'s
    // footer records having made the identical mistake.
    let host = host();
    let mut snapshot = snapshot();
    snapshot.sessions[0].git = Some(GitState {
        files_changed: 1,
        dirty: true,
        ..clean()
    });
    render_in(&host, &snapshot);
    press_in(&host, &snapshot, "D");

    let painted = confirm_paint(&host, &snapshot, (100, 20));
    // Anchored on the key hints, which share the row with the pills: anchoring
    // on a pill's own label would make a clipped pill look like a missing row.
    let pills = painted
        .iter()
        .find(|row| row.contains("esc cancel"))
        .unwrap_or_else(|| panic!("no pill row was painted: {painted:?}"));
    assert!(
        pills.contains("[ Confirm ]"),
        "the confirm pill lost a bracket: {pills:?}"
    );
    assert!(
        pills.contains("[ Cancel ]"),
        "the cancel pill lost a bracket: {pills:?}"
    );
}
