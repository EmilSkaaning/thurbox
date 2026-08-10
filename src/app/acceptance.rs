//! In-process acceptance ("end-to-end") tests for the thurbox TUI.
//!
//! Where the focused unit tests in [`super::tests`] poke individual methods,
//! these drive a *real* [`App`] the way `main.rs`'s loop does — feeding
//! `update(AppMessage)` events, running the loop's deterministic tick half
//! ([`App::tick_core`] via [`Harness::tick`]; the excluded `tick_background`
//! spawns Tokio tasks that shell out), and rendering `view(Frame)` to a
//! headless ratatui [`TestBackend`]. No TTY, tmux server, or agent process is
//! involved:
//!
//! * sessions are inert [`Session::stub`]s on a no-op [`FakeBackend`],
//! * the database is `Database::open_in_memory()`,
//! * every config/data path is redirected to a throwaway tempdir via
//!   [`crate::paths::TestPathGuard`], so the suite never touches the
//!   developer's real `~/.config/thurbox`,
//! * agent output is injected per session with [`Harness::feed_output`]
//!   (through the same vt100 parser + `TermSignals` path the PTY reader uses),
//! * wall-clock-gated behavior (timeouts, debounces, the redraw floor) is
//!   fast-forwarded with [`Harness::advance`] (see [`clock`]) — never slept.
//!
//! Stable, deterministic screens (the empty welcome state, the keybindings
//! help overlay, the theme picker) are pinned with `insta` snapshots so a UI
//! change surfaces as a reviewable diff (`cargo insta review` /
//! `INSTA_UPDATE=always cargo test`). Flows whose output depends on live
//! metrics or wall-clock time are asserted on `App` *state* instead (modal
//! kind, selection index, panel visibility, quit flag) to stay robust.
//!
//! Finally, [`monkey_random_events_uphold_invariants`] fuzzes the whole
//! surface: thousands of seeded pseudo-random events (keys, chords, mouse,
//! ticks, clock jumps, resizes, injected output) with [`assert_invariants`]
//! checked after every step — the regression net for "weird TUI behavior"
//! that no directed test anticipated.

use std::path::Path;
use std::sync::Arc;

use ratatui::backend::TestBackend;
use ratatui::Terminal;

use super::*;
use crate::agent::AgentProvider;

/// Wide layout (≥120 cols) used by the behavioral tests — exercises the full
/// multi-panel TUI the way a real terminal would.
const STD_COLS: u16 = 120;
const STD_ROWS: u16 = 40;

/// Smaller, sessionless size for the pinned snapshot screens, kept compact so
/// the `.snap` files stay readable.
const SNAP_COLS: u16 = 100;
const SNAP_ROWS: u16 = 30;

/// Initialize a git repo at `dir` with one committed file, leaving an
/// uncommitted edit when `dirty`. Used by the hard-delete tests to give a
/// session a worktree whose state `git::worktree_stats` can read.
fn init_git_repo(dir: &Path, dirty: bool) {
    let git = |args: &[&str]| {
        // `git_program` scrubs inherited `GIT_*` vars so this stays hermetic even
        // when the suite runs under the project's pre-commit hook.
        let ok = crate::git::git_program()
            .args(args)
            .current_dir(dir)
            .output()
            .expect("run git")
            .status
            .success();
        assert!(ok, "git {args:?} failed");
    };
    git(&["init", "-q"]);
    git(&["config", "user.email", "t@example.com"]);
    git(&["config", "user.name", "thurbox-test"]);
    std::fs::write(dir.join("f.txt"), "hello\n").unwrap();
    git(&["add", "."]);
    git(&["commit", "-qm", "init"]);
    if dirty {
        std::fs::write(dir.join("f.txt"), "changed\n").unwrap();
    }
}

/// Backend stand-in for the harness. Inert by default: `spawn`/`adopt` error,
/// so a test proves no accidental spawn while the session still has a real
/// vt100 parser (the session list draws). With `spawnable = true` they succeed,
/// returning an inert EOF reader + sink writer, so the spawn-dependent App flows
/// (restart, shell pane) run for real — those wire Tokio I/O tasks, so such
/// tests must be `#[tokio::test]`.
struct FakeBackend {
    spawnable: bool,
    /// Pushable remote-hook status events, drained by
    /// [`SessionBackend::take_hook_state_events`] — lets a test drive the
    /// remote-session status path without a control-mode connection.
    hook_events: std::sync::Mutex<Vec<(String, String)>>,
}

impl FakeBackend {
    /// Inert: spawning/adopting fails.
    fn stub() -> Self {
        Self {
            spawnable: false,
            hook_events: std::sync::Mutex::new(Vec::new()),
        }
    }

    /// Spawnable: `spawn`/`adopt` succeed with no-op I/O.
    fn spawnable() -> Self {
        Self {
            spawnable: true,
            hook_events: std::sync::Mutex::new(Vec::new()),
        }
    }

    /// Queue a `(pane_id, state)` event for the next drain.
    fn push_hook_event(&self, pane_id: &str, state: &str) {
        self.hook_events
            .lock()
            .unwrap()
            .push((pane_id.to_string(), state.to_string()));
    }
}

impl SessionBackend for FakeBackend {
    fn name(&self) -> &str {
        "fake"
    }
    fn check_available(&self) -> anyhow::Result<()> {
        Ok(())
    }
    fn ensure_ready(&self) -> anyhow::Result<()> {
        Ok(())
    }
    fn spawn(
        &self,
        _: &str,
        _: &str,
        _: &[String],
        _: Option<&Path>,
        _: &std::collections::HashMap<String, String>,
        _: u16,
        _: u16,
    ) -> anyhow::Result<crate::agent::backend::SpawnedSession> {
        anyhow::ensure!(self.spawnable, "inert fake backend does not spawn");
        Ok(crate::agent::backend::SpawnedSession {
            backend_id: "fake:0".into(),
            output: Box::new(std::io::empty()),
            input: Box::new(std::io::sink()),
        })
    }
    fn adopt(
        &self,
        _: &str,
        _: u16,
        _: u16,
        _: Option<Vec<u8>>,
    ) -> anyhow::Result<crate::agent::backend::AdoptedSession> {
        anyhow::ensure!(self.spawnable, "inert fake backend does not adopt");
        Ok(crate::agent::backend::AdoptedSession {
            output: Box::new(std::io::empty()),
            input: Box::new(std::io::sink()),
        })
    }
    fn discover(&self) -> anyhow::Result<Vec<crate::agent::backend::DiscoveredSession>> {
        Ok(vec![])
    }
    fn resize(&self, _: &str, _: u16, _: u16) -> anyhow::Result<()> {
        Ok(())
    }
    fn is_dead(&self, _: &str) -> anyhow::Result<bool> {
        Ok(false)
    }
    fn kill(&self, _: &str) -> anyhow::Result<()> {
        Ok(())
    }
    fn detach(&self, _: &str) -> anyhow::Result<()> {
        Ok(())
    }
    fn pane_pid(&self, _: &str) -> anyhow::Result<Option<u32>> {
        Ok(None)
    }
    fn take_hook_state_events(&self) -> Vec<(String, String)> {
        std::mem::take(&mut self.hook_events.lock().unwrap())
    }
}

/// A driveable TUI under test: a real [`App`] paired with a headless terminal,
/// plus the tempdir + path guard that keep it hermetic for the harness's life.
struct Harness {
    app: App,
    terminal: Terminal<TestBackend>,
    // Held for their `Drop` side effects (restore XDG paths / delete tempdir);
    // ordering matters — the guard resets path resolution before the dir goes.
    _guard: crate::paths::TestPathGuard,
    _tmp: tempfile::TempDir,
}

impl Harness {
    /// Build an `App` of `cols`×`rows` seeded with `session_count` stub
    /// sessions on the inert [`FakeBackend`].
    fn new(cols: u16, rows: u16, session_count: usize) -> Self {
        Self::with_backend(cols, rows, session_count, Arc::new(FakeBackend::stub()))
    }

    /// As [`Harness::new`], but on a caller-supplied backend — the seam that
    /// lets spawn-dependent flows run against a spawnable [`FakeBackend`].
    fn with_backend(
        cols: u16,
        rows: u16,
        session_count: usize,
        backend: Arc<dyn SessionBackend>,
    ) -> Self {
        let tmp = tempfile::tempdir().unwrap();
        let guard = crate::paths::TestPathGuard::new(tmp.path());

        let provider: Arc<dyn AgentProvider> = Arc::new(GenericProvider::new(
            crate::agent::agent_config::builtin_registry()
                .default_agent()
                .unwrap()
                .clone(),
        ));

        let mut app = App::new(
            rows,
            cols,
            BackendRegistry::new(Arc::clone(&backend)),
            crate::agent::agent_config::builtin_registry(),
            Database::open_in_memory().unwrap(),
        );
        for i in 0..session_count {
            app.sessions
                .push(Session::stub(&format!("session-{i}"), &backend, &provider));
        }
        if session_count > 0 {
            app.active_index = 0;
        }

        let terminal = Terminal::new(TestBackend::new(cols, rows)).unwrap();
        Self {
            app,
            terminal,
            _guard: guard,
            _tmp: tmp,
        }
    }

    /// Standard wide harness ([`STD_COLS`]×[`STD_ROWS`]) seeded with
    /// `session_count` stub sessions — the default for behavioral tests.
    fn standard(session_count: usize) -> Self {
        Self::new(STD_COLS, STD_ROWS, session_count)
    }

    /// Snapshot-sized, sessionless harness for the pinned-screen tests.
    fn snapshot() -> Self {
        Self::new(SNAP_COLS, SNAP_ROWS, 0)
    }

    /// Wide harness on a spawnable [`FakeBackend`], with each session given a
    /// resumable `agent_session_id` so spawn-dependent flows (restart) aren't
    /// no-ops. Must be driven from a `#[tokio::test]`: the spawn path wires up
    /// Tokio I/O tasks and needs a runtime.
    fn spawnable(session_count: usize) -> Self {
        let mut h = Self::with_backend(
            STD_COLS,
            STD_ROWS,
            session_count,
            Arc::new(FakeBackend::spawnable()),
        );
        for (i, session) in h.app.sessions.iter_mut().enumerate() {
            session.info.agent_session_id = Some(format!("agent-{i}"));
        }
        h
    }

    /// Point the active session at a freshly-created git repo (clean, or
    /// `dirty` with one uncommitted change) so a `soft_delete`-off delete sees —
    /// or doesn't see — work at risk. Returns the backing `TempDir`, which the
    /// caller must keep alive for the repo to exist on disk.
    fn set_active_git_cwd(&mut self, dirty: bool) -> tempfile::TempDir {
        let repo = tempfile::tempdir().unwrap();
        init_git_repo(repo.path(), dirty);
        let idx = self.app.active_index;
        self.app.sessions[idx].info.cwd = Some(repo.path().to_path_buf());
        repo
    }

    /// Feed one key event, exactly as the real event loop converts a crossterm
    /// `KeyPress` into an [`AppMessage`].
    fn key(&mut self, code: KeyCode, mods: KeyModifiers) -> &mut Self {
        self.app.update(AppMessage::KeyPress(code, mods));
        self
    }

    /// A `Ctrl+<c>` chord (the form most global thurbox bindings take).
    fn ctrl(&mut self, c: char) -> &mut Self {
        self.key(KeyCode::Char(c), KeyModifiers::CONTROL)
    }

    /// A bare function key (`F1`…`F5`).
    fn func(&mut self, n: u8) -> &mut Self {
        self.key(KeyCode::F(n), KeyModifiers::NONE)
    }

    /// A `Shift+<letter>` chord (e.g. session reordering). Terminals deliver
    /// these as an uppercase char; `KeyChord::normalized` canonicalizes the
    /// encoding, so the uppercase-char + SHIFT form resolves the same binding.
    fn shift(&mut self, c: char) -> &mut Self {
        self.key(KeyCode::Char(c.to_ascii_uppercase()), KeyModifiers::SHIFT)
    }

    /// Draw the current state to the headless backend and return the visible
    /// glyphs as newline-separated rows (one string per terminal line), the
    /// shape both `insta` snapshots and substring assertions read.
    fn render(&mut self) -> String {
        let app = &mut self.app;
        self.terminal.draw(|f| app.view(f)).unwrap();
        let buffer = self.terminal.backend().buffer();
        let area = *buffer.area();
        let mut out = String::new();
        for y in 0..area.height {
            let mut line = String::new();
            for x in 0..area.width {
                line.push_str(buffer[(x, y)].symbol());
            }
            // Drop trailing blanks so snapshots aren't a wall of spaces.
            out.push_str(line.trim_end());
            out.push('\n');
        }
        out
    }

    /// Click the open Settings panel's row for `field`. Renders first so this
    /// frame's `ModalField` hitboxes exist, locates the one carrying `field`'s
    /// `ORDER` index, and dispatches a click at its left edge.
    fn click_settings_field(&mut self, field: modals::SettingsField) -> &mut Self {
        self.render();
        let index = modals::SettingsField::ORDER
            .iter()
            .position(|f| *f == field)
            .expect("field in ORDER");
        let rect = self
            .app
            .click_targets
            .iter()
            .find_map(|t| match t.action {
                ClickAction::ModalField(i) if i == index => Some(t.rect),
                _ => None,
            })
            .expect("settings field hitbox recorded");
        self.app.update(AppMessage::MouseClick {
            x: rect.x + 1,
            y: rect.y,
            modifiers: KeyModifiers::NONE,
        });
        self
    }

    /// Run one deterministic tick — the third step of `main.rs`'s loop, minus
    /// its background half ([`App::tick_core`]; the excluded
    /// `tick_background` spawns Tokio tasks that shell out / hit the network).
    /// This drives everything tick-dependent hermetically: status derivation,
    /// timer expiry, the global-search debounce, automation firing, and the
    /// external-change poll.
    fn tick(&mut self) -> &mut Self {
        self.app.tick_core();
        self
    }

    /// Fast-forward the app's clock by `d` (see [`clock`]). Timers, debounces
    /// and retry windows age deterministically — the next [`Self::tick`] (or
    /// `should_redraw` check) observes the elapsed time without real waiting.
    fn advance(&mut self, d: std::time::Duration) -> &mut Self {
        clock::advance(d);
        self
    }

    /// Feed raw agent-output bytes to session `idx`, exactly as its PTY reader
    /// loop would — the seam for testing everything downstream of output:
    /// terminal rendering, the output-change redraw detector, OSC
    /// title/bell/notification signals, and buffer-content search.
    fn feed_output(&mut self, idx: usize, bytes: &[u8]) -> &mut Self {
        self.app.sessions[idx].feed_output_for_test(bytes);
        self
    }

    /// Resize both the app (as the real loop would on a terminal resize) and
    /// the headless backend, so subsequent renders draw at the new size.
    fn resize(&mut self, cols: u16, rows: u16) -> &mut Self {
        self.app.update(AppMessage::Resize(cols, rows));
        self.terminal = Terminal::new(TestBackend::new(cols, rows)).unwrap();
        self
    }

    /// Render, then click the central-pane tab strip's cell for `tab` (returns
    /// false when no such tab was rendered, e.g. its feature is off).
    fn click_central_tab(&mut self, tab: CentralTab) -> bool {
        self.render();
        let rect = self.app.click_targets.iter().find_map(|t| match t.action {
            ClickAction::CentralTab(found) if found == tab => Some(t.rect),
            _ => None,
        });
        let Some(rect) = rect else {
            return false;
        };
        self.app.update(AppMessage::MouseClick {
            x: rect.x + 1,
            y: rect.y,
            modifiers: KeyModifiers::NONE,
        });
        true
    }
}

// ── Snapshot tests: stable, deterministic screens ────────────────────────────

#[test]
fn empty_welcome_screen_renders() {
    let mut h = Harness::snapshot();
    insta::assert_snapshot!(h.render());
}

#[test]
fn help_overlay_lists_keybindings() {
    let mut h = Harness::snapshot();
    h.func(1); // F1 → ToggleHelp
    assert!(
        matches!(h.app.modal, modals::Modal::Help(_)),
        "F1 should open the help modal"
    );
    insta::assert_snapshot!(h.render());
}

#[test]
fn theme_picker_lists_palettes() {
    let mut h = Harness::snapshot();
    h.ctrl('y'); // Ctrl+Y → OpenThemePicker
    assert!(
        matches!(h.app.modal, modals::Modal::ThemePicker(_)),
        "Ctrl+Y should open the theme picker"
    );
    insta::assert_snapshot!(h.render());
}

#[test]
fn theme_picker_filtered_shows_both_sections() {
    // A query spanning dark and light themes: pins the `Dark`/`Light` section
    // headers, the narrowed match count, and the echoed query.
    let mut h = Harness::snapshot();
    h.ctrl('y');
    h.key(KeyCode::Char('/'), KeyModifiers::NONE); // open the filter sub-mode
    for c in "light".chars() {
        h.key(KeyCode::Char(c), KeyModifiers::NONE);
    }
    insta::assert_snapshot!(h.render());
}

#[test]
fn repo_picker_path_browser_snapshot() {
    // The dropdown under the path input: a `●git`-marked repo, a plain dir,
    // and the browser footer hints. Paths are `~`-relative (the guarded test
    // home), so the frame is stable.
    let mut h = Harness::snapshot();
    open_browser_on_home_code(&mut h);
    insta::assert_snapshot!(h.render());
}

// ── Behavioral tests: drive keys, assert on App state ────────────────────────

#[test]
fn ctrl_n_opens_repo_picker() {
    let mut h = Harness::standard(0);
    h.render();
    h.ctrl('n'); // Ctrl+N → NewSession
    assert!(
        matches!(h.app.modal, modals::Modal::RepoPicker(_)),
        "Ctrl+N should open the repo picker (no hosts configured)"
    );
    h.key(KeyCode::Esc, KeyModifiers::NONE);
    assert!(!h.app.modal.is_open(), "Esc should dismiss the modal");
}

/// Create `~/code` under the harness's guarded home with one git child and
/// one plain child, and drive the picker to the path input with `~/code/`
/// typed — the setup shared by the path-browser tests. (Local listings
/// compute inline, so no runtime/tick is needed.)
fn open_browser_on_home_code(h: &mut Harness) {
    let home = crate::paths::home_dir().unwrap();
    std::fs::create_dir_all(home.join("code").join("alpha").join(".git")).unwrap();
    std::fs::create_dir_all(home.join("code").join("notes")).unwrap();

    h.ctrl('n');
    h.key(KeyCode::Tab, KeyModifiers::NONE); // list → path input
    for c in "~/code/".chars() {
        h.key(KeyCode::Char(c), KeyModifiers::NONE);
    }
    // "alpha" and "notes" share no prefix → no ghost suggestion, so Tab
    // opens the browser dropdown instead.
    h.key(KeyCode::Tab, KeyModifiers::NONE);
}

#[test]
fn repo_picker_browser_lists_and_picks_a_git_repo() {
    let mut h = Harness::standard(0);
    open_browser_on_home_code(&mut h);

    let modals::Modal::RepoPicker(ref rp) = h.app.modal else {
        panic!("picker open");
    };
    assert!(rp.browser.open, "Tab with no suggestion opens the browser");
    assert_eq!(rp.browser.dir, "~/code");
    let names: Vec<(&str, bool)> = rp
        .browser
        .filtered
        .iter()
        .map(|&i| {
            (
                rp.browser.listing[i].name.as_str(),
                rp.browser.listing[i].is_git,
            )
        })
        .collect();
    assert_eq!(names, vec![("alpha", true), ("notes", false)]);

    // Enter on the git repo commits it directly: row added + selected +
    // classified, input reset, browser closed.
    h.key(KeyCode::Enter, KeyModifiers::NONE);
    let modals::Modal::RepoPicker(ref rp) = h.app.modal else {
        panic!("picker still open");
    };
    let home = crate::paths::home_dir().unwrap();
    assert_eq!(rp.bookmarks, vec![home.join("code").join("alpha")]);
    assert_eq!(rp.selected, vec![true]);
    assert_eq!(rp.is_git, vec![Some(true)]);
    assert!(!rp.browser.open);
    assert!(rp.path_input.value().is_empty());
}

#[test]
fn repo_picker_browser_descends_into_plain_dirs() {
    let mut h = Harness::standard(0);
    open_browser_on_home_code(&mut h);

    // Down to "notes" (a plain dir), Enter descends instead of committing.
    h.key(KeyCode::Down, KeyModifiers::NONE);
    h.key(KeyCode::Enter, KeyModifiers::NONE);

    let modals::Modal::RepoPicker(ref rp) = h.app.modal else {
        panic!("picker open");
    };
    assert_eq!(rp.path_input.value(), "~/code/notes/");
    assert!(rp.browser.open, "still browsing after the descent");
    assert_eq!(rp.browser.dir, "~/code/notes");
    assert!(
        rp.browser.filtered.is_empty(),
        "notes has no subdirectories"
    );
    assert!(rp.bookmarks.is_empty(), "descending never commits");
}

#[test]
fn repo_picker_browser_filters_live_and_esc_layers() {
    let mut h = Harness::standard(0);
    open_browser_on_home_code(&mut h);

    // Typing narrows the open dropdown by prefix without a refetch.
    for c in "al".chars() {
        h.key(KeyCode::Char(c), KeyModifiers::NONE);
    }
    let modals::Modal::RepoPicker(ref rp) = h.app.modal else {
        panic!("picker open");
    };
    assert_eq!(
        rp.browser.filtered.len(),
        1,
        "prefix `al` leaves only alpha"
    );

    // Esc closes just the dropdown; the modal (and typed path) survive.
    h.key(KeyCode::Esc, KeyModifiers::NONE);
    let modals::Modal::RepoPicker(ref rp) = h.app.modal else {
        panic!("first Esc must not close the modal");
    };
    assert!(!rp.browser.open);
    assert_eq!(rp.path_input.value(), "~/code/al");

    // The second Esc closes the picker itself.
    h.key(KeyCode::Esc, KeyModifiers::NONE);
    assert!(!h.app.modal.is_open());
}

#[test]
fn ctrl_j_and_k_cycle_session_selection() {
    let mut h = Harness::standard(3);
    assert_eq!(h.app.active_index, 0);

    h.ctrl('j'); // NextSession
    assert_eq!(h.app.active_index, 1, "Ctrl+J moves to the next session");
    h.ctrl('j');
    assert_eq!(h.app.active_index, 2);

    h.ctrl('k'); // PreviousSession
    assert_eq!(h.app.active_index, 1, "Ctrl+K moves back up");
}

#[test]
fn ctrl_w_toggles_tasks_panel() {
    let mut h = Harness::standard(0);
    assert!(!h.app.show_tasks_panel);

    h.ctrl('w'); // FocusTasks
    assert!(h.app.show_tasks_panel, "Ctrl+W reveals the tasks panel");
    h.ctrl('w');
    assert!(!h.app.show_tasks_panel, "Ctrl+W again hides it");
}

#[test]
fn f5_toggles_tasks_panel_like_ctrl_w() {
    // F5 is the documented alternate chord for FocusTasks (Ctrl+W); both must
    // drive the same toggle.
    let mut h = Harness::standard(0);
    assert!(!h.app.show_tasks_panel);

    h.func(5);
    assert!(h.app.show_tasks_panel, "F5 reveals the tasks panel");
    h.func(5);
    assert!(!h.app.show_tasks_panel, "F5 again hides it");
}

#[test]
fn f9_toggles_session_list_pane() {
    use ratatui::layout::Rect;
    let screen = Rect::new(0, 0, STD_COLS, STD_ROWS);
    let mut h = Harness::standard(1);
    assert!(h.app.show_session_list, "list visible by default");
    assert_eq!(h.app.focus, InputFocus::SessionList);
    assert!(h.app.layout_for(screen).left_panel.is_some());
    let shown_terminal_width = h.app.layout_for(screen).terminal.width;

    // F9 hides the list. Focus can't rest on the now-hidden column, so it
    // retreats to the terminal, and the left column disappears from the layout.
    h.func(9);
    assert!(!h.app.show_session_list, "F9 hides the list");
    assert_eq!(
        h.app.focus,
        InputFocus::Terminal,
        "focus retreats off the hidden list onto the terminal"
    );
    let hidden = h.app.layout_for(screen);
    assert!(hidden.left_panel.is_none(), "no left column while hidden");
    assert!(
        hidden.automations_panel.is_none(),
        "automations pane shares the column"
    );
    // The terminal reclaims the width the list would have reserved.
    assert!(
        hidden.terminal.width > shown_terminal_width,
        "terminal widens when the list is hidden"
    );

    // The rendered screen drops the `Sessions` panel border.
    let shown_screen = {
        let mut g = Harness::standard(1);
        g.render()
    };
    let hidden_screen = h.render();
    let sessions_border = shown_screen
        .lines()
        .find(|row| row.contains("Sessions"))
        .expect("the ` Sessions ` panel renders when shown");
    assert!(
        !hidden_screen.contains(sessions_border.trim()),
        "the Sessions panel border is gone while hidden"
    );

    // The focus ring skips the hidden list: from the terminal, Ctrl+L wraps
    // back to the terminal (no SessionList stop) with no side panels shown.
    h.ctrl('l'); // FocusForward
    assert_eq!(
        h.app.focus,
        InputFocus::Terminal,
        "Ctrl+L stays on the terminal — the ring has no SessionList stop while hidden"
    );

    // F9 again reveals the list. Focus stays put (showing is a visibility
    // toggle, not an interaction switch — it does not grab focus).
    h.func(9);
    assert!(h.app.show_session_list, "F9 reveals the list again");
    assert_eq!(
        h.app.focus,
        InputFocus::Terminal,
        "revealing the list does not steal focus from the terminal"
    );
    assert!(h.app.layout_for(screen).left_panel.is_some());
}

#[test]
fn f9_toggles_session_list_from_focused_terminal() {
    // F9 has no Ctrl+<letter> primary, so it is not a terminal-passthrough
    // chord — it toggles the list even while the agent terminal is focused.
    let mut h = Harness::standard(1);
    h.ctrl('l'); // focus the terminal
    assert_eq!(h.app.focus, InputFocus::Terminal);
    assert!(h.app.show_session_list);

    h.func(9);
    assert!(
        !h.app.show_session_list,
        "F9 hides the list from the terminal"
    );
    // Focus was already on the terminal; hiding the list leaves it there.
    assert_eq!(h.app.focus, InputFocus::Terminal);
}

#[test]
fn session_collapse_chevron_renders_and_toggles() {
    // A lightweight collapse/expand chevron on the central pane's top-left border
    // toggles the session list. It shows the live F9 hint and flips its arrow
    // direction with the list's visibility (◀ collapse ↔ ▶ expand). Clicking it
    // shares the F9 path (ClickAction::Global(ToggleSessionList)).
    let mut h = Harness::standard(1);
    let shown = h.render();
    // At the standard 120-col width the F9 hint is shown beside the chevron.
    assert!(
        shown.contains("◀") && shown.contains("F9"),
        "collapse chevron ◀ + F9 hint renders while the list is shown: {shown:?}"
    );
    let chevron_rect = |app: &super::App| {
        app.click_targets.iter().find_map(|t| match t.action {
            ClickAction::Global(crate::session::Action::ToggleSessionList) => Some(t.rect),
            _ => None,
        })
    };
    let rect = chevron_rect(&h.app).expect("chevron is a recorded click target when shown");

    // Clicking the chevron collapses the list (same effect as pressing F9).
    h.app.update(AppMessage::MouseClick {
        x: rect.x + 1,
        y: rect.y,
        modifiers: KeyModifiers::NONE,
    });
    assert!(!h.app.show_session_list, "clicking ◀ collapses the list");

    // While hidden the chevron flips to ▶ (expand) and stays clickable — the only
    // affordance left to bring the list back.
    let hidden = h.render();
    assert!(
        hidden.contains("▶"),
        "chevron flips to ▶ (expand) while hidden: {hidden:?}"
    );
    let rect = chevron_rect(&h.app).expect("chevron still recorded while hidden");
    h.app.update(AppMessage::MouseClick {
        x: rect.x + 1,
        y: rect.y,
        modifiers: KeyModifiers::NONE,
    });
    assert!(h.app.show_session_list, "clicking ▶ re-expands the list");
}

/// Leftmost recorded click-target rect whose action satisfies `pred` — the
/// on-border chevron/tab lookups both want the first hitbox left-to-right.
/// Panics when nothing matches, so a test that stops rendering its target
/// fails loudly instead of silently asserting nothing.
fn first_target_rect(app: &super::App, pred: impl Fn(&ClickAction) -> bool) -> Rect {
    app.click_targets
        .iter()
        .filter(|t| pred(&t.action))
        .map(|t| t.rect)
        .min_by_key(|r| r.x)
        .expect("click target recorded this frame")
}

/// The two on-border central-pane hitboxes the collapse-chevron tests compare:
/// the chevron itself and the leftmost view tab (Agent).
fn chevron_and_first_tab_rects(app: &super::App) -> (Rect, Rect) {
    let chevron = first_target_rect(app, |a| {
        matches!(
            a,
            ClickAction::Global(crate::session::Action::ToggleSessionList)
        )
    });
    let tab = first_target_rect(app, |a| matches!(a, ClickAction::CentralTab(_)));
    (chevron, tab)
}

#[test]
fn session_collapse_chevron_keeps_a_gap_before_the_tab_strip() {
    // The chevron is packed left of the tab strip, but it must keep the same
    // one-cell gap the pills keep between themselves: their hover fills are
    // adjacent rects, so a flush chevron/Agent pair lights up as one chip.
    let mut h = Harness::standard(1);
    h.render();
    let (chevron, first_tab) = chevron_and_first_tab_rects(&h.app);
    assert_eq!(
        first_tab.x,
        chevron.right() + 1,
        "one blank border cell separates the chevron from the first pill \
         (chevron {chevron:?}, first tab {first_tab:?})"
    );
}

#[test]
fn session_collapse_chevron_hovers_lighter_than_a_pill() {
    // The chevron is a bare border glyph, not a filled pill, so hovering it must
    // use the subtle row band (`selection_bg`) rather than the pill's bright
    // `accent_bright` fill — otherwise hover dresses it up as the peer view-tab
    // it deliberately isn't. Contrast it against a real pill (the Agent tab),
    // which keeps the bright treatment.
    let mut h = Harness::standard(1);
    h.render();
    let (chevron, pill) = chevron_and_first_tab_rects(&h.app);

    // Background the hovered cell ends up with, for the target under `rect`.
    let hovered_bg = |h: &mut Harness, rect: Rect| {
        h.app.update(AppMessage::MouseMove {
            x: rect.x,
            y: rect.y,
        });
        h.render();
        h.terminal.backend().buffer()[(rect.x, rect.y)].bg
    };

    // Hovering the pill brightens it; hovering the chevron only bands it.
    assert_eq!(
        hovered_bg(&mut h, pill),
        crate::ui::theme::Theme::accent_bright(),
        "a real tab pill keeps the bright button hover"
    );
    assert_eq!(
        hovered_bg(&mut h, chevron),
        crate::ui::theme::Theme::selection_bg(),
        "the collapse chevron gets the subtle row band instead"
    );
}

#[test]
fn ctrl_slash_opens_global_search_strip() {
    let mut h = Harness::standard(2);
    assert!(!h.app.global_search.active);

    h.ctrl('/'); // GlobalSearch
    assert!(h.app.global_search.active, "Ctrl+/ opens the search strip");

    // The strip captures typing before global keybindings, so a plain letter
    // edits the query rather than triggering a binding.
    h.key(KeyCode::Char('s'), KeyModifiers::NONE);
    assert_eq!(h.app.global_search.query.value(), "s");

    // Esc restores the prior state.
    h.key(KeyCode::Esc, KeyModifiers::NONE);
    assert!(!h.app.global_search.active, "Esc closes the search strip");
}

#[test]
fn settings_panel_opens_and_closes() {
    let mut h = Harness::standard(1);
    h.ctrl(','); // OpenSettings
    assert!(
        matches!(h.app.modal, modals::Modal::Settings(_)),
        "Ctrl+, should open the settings panel"
    );

    // The panel shows section headers, the selected field's description, and
    // the restart marker on restart-required rows.
    let screen = h.render();
    assert!(screen.contains("FEATURES"), "section header renders");
    assert!(
        screen.contains("Tasks panel"),
        "selected field's description renders in the footer"
    );
    assert!(screen.contains('⟳'), "restart marker renders");

    h.key(KeyCode::Esc, KeyModifiers::NONE);
    assert!(!h.app.modal.is_open(), "Esc closes the settings panel");
}

#[test]
fn settings_panel_live_toggle_applies_on_save() {
    let mut h = Harness::standard(1);
    assert!(h.app.features.tasks, "tasks default on");

    h.ctrl(','); // OpenSettings — starts on the `tasks` field
    h.key(KeyCode::Char(' '), KeyModifiers::NONE); // toggle tasks off in the draft
    assert!(h.app.features.tasks, "draft edits don't apply until save");

    h.ctrl('s'); // Save
    assert!(!h.app.modal.is_open(), "save closes the panel");
    assert!(
        !h.app.features.tasks,
        "a live feature flag applies immediately on save"
    );
}

#[test]
fn settings_panel_click_toggles_boolean_field() {
    let mut h = Harness::standard(1);
    assert!(h.app.features.mouse, "mouse on by default");
    assert!(h.app.features.info_panel, "info_panel default on");

    h.ctrl(','); // OpenSettings — opens on the `tasks` field
                 // Click a *different* field than the one focused on open, so the click must
                 // both select the row and toggle its boolean.
    h.click_settings_field(modals::SettingsField::FeatInfoPanel);

    let modals::Modal::Settings(s) = &h.app.modal else {
        panic!("settings panel still open after the click");
    };
    assert_eq!(
        s.field,
        modals::SettingsField::FeatInfoPanel,
        "the click selected the clicked row"
    );
    assert!(
        !s.draft.features.info_panel,
        "the click also toggled the boolean off in the draft"
    );
    assert!(
        h.app.features.info_panel,
        "draft edits don't apply until save"
    );
}

#[test]
fn settings_panel_click_does_not_change_scalar() {
    let mut h = Harness::standard(1);
    h.ctrl(','); // OpenSettings
    h.render();
    let before = match &h.app.modal {
        modals::Modal::Settings(s) => s.draft.scrollback_lines,
        _ => unreachable!(),
    };

    h.click_settings_field(modals::SettingsField::ScrollbackLines);

    let modals::Modal::Settings(s) = &h.app.modal else {
        panic!("settings panel still open");
    };
    assert_eq!(
        s.field,
        modals::SettingsField::ScrollbackLines,
        "the click selected the scalar row"
    );
    assert_eq!(
        s.draft.scrollback_lines, before,
        "a click never steps a scalar value — only selects it"
    );
}

#[test]
fn settings_panel_esc_discards() {
    let mut h = Harness::standard(1);
    assert!(h.app.features.tasks);

    h.ctrl(','); // OpenSettings
    h.key(KeyCode::Char(' '), KeyModifiers::NONE); // toggle in the draft
    h.key(KeyCode::Esc, KeyModifiers::NONE); // discard

    assert!(
        h.app.features.tasks,
        "Esc discards the draft — no live preview applied"
    );
}

#[test]
fn ctrl_q_requests_quit() {
    let mut h = Harness::standard(1);
    assert!(!h.app.should_quit());
    h.ctrl('q'); // QuitApp
    assert!(h.app.should_quit(), "Ctrl+Q should request shutdown");
}

#[test]
fn session_list_renders_seeded_sessions() {
    // Not a snapshot (status dots/metrics drift); assert the names appear.
    let mut h = Harness::standard(2);
    let frame = h.render();
    assert!(
        frame.contains("session-0"),
        "first session name should render"
    );
    assert!(
        frame.contains("session-1"),
        "second session name should render"
    );
}

// ── Side panels: file viewer, info panel ─────────────────────────────────────

#[test]
fn file_viewer_toggles_via_f3_and_ctrl_e() {
    // F3 and Ctrl+E are the two default chords for ToggleFileViewer.
    let mut h = Harness::standard(1);
    assert!(!h.app.show_file_viewer);

    h.func(3);
    assert!(h.app.show_file_viewer, "F3 reveals the file viewer");
    h.func(3);
    assert!(!h.app.show_file_viewer, "F3 again hides it");

    h.ctrl('e');
    assert!(
        h.app.show_file_viewer,
        "Ctrl+E also reveals the file viewer"
    );
    h.ctrl('e');
    assert!(!h.app.show_file_viewer, "Ctrl+E again hides it");
}

#[test]
fn info_panel_toggles_via_f2_and_ctrl_b() {
    let mut h = Harness::standard(1);
    let initial = h.app.show_info_panel;

    h.func(2);
    assert_ne!(h.app.show_info_panel, initial, "F2 toggles the info panel");
    h.ctrl('b');
    assert_eq!(
        h.app.show_info_panel, initial,
        "Ctrl+B toggles it back (same action, alternate chord)"
    );
}

// ── Modals: automations list, restore deleted sessions ───────────────────────

#[test]
fn automations_list_modal_empty() {
    let mut h = Harness::snapshot();
    h.ctrl('p'); // Ctrl+P → OpenAutomations
    assert!(
        matches!(h.app.modal, modals::Modal::AutomationsList(_)),
        "Ctrl+P opens the automations list modal"
    );
    insta::assert_snapshot!(h.render());
}

#[test]
fn restore_sessions_modal_empty() {
    let mut h = Harness::snapshot();
    h.ctrl('u'); // Ctrl+U → OpenRestoreSessions
    assert!(
        matches!(h.app.modal, modals::Modal::RestoreSessions(_)),
        "Ctrl+U opens the restore-deleted-sessions modal"
    );
    insta::assert_snapshot!(h.render());
}

#[test]
fn force_deleted_restore_confirms_then_best_effort_restores() {
    let mut h = Harness::standard(1);

    // Persist the stub session, then force-delete it — the soft-deleted +
    // force-deleted DB row a best-effort recovery acts on (no worktrees → no
    // on-disk teardown needed).
    let id = h.app.sessions[0].info.id;
    let shared = h.app.session_to_shared(&h.app.sessions[0]);
    h.app.db.upsert_session(&shared).unwrap();
    crate::session_ops::delete_session_headless(&h.app.db, id, true).unwrap();
    assert!(
        h.app.db.get_deleted_session_by_id(id).unwrap().is_some(),
        "row is soft-deleted + force-deleted"
    );

    // Ctrl+U lists it; Enter on a force-deleted row opens the confirm prompt
    // rather than restoring immediately.
    h.ctrl('u');
    assert!(matches!(h.app.modal, modals::Modal::RestoreSessions(_)));
    h.key(KeyCode::Enter, KeyModifiers::NONE);
    assert!(
        matches!(h.app.modal, modals::Modal::ConfirmRestore(_)),
        "Enter on a force-deleted row asks for confirmation"
    );
    assert!(
        h.app.db.get_deleted_session_by_id(id).unwrap().is_some(),
        "nothing restored before confirmation"
    );

    // Confirm → `restore_session` clears `deleted_at` + `force_deleted`, so the
    // row leaves the deleted list and is an active session again.
    h.key(KeyCode::Enter, KeyModifiers::NONE);
    assert!(!h.app.modal.is_open(), "confirm closes the prompt");
    assert!(
        h.app.db.get_deleted_session_by_id(id).unwrap().is_none(),
        "the session is no longer in the deleted list"
    );
    assert!(
        h.app.db.get_session_by_id(id).unwrap().is_some(),
        "the row is an active session again"
    );
}

// ── Delete + undo ────────────────────────────────────────────────────────────

#[test]
fn ctrl_d_soft_deletes_and_ctrl_z_undoes() {
    let mut h = Harness::standard(2);
    assert_eq!(h.app.sessions.len(), 2);

    h.ctrl('d'); // DeleteSession (soft, with a 10s undo window)
    assert_eq!(
        h.app.sessions.len(),
        1,
        "delete removes the session from the list"
    );
    assert!(
        h.app.pending_delete.is_some(),
        "a pending delete is held for undo"
    );

    h.ctrl('z'); // UndoDelete
    assert_eq!(h.app.sessions.len(), 2, "undo restores the session");
    assert!(
        h.app.pending_delete.is_none(),
        "the undo consumes the pending delete"
    );
}

#[test]
fn ctrl_d_hard_delete_confirms_when_soft_delete_disabled() {
    let mut h = Harness::standard(2);
    h.app.features.soft_delete = false;
    // The active session has uncommitted work, so a hard delete must confirm.
    let _repo = h.set_active_git_cwd(true);

    // Ctrl+D now opens a confirmation prompt instead of deleting immediately.
    h.ctrl('d');
    assert!(
        matches!(h.app.modal, modals::Modal::ConfirmDelete(_)),
        "Ctrl+D opens the hard-delete confirmation when soft_delete is off"
    );
    assert_eq!(
        h.app.sessions.len(),
        2,
        "nothing is deleted before confirmation"
    );

    // Esc cancels, leaving the session untouched.
    h.key(KeyCode::Esc, KeyModifiers::NONE);
    assert!(!h.app.modal.is_open(), "Esc closes the confirmation");
    assert_eq!(h.app.sessions.len(), 2, "cancel leaves the session intact");

    // Re-open and confirm with Enter → the session is torn down, no undo.
    h.ctrl('d');
    h.key(KeyCode::Enter, KeyModifiers::NONE);
    assert!(!h.app.modal.is_open(), "confirm closes the confirmation");
    assert_eq!(h.app.sessions.len(), 1, "confirm removes the session");
    assert!(
        h.app.pending_delete.is_none(),
        "a hard delete offers no Ctrl+Z undo"
    );
}

#[test]
fn hard_delete_confirmation_accepts_y_and_n_keys() {
    let mut h = Harness::standard(2);
    h.app.features.soft_delete = false;
    let _repo = h.set_active_git_cwd(true);

    // 'n' cancels, like Esc.
    h.ctrl('d');
    h.key(KeyCode::Char('n'), KeyModifiers::NONE);
    assert!(!h.app.modal.is_open(), "'n' closes the confirmation");
    assert_eq!(h.app.sessions.len(), 2, "'n' cancels the delete");

    // 'y' confirms, like Enter.
    h.ctrl('d');
    h.key(KeyCode::Char('y'), KeyModifiers::NONE);
    assert!(!h.app.modal.is_open(), "'y' closes the confirmation");
    assert_eq!(h.app.sessions.len(), 1, "'y' confirms the delete");
}

#[test]
fn ctrl_d_hard_deletes_clean_session_without_confirmation() {
    let mut h = Harness::standard(2);
    h.app.features.soft_delete = false;
    // A clean git worktree has no work at risk → delete straight away.
    let _repo = h.set_active_git_cwd(false);

    h.ctrl('d');
    assert!(
        !h.app.modal.is_open(),
        "a clean session is hard-deleted without a confirmation prompt"
    );
    assert_eq!(h.app.sessions.len(), 1, "the clean session is removed");
    assert!(
        h.app.pending_delete.is_none(),
        "a hard delete offers no Ctrl+Z undo"
    );
}

#[test]
fn ctrl_d_confirms_dirty_session_and_lists_risk() {
    let mut h = Harness::standard(2);
    h.app.features.soft_delete = false;
    let _repo = h.set_active_git_cwd(true);

    h.ctrl('d');
    let modals::Modal::ConfirmDelete(ref cd) = h.app.modal else {
        panic!("a dirty session opens the hard-delete confirmation");
    };
    assert!(
        cd.risk.dirty && cd.risk.files_changed > 0,
        "the risk reflects the uncommitted change: {:?}",
        cd.risk
    );
    assert!(!cd.risk.unknown, "a local git worktree is inspectable");
}

// ── Pane focus cycling ───────────────────────────────────────────────────────

#[test]
fn focus_cycles_between_session_list_and_terminal() {
    // With no side panels shown, the session ring is [SessionList, Terminal].
    let mut h = Harness::standard(1);
    assert!(
        matches!(h.app.focus, InputFocus::SessionList),
        "focus starts on the session list"
    );

    h.ctrl('l'); // FocusForward
    assert!(
        matches!(h.app.focus, InputFocus::Terminal),
        "Ctrl+L moves to the terminal"
    );
    h.ctrl('l');
    assert!(
        matches!(h.app.focus, InputFocus::SessionList),
        "Ctrl+L wraps back to the session list"
    );
    h.ctrl('h'); // FocusBackward
    assert!(
        matches!(h.app.focus, InputFocus::Terminal),
        "Ctrl+H steps backward to the terminal"
    );
}

#[test]
fn focus_ring_includes_file_viewer_when_shown() {
    let mut h = Harness::standard(1);
    h.func(3); // show the file viewer
    assert!(h.app.show_file_viewer);

    // Cycling forward from the session list must reach the file viewer.
    let mut saw_file_viewer = false;
    for _ in 0..4 {
        h.ctrl('l');
        if matches!(h.app.focus, InputFocus::FileViewer) {
            saw_file_viewer = true;
            break;
        }
    }
    assert!(
        saw_file_viewer,
        "the focus ring visits the file viewer while it is shown"
    );
}

// ── Code review: focusable changed-files pane ────────────────────────────────

/// Open a synthetic review with `n` files on the active session and focus the
/// diff pane, without needing a real git worktree.
fn open_review(h: &mut Harness, n: usize) {
    let sid = h.app.active_session_id().unwrap();
    h.app
        .code_reviews
        .insert(sid, super::code_review::CodeReviewState::for_test(sid, n));
    h.app.focus = InputFocus::CodeReview;
}

#[test]
fn review_files_pane_joins_focus_ring_and_replaces_file_viewer() {
    let mut h = Harness::standard(1);
    h.func(3); // show the file viewer too — the review must still take the column
    open_review(&mut h, 3);

    // Cycling forward from the diff reaches the changed-files pane, never the
    // plain file viewer while a review owns the column.
    let mut saw_review_files = false;
    for _ in 0..4 {
        h.ctrl('l');
        assert!(
            !matches!(h.app.focus, InputFocus::FileViewer),
            "the file viewer is not a ring stop while a review is open"
        );
        if matches!(h.app.focus, InputFocus::ReviewFiles) {
            saw_review_files = true;
            break;
        }
    }
    assert!(
        saw_review_files,
        "the focus ring visits the changed-files pane"
    );
}

#[test]
fn review_files_pane_navigates_and_opens_into_diff() {
    let mut h = Harness::standard(1);
    open_review(&mut h, 3);
    h.app.focus = InputFocus::ReviewFiles;

    // The diff starts on the first file.
    assert_eq!(h.app.active_review().unwrap().current_file(), Some(0));

    // `j` walks to the next file (the diff follows).
    h.key(KeyCode::Char('j'), KeyModifiers::NONE);
    assert_eq!(h.app.active_review().unwrap().current_file(), Some(1));
    h.key(KeyCode::Char('k'), KeyModifiers::NONE);
    assert_eq!(h.app.active_review().unwrap().current_file(), Some(0));

    // `G` jumps to the last file.
    h.key(KeyCode::Char('G'), KeyModifiers::SHIFT);
    assert_eq!(h.app.active_review().unwrap().current_file(), Some(2));

    // `r` marks the current file reviewed.
    h.key(KeyCode::Char('r'), KeyModifiers::NONE);
    assert!(!h.app.active_review().unwrap().reviewed_files.is_empty());

    // `Enter` drops focus into the diff at the selected file.
    h.key(KeyCode::Enter, KeyModifiers::NONE);
    assert!(matches!(h.app.focus, InputFocus::CodeReview));
}

#[test]
fn review_jump_to_file_anchors_header_to_top() {
    let mut h = Harness::standard(1);
    open_review(&mut h, 5);

    // Jumping to a file below the current window must scroll its header to the
    // top of the viewport, not leave it pinned to the bottom line (the renderer
    // only clamps the *upper* edge). Regression: clicking a changed-files row
    // landed the file at the last visible row.
    h.app.cr_jump_to_file(3);
    let cr = h.app.active_review().unwrap();
    assert_eq!(cr.current_file(), Some(3));
    assert_eq!(
        cr.scroll, cr.selected,
        "the jumped-to file header sits at the top of the viewport"
    );
}

#[test]
fn review_files_pane_demoted_to_terminal_when_review_closes() {
    let mut h = Harness::standard(1);
    open_review(&mut h, 2);
    h.app.focus = InputFocus::ReviewFiles;

    // Esc from the changed-files pane closes the review and drops focus back to
    // the terminal (no review owns the central pane anymore).
    h.key(KeyCode::Esc, KeyModifiers::NONE);
    assert!(h.app.active_review().is_none());
    assert!(matches!(h.app.focus, InputFocus::Terminal));
}

// ── Manual session ordering ──────────────────────────────────────────────────

#[test]
fn shift_j_reorders_sessions() {
    let mut h = Harness::standard(2);
    let before = h.app.render_order_indices();
    assert_eq!(
        before,
        vec![0, 1],
        "initial render order is insertion order"
    );

    h.shift('j'); // SessionListMoveDown — move the selected (first) row down
    let after = h.app.render_order_indices();
    assert_eq!(
        after,
        vec![1, 0],
        "Shift+J swaps the first session past the second"
    );

    h.shift('k'); // SessionListMoveUp — move it back
    assert_eq!(
        h.app.render_order_indices(),
        vec![0, 1],
        "Shift+K restores the original order"
    );
}

// ── Tasks: panel focus + new-task editor ─────────────────────────────────────

#[test]
fn tasks_panel_new_task_opens_editor() {
    let mut h = Harness::standard(0);
    h.ctrl('w'); // FocusTasks → panel shown and focused
    assert!(h.app.show_tasks_panel);
    assert!(matches!(h.app.focus, InputFocus::TaskList));

    h.key(KeyCode::Char('n'), KeyModifiers::NONE); // new task
    assert!(
        matches!(h.app.focus, InputFocus::TaskEditor),
        "'n' opens the central-pane task editor"
    );
    assert!(
        h.app.task_ui.task_editor.is_some(),
        "a fresh task editor is in flight"
    );
}

// ── Fork ─────────────────────────────────────────────────────────────────────

#[test]
fn ctrl_f_fork_opens_session_name_prompt() {
    // Fork pre-fills the session-name modal with "<name>-fork" before spawning,
    // so it is observable without a real backend.
    let mut h = Harness::standard(1);
    h.ctrl('f'); // ForkSession
    assert!(
        matches!(h.app.modal, modals::Modal::SessionName(_)),
        "Ctrl+F opens the session-name prompt for the fork"
    );
}

// ── Help editor: capture mode ────────────────────────────────────────────────

#[test]
fn help_editor_enters_capture_mode() {
    let mut h = Harness::standard(0);
    h.func(1); // F1 → help
    match h.app.modal {
        modals::Modal::Help(ref help) => assert!(!help.capturing, "starts in navigation mode"),
        ref other => panic!("expected help modal, got {other:?}"),
    }

    h.key(KeyCode::Enter, KeyModifiers::NONE); // begin capturing a new chord
    match h.app.modal {
        modals::Modal::Help(ref help) => {
            assert!(
                help.capturing,
                "Enter starts capture mode for the selected action"
            )
        }
        ref other => panic!("expected help modal, got {other:?}"),
    }
}

// ── Behavioral effects: assert the action actually changed state ──────────────

#[test]
fn theme_picker_selection_applies_and_persists() {
    let mut h = Harness::standard(0);
    let entries = crate::ui::theme::all_theme_entries();
    let default_name = h.app.active_theme.name.clone();

    h.ctrl('y'); // open the picker (opens on the active theme, index 0)
    h.key(KeyCode::Char('j'), KeyModifiers::NONE); // move to the next palette
    h.key(KeyCode::Enter, KeyModifiers::NONE); // confirm

    assert!(!h.app.modal.is_open(), "confirming closes the picker");
    assert_eq!(
        h.app.active_theme.name, entries[1].name,
        "the second palette becomes active"
    );
    assert_ne!(
        h.app.active_theme.name, default_name,
        "the theme actually changed"
    );
    assert_eq!(
        h.app.db.get_active_theme().ok().flatten().as_deref(),
        Some(entries[1].name.as_str()),
        "the choice is persisted to the database"
    );
}

#[test]
fn theme_picker_cancel_restores_previewed_theme() {
    // The picker live-previews by mutating the global palette as the selection
    // moves; cancelling (`Esc`) must undo that preview, leaving the original
    // theme active and unpersisted.
    let mut h = Harness::standard(0);
    let entries = crate::ui::theme::all_theme_entries();
    let original_name = h.app.active_theme.name.clone();
    let original_palette = crate::ui::theme::current();

    h.ctrl('y'); // open the picker (opens on the active theme, index 0)
    h.key(KeyCode::Char('j'), KeyModifiers::NONE); // preview the next palette
    assert_eq!(
        crate::ui::theme::current(),
        entries[1].palette,
        "navigating previews the highlighted palette globally"
    );

    h.key(KeyCode::Esc, KeyModifiers::NONE); // cancel

    assert!(!h.app.modal.is_open(), "Esc closes the picker");
    assert_eq!(
        crate::ui::theme::current(),
        original_palette,
        "cancelling restores the palette active when the picker opened"
    );
    assert_eq!(
        h.app.active_theme.name, original_name,
        "the active theme is unchanged after cancel"
    );
    assert_eq!(
        h.app.db.get_active_theme().ok().flatten(),
        None,
        "cancelling persists nothing to the database"
    );
}

#[test]
fn help_editor_capture_rebinds_the_selected_action() {
    // The help editor opens with the first rebindable action selected; capturing
    // a fresh chord must reassign exactly that action.
    let action = crate::session::Action::rebindable_in_order()[0];
    let new_chord = crate::session::KeyChord::ctrl('x');

    let mut h = Harness::standard(0);
    h.func(1); // F1 → help
    h.key(KeyCode::Enter, KeyModifiers::NONE); // begin capture
    h.ctrl('x'); // the captured chord

    assert_eq!(
        h.app.keybindings.chord_for(action),
        Some(&new_chord),
        "the selected action is rebound to the captured chord"
    );
    match h.app.modal {
        modals::Modal::Help(ref help) => {
            assert!(!help.capturing, "capture ends after one chord")
        }
        ref other => panic!("expected help modal, got {other:?}"),
    }
}

#[test]
fn task_editor_creates_task_and_space_cycles_status() {
    let mut h = Harness::standard(0);
    h.ctrl('w'); // focus the tasks panel
    h.key(KeyCode::Char('n'), KeyModifiers::NONE); // new-task editor

    for ch in "Demo task".chars() {
        h.key(KeyCode::Char(ch), KeyModifiers::NONE);
    }
    h.ctrl('s'); // save from any field

    assert!(
        matches!(h.app.focus, InputFocus::TaskList),
        "saving returns to the panel"
    );
    assert_eq!(h.app.task_ui.cached_tasks.len(), 1, "the task is persisted");
    let task = &h.app.task_ui.cached_tasks[0];
    assert_eq!(task.title, "Demo task");
    assert_eq!(
        task.status,
        crate::session::TaskStatus::Todo,
        "new tasks start as Todo"
    );

    h.key(KeyCode::Char(' '), KeyModifiers::NONE); // cycle status
    assert_eq!(
        h.app.task_ui.cached_tasks[0].status,
        crate::session::TaskStatus::InProgress,
        "Space advances Todo → InProgress"
    );
}

#[test]
fn global_search_returns_results_for_a_session_query() {
    let mut h = Harness::standard(2); // session-0, session-1
    h.ctrl('/'); // open the search strip
    for ch in "session-1".chars() {
        h.key(KeyCode::Char(ch), KeyModifiers::NONE);
    }
    assert_eq!(h.app.global_search.query.value(), "session-1");
    assert!(
        !h.app.global_search.results.is_empty(),
        "a matching session name yields at least one result"
    );
}

#[test]
fn ctrl_c_in_a_focused_terminal_preserves_sigint_over_status_copy() {
    // Ctrl+C copies the status message only in non-PTY panes. In a focused
    // terminal it must still fall through to SIGINT — so the status-copy path is
    // skipped and the shown message is left untouched (deterministic: no
    // clipboard call). The status row stays reachable by mouse click there.
    let mut h = Harness::standard(1);
    h.app.focus = InputFocus::Terminal;
    h.app.set_error("boom");

    h.ctrl('c'); // Copy — no selection, terminal-focused → SIGINT, not a copy

    let msg = h.app.status_message.as_ref().expect("message still shown");
    assert_eq!(
        msg.text, "boom",
        "terminal Ctrl+C leaves the status message intact (SIGINT, no copy)"
    );
}

#[test]
fn ctrl_c_outside_a_terminal_copies_the_status_message() {
    // In a non-PTY pane Ctrl+C (no selection) runs the status-copy path. Every
    // branch of `copy_status_to_clipboard` overwrites the toast (copied /
    // unavailable / write-error), so the original message no longer stands —
    // deterministic regardless of whether a clipboard exists on the runner.
    let mut h = Harness::standard(1);
    h.app.focus = InputFocus::SessionList;
    h.app.set_error("boom");

    h.ctrl('c'); // Copy — no selection, non-terminal → copy the status message

    let msg = h
        .app
        .status_message
        .as_ref()
        .expect("a toast is still shown");
    assert_ne!(
        msg.text, "boom",
        "the copy path fired and replaced the original message with its result"
    );
}

#[test]
fn status_message_row_records_a_click_to_copy_target() {
    // The status row is click-to-copy: whenever a message is shown, its rect is
    // registered as a `CopyStatus` hitbox so a mouse click pulls the text out.
    let mut h = Harness::standard(1);
    h.app.set_info("something worth copying");
    h.render();

    let has_target = h
        .app
        .click_targets
        .iter()
        .any(|t| matches!(t.action, ClickAction::CopyStatus));
    assert!(
        has_target,
        "a shown status message registers a CopyStatus click target"
    );
}

// ── Spawn-dependent flows (fake backend, real Tokio I/O wiring) ───────────────

#[tokio::test]
async fn ctrl_r_restarts_session_on_spawnable_backend() {
    // Restart kills + respawns through the backend and rewires I/O; the fake
    // backend makes that succeed without a real tmux/PTY.
    let mut h = Harness::spawnable(1);
    h.ctrl('r'); // RestartSession

    let msg = h
        .app
        .status_message
        .as_ref()
        .expect("restart reports a status toast");
    assert!(
        matches!(msg.level, StatusLevel::Info),
        "restart succeeds (not an error toast): {:?}",
        msg.text
    );
    assert!(
        msg.text.contains("restart"),
        "the toast names the restart: {:?}",
        msg.text
    );
}

#[tokio::test]
async fn ctrl_r_restart_preserves_thurbox_identity_env() {
    // `Session::restart` replaces the session env wholesale, so the restart path
    // must re-inject the `THURBOX_*` identity vars — otherwise the restarted
    // agent loses its identity and the metrics/status hooks break.
    let mut h = Harness::spawnable(1);
    let session_id = h.app.sessions[0].info.id;
    let agent_session_id = h.app.sessions[0]
        .info
        .agent_session_id
        .clone()
        .expect("spawnable sessions have an agent_session_id");

    h.ctrl('r'); // RestartSession

    let env = h.app.sessions[0].env();
    assert_eq!(
        env.get("THURBOX_SESSION"),
        Some(&session_id.to_string()),
        "the thurbox session key survives the restart"
    );
    assert_eq!(
        env.get("THURBOX_SESSION_ID"),
        Some(&agent_session_id),
        "the agent conversation id survives the restart"
    );
}

#[tokio::test]
async fn ctrl_t_opens_shell_pane_on_spawnable_backend() {
    // Ctrl+T lazily spawns a shell pane via the backend and flips the session's
    // terminal view to the shell.
    let mut h = Harness::spawnable(1);
    let id = h.app.sessions[0].info.id;

    h.ctrl('t'); // ToggleShell

    assert!(
        h.app.status_message.is_none()
            || !matches!(
                h.app.status_message.as_ref().unwrap().level,
                StatusLevel::Error
            ),
        "opening the shell pane does not error"
    );
    assert!(
        h.app.sessions[0].shell_pane.is_some(),
        "a shell pane was spawned for the session"
    );
    assert_eq!(
        h.app.session_terminal_views.get(&id),
        Some(&TerminalView::Shell),
        "the active session now shows its shell view"
    );
}

#[tokio::test]
async fn central_tab_strip_switches_agent_and_shell_views() {
    // The top-border tab strip is mouse-driven: clicking Shell flips to the
    // shell view (spawning the pane), clicking Agent flips back.
    let mut h = Harness::spawnable(1);
    let id = h.app.sessions[0].info.id;
    assert_eq!(h.app.active_central_tab(), CentralTab::Agent);

    assert!(h.click_central_tab(CentralTab::Shell), "Shell tab rendered");
    assert!(
        h.app.sessions[0].shell_pane.is_some(),
        "clicking Shell spawned the shell pane"
    );
    assert_eq!(
        h.app.session_terminal_views.get(&id),
        Some(&TerminalView::Shell),
        "clicking Shell selects the shell view"
    );
    assert_eq!(h.app.active_central_tab(), CentralTab::Shell);

    assert!(h.click_central_tab(CentralTab::Agent), "Agent tab rendered");
    assert_eq!(
        h.app.session_terminal_views.get(&id),
        Some(&TerminalView::Claude),
        "clicking Agent returns to the agent view"
    );
    assert_eq!(h.app.active_central_tab(), CentralTab::Agent);
}

#[tokio::test]
async fn f8_leaves_open_review_for_the_shell() {
    // With a review overlaying the central pane, F8 (ToggleShell) must reach the
    // global binding (the review's key capture lets it fall through) and land on
    // the shell — not silently flip the hidden terminal view behind the review.
    let mut h = Harness::spawnable(1);
    let sid = h.app.active_session_id().unwrap();
    h.app
        .code_reviews
        .insert(sid, super::code_review::CodeReviewState::for_test(sid, 2));
    h.app.focus = InputFocus::CodeReview;
    assert_eq!(h.app.active_central_tab(), CentralTab::Review);

    h.func(8); // F8 = ToggleShell

    assert!(
        h.app.active_review().is_none(),
        "F8 closes the open review instead of being swallowed"
    );
    assert_eq!(
        h.app.active_central_tab(),
        CentralTab::Shell,
        "F8 lands on the shell view"
    );
    assert_eq!(
        h.app.focus,
        InputFocus::Terminal,
        "focus moves out of the (now closed) review to the terminal"
    );
    assert!(
        h.app.sessions[0].shell_pane.is_some(),
        "the shell pane was spawned"
    );
}

#[tokio::test]
async fn central_tab_strip_renders_labels_and_shortcuts() {
    // The strip paints Agent/Shell/Review with each toggle's shortcut hint in
    // the pane's top border (Agent has no dedicated key, so no hint).
    let mut h = Harness::spawnable(1);
    let screen = h.render();
    // The tab strip is the pane border row carrying the Review toggle hint `F7`
    // (anchored on it to avoid the "…Agent Orchestrator" header banner). The
    // F-key form is shown, not `^X`, since a focused terminal passes Ctrl chords
    // through to the agent.
    let top = screen.lines().find(|l| l.contains("F7")).unwrap_or("");
    for needle in ["Agent", "Shell", "F8", "Review", "F7"] {
        assert!(
            top.contains(needle),
            "central tab strip missing {needle:?}: {top:?}"
        );
    }
}

#[tokio::test]
async fn central_tab_strip_omits_feature_gated_tabs() {
    // Shell/Review tabs are gated by their feature flags. With both off, only
    // the Agent pill would remain — a tab strip you can't switch away from — so
    // the whole strip is suppressed rather than advertising a lone dead tab.
    let mut h = Harness::spawnable(1);
    let collect_tabs = |app: &super::App| -> Vec<CentralTab> {
        app.click_targets
            .iter()
            .filter_map(|t| match t.action {
                ClickAction::CentralTab(tab) => Some(tab),
                _ => None,
            })
            .collect()
    };

    // Only one alternate view gated off → the strip stays (Agent + the other).
    h.app.features.shell_pane = false;
    h.app.features.code_review = true;
    h.render();
    assert_eq!(
        collect_tabs(&h.app),
        vec![CentralTab::Agent, CentralTab::Review],
        "Review survives when only Shell is gated off"
    );

    // Both alternate views gated off → no tab strip at all.
    h.app.features.shell_pane = false;
    h.app.features.code_review = false;
    h.render();
    assert!(
        collect_tabs(&h.app).is_empty(),
        "the lone Agent tab is dropped when Shell and Review are both off"
    );
}

/// The F2 info panel lists upcoming automation runs. When the `automations`
/// feature is off the TUI never fires those schedules (and the pane is hidden),
/// so the info panel must not surface them either — even though the cache is
/// still loaded from the DB.
#[tokio::test]
async fn info_panel_hides_automations_when_feature_off() {
    use crate::session::{Automation, AutomationAction, AutomationSchedule};
    let mut h = Harness::spawnable(1);
    h.app.show_info_panel = true;
    let far_future = crate::sync::current_time_millis() + 3_600_000;
    h.app.automation_ui.cached_automations = vec![Automation {
        id: 1,
        name: "infopanelnightly".into(),
        enabled: true,
        schedule: AutomationSchedule::Once { at: 0 },
        timezone: None,
        action: AutomationAction::Send {
            session_id: SessionId::default(),
        },
        prompt: "p".into(),
        created_at: 0,
        updated_at: 0,
        last_run_at: None,
        next_run_at: Some(far_future),
    }];

    // Feature on: the info panel's automations section lists it.
    h.app.features.automations = true;
    assert!(
        h.render().contains("infopanelnightly"),
        "info panel surfaces the upcoming automation when the feature is on"
    );

    // Feature off: the pane is hidden *and* the info-panel section is dropped,
    // so the automation appears nowhere.
    h.app.features.automations = false;
    assert!(
        !h.render().contains("infopanelnightly"),
        "info panel must not surface automations when the feature is off"
    );
}

// ── Performance counters: deterministic render-path proxies ───────────────────
//
// These assert on `App::perf_counters()` — wall-clock-free counts — so they
// gate the redraw-throttling and per-frame caching optimizations without timing
// flakiness. The acceptance harness drives `view()` directly (it skips
// `tick()`), so only the render-path counters are exercised here; the
// tick-driven counters (`status_refreshes`) and the redraw-skip accounting live
// in the `#[tokio::test]` units in `super::tests`.

#[test]
fn perf_hud_toggles_with_f12_and_activates_timing() {
    let mut h = Harness::standard(1);
    assert!(!h.app.perf_timing_active(), "timing is off by default");
    h.key(KeyCode::F(12), KeyModifiers::NONE);
    assert!(h.app.show_perf_hud, "F12 opens the perf HUD");
    assert!(
        h.app.perf_timing_active(),
        "an open HUD switches timing collection on"
    );
    h.render(); // the overlay renders without disturbing the panes
    h.key(KeyCode::F(12), KeyModifiers::NONE);
    assert!(!h.app.show_perf_hud, "F12 closes it again");
}

#[test]
fn perf_hud_feature_flag_disables_toggle_and_closes_overlay() {
    let mut h = Harness::standard(1);
    h.key(KeyCode::F(12), KeyModifiers::NONE);
    assert!(h.app.show_perf_hud);
    // Disabling the live flag tears the overlay down and blocks the chord.
    let mut settings = crate::session::settings::Settings::default();
    settings.features.perf_hud = false;
    h.app.apply_live_settings(&settings);
    assert!(!h.app.show_perf_hud, "disabling the flag closes the HUD");
    h.key(KeyCode::F(12), KeyModifiers::NONE);
    assert!(!h.app.show_perf_hud, "the chord toasts instead of toggling");
}

#[test]
fn perf_render_counter_tracks_painted_frames() {
    let mut h = Harness::standard(2);
    assert_eq!(h.app.perf_counters().frames_rendered, 0);
    h.render();
    h.render();
    h.render();
    assert_eq!(
        h.app.perf_counters().frames_rendered,
        3,
        "each view() paint bumps frames_rendered exactly once"
    );
}

#[test]
fn perf_terminal_render_locks_parser_once_per_frame() {
    // With an active session, the central pane locks its vt100 parser once per
    // painted frame (the O(1) scrollback read rides along, so it is not tracked
    // separately). Redraw throttling, not caching, bounds how often this runs.
    let mut h = Harness::standard(1);
    h.render();
    h.render();
    assert_eq!(
        h.app.perf_counters().parser_locks_render,
        2,
        "one parser lock per terminal frame"
    );
}

#[test]
fn perf_session_order_cached_across_idle_frames() {
    // The session-list ordering is status-independent, so once built it is
    // reused across frames whose grouping/nesting inputs didn't change. Three
    // paints with no session mutation must rebuild the order exactly once.
    let mut h = Harness::standard(3);
    h.render();
    h.render();
    h.render();
    assert_eq!(
        h.app.perf_counters().ordered_sessions_rebuilds,
        1,
        "the session order is cached: only the first frame rebuilds it"
    );
}

#[test]
fn perf_session_order_rebuilds_when_sessions_change() {
    // Adding a session changes the order signature, so the cache is invalidated
    // and the order rebuilt — exactly once for the change.
    let mut h = Harness::standard(2);
    h.render(); // builds the order (rebuild #1)
    h.render(); // cache hit, no rebuild
    assert_eq!(h.app.perf_counters().ordered_sessions_rebuilds, 1);

    // Mutate the session set, then repaint.
    let backend: Arc<dyn SessionBackend> = Arc::new(FakeBackend::stub());
    let provider: Arc<dyn AgentProvider> = Arc::new(GenericProvider::new(
        crate::agent::agent_config::builtin_registry()
            .default_agent()
            .unwrap()
            .clone(),
    ));
    h.app
        .sessions
        .push(Session::stub("session-new", &backend, &provider));
    h.render(); // signature changed → rebuild #2
    h.render(); // cache hit again
    assert_eq!(
        h.app.perf_counters().ordered_sessions_rebuilds,
        2,
        "a session-set change invalidates the cache exactly once"
    );
}

#[test]
fn perf_status_change_keeps_order_cache() {
    // The order is status-independent (ADR-P3): a session changing status must
    // NOT invalidate the cache — only grouping/ordering/nesting inputs do. This
    // pins the signature's field set; adding `status` to it would fail here.
    let mut h = Harness::standard(2);
    h.render(); // rebuild #1
    h.render(); // cache hit
    assert_eq!(h.app.perf_counters().ordered_sessions_rebuilds, 1);

    h.app.sessions[0].info.status = SessionStatus::Blocked;
    h.render(); // status changed, but order inputs did not → still a cache hit
    assert_eq!(
        h.app.perf_counters().ordered_sessions_rebuilds,
        1,
        "a status change must not rebuild the (status-independent) order"
    );
}

// ── Redraw throttling: the dirty-flag decision the render loop gates on ───────

#[test]
fn perf_first_frame_is_always_dirty() {
    // `needs_redraw` starts true so the very first loop iteration paints (the
    // smoke test and a real launch both rely on this).
    let h = Harness::standard(1);
    assert!(h.app.should_redraw(), "a freshly built App must paint once");
}

#[test]
fn perf_clean_state_skips_redraw() {
    // After a paint with nothing changed, the loop skips the (expensive) draw.
    let mut h = Harness::standard(1);
    h.app.mark_redrawn();
    assert!(
        !h.app.should_redraw(),
        "no input/output/forced-floor → no redraw"
    );
}

#[test]
fn perf_input_requests_redraw() {
    // Any key event re-dirties the UI so keypress-to-screen stays immediate.
    let mut h = Harness::standard(1);
    h.app.mark_redrawn();
    assert!(!h.app.should_redraw());
    h.ctrl('j'); // NextSession — goes through update()
    assert!(
        h.app.should_redraw(),
        "input must mark the UI dirty for the next frame"
    );
}

#[test]
fn perf_no_new_output_does_not_request_redraw() {
    // The lock-free output detector must not false-positive: with no reader
    // thread producing output, a second poll sees an unchanged signature and
    // leaves the UI clean.
    let mut h = Harness::standard(2);
    h.app.detect_output_redraw(); // prime the output-generation baseline
    h.app.mark_redrawn(); // clear any dirty from the first observation
    h.app.detect_output_redraw(); // no new output
    assert!(
        !h.app.should_redraw(),
        "unchanged output signature must not trigger a redraw"
    );
}

#[test]
fn perf_idle_iterations_skip_the_paint() {
    // Mimic the render loop's gate over several idle iterations (well within the
    // forced-redraw floor): the first paints, the rest are skipped.
    let mut h = Harness::standard(2);
    h.app.detect_output_redraw(); // prime output baseline
    let mut requested = 0u64;
    let mut skipped = 0u64;
    for _ in 0..5 {
        if h.app.should_redraw() {
            h.app.mark_redrawn();
            requested += 1;
        } else {
            h.app.note_redraw_skipped();
            skipped += 1;
        }
        h.app.detect_output_redraw(); // no new output between iterations
    }
    assert_eq!(requested, 1, "only the initial dirty frame paints");
    assert_eq!(skipped, 4, "idle iterations skip the expensive draw");
    assert_eq!(h.app.perf_counters().redraws_skipped, 4);
}

/// Disabling a live feature flag at runtime tears down whatever panel/view it
/// had left open (otherwise the panel keeps rendering with its tab/footer
/// affordance gone). Covers the `apply_live_settings` → `enforce_feature_visibility`
/// path the settings panel and config-reload both run.
#[tokio::test]
async fn disabling_a_live_feature_tears_down_its_open_surfaces() {
    let mut h = Harness::spawnable(1);
    let sid = h.app.sessions[0].info.id;

    // Open every live-gated surface, and park focus on the file viewer.
    h.app.show_info_panel = true;
    h.app.show_file_viewer = true;
    h.app.show_tasks_panel = true;
    h.app
        .session_terminal_views
        .insert(sid, TerminalView::Shell);
    open_minimal_review(&mut h);
    h.app.focus = InputFocus::FileViewer;

    // Flip the live UI feature flags off and re-apply (as the settings panel does).
    let mut settings = crate::session::settings::Settings::default();
    settings.features.info_panel = false;
    settings.features.file_viewer = false;
    settings.features.tasks = false;
    settings.features.shell_pane = false;
    settings.features.code_review = false;
    h.app.apply_live_settings(&settings);

    assert!(!h.app.show_info_panel, "info panel hidden");
    assert!(!h.app.show_file_viewer, "file viewer hidden");
    assert!(!h.app.show_tasks_panel, "tasks panel hidden");
    assert_eq!(
        h.app.session_terminal_views.get(&sid).copied(),
        Some(TerminalView::Claude),
        "shell view reverted to the agent view"
    );
    assert!(h.app.code_reviews.is_empty(), "open review closed");
    assert!(
        matches!(h.app.focus, InputFocus::SessionList),
        "focus moved off the now-hidden file viewer"
    );
}

/// `dispatch_action` partitions `Action` across several sub-dispatchers whose
/// final arm (`dispatch_scoped_pane_action`) is `unreachable!()`. A new `Action`
/// variant that isn't wired into any dispatcher would therefore panic at runtime
/// instead of failing to compile — this exercises every variant through the real
/// dispatch path so an unrouted action fails the suite loudly. A fresh harness
/// per action keeps the routing decision independent of accumulated side effects.
#[tokio::test]
async fn every_action_is_routed_by_dispatch_action() {
    for &action in crate::session::Action::all() {
        let mut h = Harness::standard(1);
        // The assertion is simply that this does not hit the `unreachable!()` in
        // `dispatch_scoped_pane_action` (or otherwise panic).
        let _ = h.app.dispatch_action(action);
    }
}

/// Install a minimal open+focused review on the harness (no git worktree
/// needed), for testing the view's key fall-through behavior.
fn open_minimal_review(h: &mut Harness) {
    use std::collections::HashSet;
    let sid = h.app.sessions[0].info.id;
    h.app.code_reviews.insert(
        sid,
        crate::app::code_review::CodeReviewState {
            session_id: sid,
            loading: false,
            repos: Vec::new(),
            multi: false,
            files: Vec::new(),
            comments: Vec::new(),
            reviewed_files: HashSet::new(),
            reviewed_hunks: HashSet::new(),
            fold_override: HashSet::new(),
            rows: Vec::new(),
            selected: 0,
            scroll: 0,
            compose: None,
            side_by_side: false,
            click_side: None,
            h_scroll: 0,
            wrap: false,
            target: crate::app::code_review::ReviewTarget::Working,
            commits: Vec::new(),
            host: None,
            target_picker: None,
            search: None,
        },
    );
    h.app.focus = InputFocus::CodeReview;
}

/// The review pane toggles shut on its own key, like every other pane: with a
/// review open and focused, pressing the bound chord (F7) again closes it and
/// moves focus away. Regression for the key being swallowed by the review's
/// own capture handler.
#[test]
fn review_toggle_key_closes_open_review() {
    let mut h = Harness::new(STD_COLS, STD_ROWS, 1);
    open_minimal_review(&mut h);

    h.key(KeyCode::F(7), KeyModifiers::NONE);

    assert!(
        h.app.active_review().is_none(),
        "pressing the review toggle again closes the open review"
    );
    assert_ne!(
        h.app.focus,
        InputFocus::CodeReview,
        "focus leaves the review when it closes"
    );
}

/// `/` opens find-in-diff (file-viewer pattern): typing jumps to the first
/// match, `Tab` commits, `n`/`N` step matches relative to the cursor, and `Esc`
/// clears the search before it closes the review.
#[test]
fn review_search_flow_finds_navigates_and_clears() {
    let mut h = Harness::new(STD_COLS, STD_ROWS, 1);
    let sid = h.app.sessions[0].info.id;
    // Two files: src/f0.rs, src/f1.rs (each one added line).
    h.app.code_reviews.insert(
        sid,
        crate::app::code_review::CodeReviewState::for_test(sid, 2),
    );
    h.app.focus = InputFocus::CodeReview;

    // `/` enters the search sub-mode (capturing keys).
    h.key(KeyCode::Char('/'), KeyModifiers::NONE);
    assert!(h
        .app
        .active_review()
        .and_then(|cr| cr.search.as_ref())
        .is_some_and(|s| s.editing));

    // Type ".rs" → matches both file headers; selection jumps to the first.
    for c in ['.', 'r', 's'] {
        h.key(KeyCode::Char(c), KeyModifiers::NONE);
    }
    let (first, second) = {
        let cr = h.app.active_review().unwrap();
        let s = cr.search.as_ref().unwrap();
        assert_eq!(s.matches.len(), 2, "both file headers match '.rs'");
        assert_eq!(cr.selected, s.matches[0], "jumps to the first match");
        (s.matches[0], s.matches[1])
    };

    // Enter (while typing) steps to the next match without leaving the input.
    h.key(KeyCode::Enter, KeyModifiers::NONE);
    assert_eq!(h.app.active_review().unwrap().selected, second);
    assert!(
        h.app
            .active_review()
            .unwrap()
            .search
            .as_ref()
            .unwrap()
            .editing
    );

    // Tab commits: still open, no longer editing.
    h.key(KeyCode::Tab, KeyModifiers::NONE);
    assert!(h
        .app
        .active_review()
        .and_then(|cr| cr.search.as_ref())
        .is_some_and(|s| !s.editing));

    // `n`/`N` step matches relative to the cursor (wrapping).
    h.key(KeyCode::Char('n'), KeyModifiers::NONE);
    assert_eq!(
        h.app.active_review().unwrap().selected,
        first,
        "n from the last match wraps to the first"
    );
    h.key(KeyCode::Char('N'), KeyModifiers::NONE);
    assert_eq!(
        h.app.active_review().unwrap().selected,
        second,
        "N from the first match wraps to the last"
    );

    // Esc clears the search but keeps the review open; a second Esc closes it.
    h.key(KeyCode::Esc, KeyModifiers::NONE);
    assert!(h.app.active_review().is_some());
    assert!(h.app.active_review().unwrap().search.is_none());
    h.key(KeyCode::Esc, KeyModifiers::NONE);
    assert!(h.app.active_review().is_none());
}

/// Insert a review whose first diff line is `width` chars wide, so horizontal
/// scroll / wrap have something to act on. Returns the session id.
#[cfg(test)]
fn open_review_with_long_line(h: &mut Harness, width: usize) -> crate::session::SessionId {
    let sid = h.app.sessions[0].info.id;
    let mut cr = crate::app::code_review::CodeReviewState::for_test(sid, 1);
    cr.files[0].hunks[0].lines[0].text = "a".repeat(width);
    cr.rebuild_rows();
    h.app.code_reviews.insert(sid, cr);
    h.app.focus = InputFocus::CodeReview;
    sid
}

/// `Left`/`Right` (and `h`/`l`) scroll the diff body horizontally, clamped to
/// the longest line; `w` toggles wrap and resets the offset; scroll is a no-op
/// while wrapped.
#[test]
fn review_horizontal_scroll_and_wrap_toggle() {
    let mut h = Harness::new(STD_COLS, STD_ROWS, 1);
    open_review_with_long_line(&mut h, 300);

    // Right scrolls the body (step 8); Left scrolls back and clamps at 0.
    h.key(KeyCode::Right, KeyModifiers::NONE);
    assert_eq!(h.app.active_review().unwrap().h_scroll, 8);
    h.key(KeyCode::Char('l'), KeyModifiers::NONE);
    assert_eq!(
        h.app.active_review().unwrap().h_scroll,
        16,
        "`l` also scrolls right"
    );
    for _ in 0..10 {
        h.key(KeyCode::Left, KeyModifiers::NONE);
    }
    assert_eq!(
        h.app.active_review().unwrap().h_scroll,
        0,
        "Left clamps at 0"
    );

    // A big jump clamps to the longest line (max_line_width - 1 = 299).
    for _ in 0..100 {
        h.key(KeyCode::Right, KeyModifiers::NONE);
    }
    assert_eq!(
        h.app.active_review().unwrap().h_scroll,
        299,
        "scroll clamps to the widest line"
    );

    // `w` turns on wrap and resets the horizontal offset; while wrapped, scroll
    // is a no-op.
    h.key(KeyCode::Char('w'), KeyModifiers::NONE);
    {
        let cr = h.app.active_review().unwrap();
        assert!(cr.wrap, "`w` enables wrap");
        assert_eq!(cr.h_scroll, 0, "enabling wrap resets h_scroll");
    }
    h.key(KeyCode::Right, KeyModifiers::NONE);
    assert_eq!(
        h.app.active_review().unwrap().h_scroll,
        0,
        "horizontal scroll is a no-op while wrapped"
    );

    // `w` again turns wrap off.
    h.key(KeyCode::Char('w'), KeyModifiers::NONE);
    assert!(!h.app.active_review().unwrap().wrap);
}

/// Entering side-by-side pins the horizontal offset to 0 (h-scroll is
/// unified-only) and scroll stays a no-op there.
#[test]
fn review_side_by_side_disables_horizontal_scroll() {
    let mut h = Harness::new(STD_COLS, STD_ROWS, 1);
    open_review_with_long_line(&mut h, 300);

    h.key(KeyCode::Right, KeyModifiers::NONE);
    assert_eq!(h.app.active_review().unwrap().h_scroll, 8);

    // `v` → side-by-side resets the offset.
    h.key(KeyCode::Char('v'), KeyModifiers::NONE);
    {
        let cr = h.app.active_review().unwrap();
        assert!(cr.side_by_side);
        assert_eq!(cr.h_scroll, 0, "side-by-side resets h_scroll");
    }
    h.key(KeyCode::Right, KeyModifiers::NONE);
    assert_eq!(
        h.app.active_review().unwrap().h_scroll,
        0,
        "no horizontal scroll in side-by-side"
    );
}

/// A review is per-session like the shell view: switching to another session
/// hides it (and demotes the central focus), and switching back shows it again
/// — the state is preserved, not torn down.
#[test]
fn review_persists_per_session_across_switches() {
    let mut h = Harness::new(STD_COLS, STD_ROWS, 2);
    h.app.active_index = 0;
    open_minimal_review(&mut h); // review open + focused on session 0
    h.render();
    assert!(h.app.active_review().is_some());
    assert_eq!(h.app.focus, InputFocus::CodeReview);

    // Switch to session 1 (no review): it's hidden and focus drops off the
    // review (synced on render).
    h.app.active_index = 1;
    h.render();
    assert!(
        h.app.active_review().is_none(),
        "the other session has no review"
    );
    assert_ne!(
        h.app.focus,
        InputFocus::CodeReview,
        "focus leaves the review when its session isn't active"
    );

    // Switch back to session 0: the review is still there and re-focused.
    h.app.active_index = 0;
    h.render();
    assert!(
        h.app.active_review().is_some(),
        "session 0's review is preserved across the round-trip"
    );
    assert_eq!(
        h.app.focus,
        InputFocus::CodeReview,
        "returning to the review session re-focuses it"
    );
}

/// Hovering a code-review footer button brightens its fill to `accent_bright`,
/// exactly like the global footer and modal buttons. Regression: review footer
/// buttons (recorded as `ClickAction::ReviewButton`) were left out of the hover
/// highlight, so they never lit up under the pointer.
#[test]
fn hovering_review_footer_button_brightens_it() {
    let mut h = Harness::new(STD_COLS, STD_ROWS, 1);
    open_minimal_review(&mut h);
    h.render();
    let r = h
        .app
        .click_targets
        .iter()
        .find(|t| matches!(t.action, ClickAction::ReviewButton(_)))
        .map(|t| t.rect)
        .expect("review footer buttons recorded");
    h.app.update(AppMessage::MouseMove { x: r.x, y: r.y });
    h.render();
    let buf = h.terminal.backend().buffer();
    assert_eq!(
        buf[(r.x, r.y)].bg,
        crate::ui::theme::Theme::accent_bright(),
        "hovered review footer button should brighten to accent_bright"
    );
}

/// Clicking a diff row in the main review pane focuses it (regression: the
/// `ReviewRow` click selected the row but never set `InputFocus::CodeReview`,
/// so a click while another pane was focused left focus elsewhere — the
/// whole-pane `FocusPane` fallback is recorded after the row targets and never
/// wins on a row hit).
#[test]
fn clicking_review_row_focuses_the_pane() {
    let mut h = Harness::new(STD_COLS, STD_ROWS, 1);
    let sid = h.app.sessions[0].info.id;
    h.app.code_reviews.insert(
        sid,
        crate::app::code_review::CodeReviewState::for_test(sid, 2),
    );
    h.app.focus = InputFocus::CodeReview;
    h.render();
    // Move focus off the review, as if the session list were active.
    h.app.focus = InputFocus::SessionList;
    let r = h
        .app
        .click_targets
        .iter()
        .find(|t| matches!(t.action, ClickAction::ReviewRow(_)))
        .map(|t| t.rect)
        .expect("review diff rows recorded as click targets");
    h.app.update(AppMessage::MouseClick {
        x: r.x,
        y: r.y,
        modifiers: KeyModifiers::NONE,
    });
    assert_eq!(
        h.app.focus,
        InputFocus::CodeReview,
        "clicking a diff row focuses the review pane"
    );
}

/// A click in the paired side-by-side layout records which column (old | new)
/// it hit, so a follow-up comment attaches to that side; a later column-less
/// select (scrollbar drag) clears it.
#[test]
fn side_by_side_click_records_column_side() {
    use crate::session::review::Side;
    let mut h = Harness::new(STD_COLS, STD_ROWS, 1);
    let sid = h.app.sessions[0].info.id;
    let mut cr = crate::app::code_review::CodeReviewState::for_test(sid, 1);
    cr.side_by_side = true;
    cr.rebuild_rows();
    h.app.code_reviews.insert(sid, cr);
    h.app.focus = InputFocus::CodeReview;
    h.render();
    let r = h
        .app
        .click_targets
        .iter()
        .find(|t| matches!(t.action, ClickAction::ReviewRow(_)))
        .map(|t| t.rect)
        .expect("review diff rows recorded as click targets");

    // Right column → the New side is recorded for that row.
    h.app.update(AppMessage::MouseClick {
        x: r.x + r.width - 1,
        y: r.y,
        modifiers: KeyModifiers::NONE,
    });
    assert!(
        matches!(
            h.app.active_review().unwrap().click_side,
            Some((_, Side::New))
        ),
        "a right-column click records the New side"
    );

    // Left column → the Old side.
    h.app.update(AppMessage::MouseClick {
        x: r.x,
        y: r.y,
        modifiers: KeyModifiers::NONE,
    });
    assert!(
        matches!(
            h.app.active_review().unwrap().click_side,
            Some((_, Side::Old))
        ),
        "a left-column click records the Old side"
    );

    // A column-less select (scrollbar drag / cr_select_row) clears it.
    let sel = h.app.active_review().unwrap().selected;
    h.app.cr_select_row(sel);
    assert!(
        h.app.active_review().unwrap().click_side.is_none(),
        "a column-less select clears the recorded click side"
    );
}

/// The compose box is an anchored overlay, so it is hit-tested **before** the
/// diff rows it floats over: a click inside it is swallowed instead of selecting
/// the row underneath. Before the overlay layer the click fell through, moving
/// the selection while the box went on commenting on the line it was opened for.
#[test]
fn clicking_the_compose_overlay_does_not_move_the_diff_selection() {
    use crate::app::code_review::ReviewRow;
    let mut h = Harness::new(STD_COLS, STD_ROWS, 1);
    let sid = h.app.sessions[0].info.id;
    let mut cr = crate::app::code_review::CodeReviewState::for_test(sid, 6);
    // Anchor the box to the first diff line, so the rows it covers are below it.
    cr.selected = cr
        .rows
        .iter()
        .position(|r| matches!(r, ReviewRow::Line(..)))
        .expect("the fixture has diff lines");
    h.app.code_reviews.insert(sid, cr);
    h.app.focus = InputFocus::CodeReview;
    h.app
        .cr_button(crate::app::code_review::ReviewButton::Comment);
    h.render();

    let overlay = h
        .app
        .click_targets
        .iter()
        .find(|t| matches!(t.action, ClickAction::OverlayCapture))
        .map(|t| t.rect)
        .expect("the open compose box records an overlay target");
    // A row target the box covers: the click hits both, and the overlay wins
    // because it is recorded first.
    let covered = h
        .app
        .click_targets
        .iter()
        .find(|t| {
            matches!(t.action, ClickAction::ReviewRow(_))
                && t.rect.y > overlay.y
                && t.rect.y < overlay.y + overlay.height
        })
        .map(|t| (t.action, t.rect))
        .expect("the compose box covers at least one diff row");
    let (covered_action, covered_rect) = covered;
    let selected = h.app.active_review().unwrap().selected;

    h.app.update(AppMessage::MouseClick {
        x: covered_rect.x + 2,
        y: covered_rect.y,
        modifiers: KeyModifiers::NONE,
    });
    assert_eq!(
        h.app.active_review().unwrap().selected,
        selected,
        "a click inside the compose box leaves the selection alone"
    );

    // The same press with nothing anchored selects that row — so the assertion
    // above is about the overlay, not about the row being unclickable.
    h.app
        .cr_button(crate::app::code_review::ReviewButton::Cancel);
    h.render();
    h.app.update(AppMessage::MouseClick {
        x: covered_rect.x + 2,
        y: covered_rect.y,
        modifiers: KeyModifiers::NONE,
    });
    let ClickAction::ReviewRow(index) = covered_action else {
        unreachable!("filtered to review rows above");
    };
    assert_eq!(
        h.app.active_review().unwrap().selected,
        index,
        "without the overlay the same click selects the row"
    );
}

/// The review-target picker is mouse-driven too: clicking one of its entries
/// dispatches the switch to that target, mirroring the keyboard Enter path.
/// The rebuild itself runs on a background worker (ADR-P8) — its application
/// is covered by `perf_review_build_result_applied_via_poll` — so this asserts
/// the dispatch side: picker closed, loading state on, build handed off.
#[tokio::test]
async fn clicking_review_target_entry_switches_target() {
    use crate::app::code_review::ReviewTarget;
    let mut h = Harness::new(STD_COLS, STD_ROWS, 1);
    let sid = h.app.sessions[0].info.id;
    h.app.code_reviews.insert(
        sid,
        crate::app::code_review::CodeReviewState::for_test(sid, 2),
    );
    h.app.focus = InputFocus::CodeReview;
    // The picker opens on Branch (the for_test default); it offers Working +
    // Branch entries.
    h.app.cr_open_target_picker();
    assert_eq!(h.app.active_review().unwrap().target, ReviewTarget::Branch);
    h.render();

    // Find the "Working" entry's hitbox (index 0) and click it.
    let r = h
        .app
        .click_targets
        .iter()
        .find(|t| matches!(t.action, ClickAction::ReviewTarget(0)))
        .map(|t| t.rect)
        .expect("target-picker entries recorded as click targets");
    h.app.update(AppMessage::MouseClick {
        x: r.x,
        y: r.y,
        modifiers: KeyModifiers::NONE,
    });

    let cr = h.app.active_review().unwrap();
    assert_eq!(
        cr.target,
        ReviewTarget::Branch,
        "the target only switches once the background build lands"
    );
    assert!(cr.loading, "the click enters the loading state");
    assert!(
        cr.target_picker.is_none(),
        "selecting a target closes the picker"
    );
    assert_eq!(
        h.app.perf_counters().review_builds_dispatched,
        1,
        "the rebuild was handed to the background worker"
    );
}

/// Global overlay/panel toggles fall through the review's key capture so they
/// stay reachable while a review is open (regression: the capture handler
/// swallowed them). The review itself stays open.
#[test]
fn info_panel_toggles_while_review_is_open() {
    let mut h = Harness::new(STD_COLS, STD_ROWS, 1);
    open_minimal_review(&mut h);
    assert!(!h.app.show_info_panel);

    h.key(KeyCode::F(2), KeyModifiers::NONE);

    assert!(
        h.app.show_info_panel,
        "F2 toggles the info panel even while the review is focused"
    );
    assert!(
        h.app.active_review().is_some(),
        "toggling the info panel leaves the review open"
    );
}

// ── Remote-hook status events (control-mode subscription → hook columns) ──────

/// The backend queued a remote-hook event for a session's pane: one refresh
/// drains it into the hook columns and the derived status reflects it — the
/// remote analogue of a local `thurbox-cli session signal`.
#[test]
fn remote_hook_event_drives_session_status() {
    let backend = Arc::new(FakeBackend::stub());
    let mut h = Harness::with_backend(STD_COLS, STD_ROWS, 2, backend.clone());
    h.app.sessions[0].info.backend_id = Some("%5".into());
    h.app.sessions[1].info.backend_id = Some("%9".into());
    h.app.save_state(); // hook columns update persisted rows

    backend.push_hook_event("%5", "working");
    h.app.refresh_session_statuses();

    // Stub sessions have fresh output, so `working` isn't quiescence-demoted.
    assert_eq!(h.app.sessions[0].info.status, SessionStatus::Working);
    assert_eq!(
        h.app.sessions[1].info.status,
        SessionStatus::Idle,
        "the other session is untouched"
    );
    let rows = h.app.db.load_hook_states().unwrap();
    assert_eq!(
        rows.get(&h.app.sessions[0].info.id)
            .and_then(|r| r.state.as_deref()),
        Some("working"),
        "the event is persisted through set_hook_state"
    );
}

/// Events that don't resolve to a session never touch the DB (an unknown pane
/// is *parked* for the adoption retry — see
/// `remote_hook_event_parked_until_session_adopted` — not applied), and a
/// non-allow-listed state (the value is remote-controlled free text) is
/// dropped outright.
#[test]
fn remote_hook_event_ignores_unmatched_and_invalid() {
    let backend = Arc::new(FakeBackend::stub());
    let mut h = Harness::with_backend(STD_COLS, STD_ROWS, 1, backend.clone());
    h.app.sessions[0].info.backend_id = Some("%5".into());
    h.app.save_state();

    backend.push_hook_event("%99", "working"); // no such pane
    backend.push_hook_event("%5", "rm -rf /"); // not an allowed state
    h.app.refresh_session_statuses();

    assert_eq!(h.app.sessions[0].info.status, SessionStatus::Idle);
    assert!(
        h.app
            .db
            .load_hook_states()
            .unwrap()
            .values()
            .all(|r| r.state.is_none()),
        "neither event may reach the hook columns"
    );
}

/// An event for a pane no session claims *yet* is parked and re-applied once
/// the session appears: the subscription's initial catch-up report lands while
/// the background restore is still adopting that host's windows, and dropping
/// it would lose e.g. a `done` set while the TUI was closed.
#[test]
fn remote_hook_event_parked_until_session_adopted() {
    let backend = Arc::new(FakeBackend::stub());
    let mut h = Harness::with_backend(STD_COLS, STD_ROWS, 1, backend.clone());
    let id = h.app.sessions[0].info.id;

    backend.push_hook_event("%5", "working");
    h.app.refresh_session_statuses(); // no session owns %5 yet → parked

    h.app.sessions[0].info.backend_id = Some("%5".into());
    h.app.save_state();
    h.app.refresh_session_statuses(); // nothing new pushed — the parked event applies

    let rows = h.app.db.load_hook_states().unwrap();
    assert_eq!(
        rows.get(&id).and_then(|r| r.state.as_deref()),
        Some("working"),
        "the pre-adoption event must survive to the adopting tick"
    );
}

/// Two transitions for one pane in a single drained batch (`working` then
/// `done`, e.g. queued while the main thread stalled) must both land: deduping
/// the second against the stale pre-batch cache would swallow the `done` and
/// leave the session spinning on `working`.
#[test]
fn remote_hook_batch_applies_both_transitions() {
    let backend = Arc::new(FakeBackend::stub());
    let mut h = Harness::with_backend(STD_COLS, STD_ROWS, 1, backend.clone());
    h.app.sessions[0].info.backend_id = Some("%5".into());
    h.app.save_state();
    let id = h.app.sessions[0].info.id;

    // A previous turn ended `done`, absorbed into the cache.
    backend.push_hook_event("%5", "done");
    h.app.refresh_session_statuses();

    backend.push_hook_event("%5", "working");
    backend.push_hook_event("%5", "done");
    h.app.refresh_session_statuses();

    let rows = h.app.db.load_hook_states().unwrap();
    assert_eq!(
        rows.get(&id).and_then(|r| r.state.as_deref()),
        Some("done"),
        "the batch's final transition must not be swallowed by the stale cache"
    );
}

/// A re-report of the current state (the subscription re-sends the pane
/// option's value on reconnect/TUI restart) must not re-stamp `state_at` —
/// otherwise an already-acknowledged `done` resurrects as unseen and re-fires
/// its OS notification on every restart.
#[test]
fn remote_hook_event_dedupes_repeated_state() {
    let backend = Arc::new(FakeBackend::stub());
    let mut h = Harness::with_backend(STD_COLS, STD_ROWS, 1, backend.clone());
    h.app.sessions[0].info.backend_id = Some("%5".into());
    h.app.save_state();
    let id = h.app.sessions[0].info.id;

    backend.push_hook_event("%5", "done");
    h.app.refresh_session_statuses();
    let first_at = h.app.db.load_hook_states().unwrap()[&id].state_at;
    assert!(first_at.is_some());

    std::thread::sleep(std::time::Duration::from_millis(5));
    backend.push_hook_event("%5", "done");
    h.app.refresh_session_statuses();
    let second_at = h.app.db.load_hook_states().unwrap()[&id].state_at;
    assert_eq!(
        first_at, second_at,
        "an identical re-report must not re-stamp state_at"
    );
}

// ── Tick-driven behavior: timers, debounce, redraw floor ─────────────────────
//
// These drive `App::tick_core` (the deterministic half of the event loop's
// tick) with the clock fast-forwarded via `Harness::advance`, so every
// wall-clock-gated behavior is asserted without sleeping.

#[test]
fn status_message_expires_after_timeout_via_tick() {
    let mut h = Harness::standard(1);
    h.app.set_status(StatusLevel::Info, "transient note");
    assert!(h.app.status_message.is_some());

    h.tick();
    assert!(
        h.app.status_message.is_some(),
        "a fresh message survives a tick"
    );

    h.advance(STATUS_MESSAGE_TIMEOUT).tick();
    assert!(
        h.app.status_message.is_none(),
        "the tick clears an expired status message"
    );
}

/// The regression this whole indicator exists for: creating a session used to
/// announce itself with a `status_message`, which expires after 5 s — so a
/// `git worktree add` on a large repo went silent partway through and the app
/// looked idle. `pending_spawn` is not a status message and must outlive it.
#[test]
fn spawn_progress_outlives_the_status_message_timeout() {
    let mut h = Harness::standard(1);
    h.app.pending_spawn = Some(PendingSpawn::new("feat/big", SpawnPhase::Worktree));

    h.advance(STATUS_MESSAGE_TIMEOUT * 4).tick();

    assert!(
        h.app.pending_spawn.is_some(),
        "a spawn still in flight is never expired by the status-message timer"
    );
    let screen = h.render();
    assert!(
        screen.contains("Creating worktree(s)…"),
        "the status row still shows the phase after 20s:\n{screen}"
    );
    assert!(
        screen.contains("feat/big"),
        "the placeholder row names the session being created:\n{screen}"
    );
}

/// The elapsed counter is what makes a long wait read as progressing rather
/// than hung, so it must actually advance with the clock.
#[test]
fn spawn_progress_reports_elapsed_time() {
    let mut h = Harness::standard(1);
    h.app.pending_spawn = Some(PendingSpawn::new("feat/big", SpawnPhase::Worktree));
    assert_eq!(h.app.pending_spawn.as_ref().unwrap().elapsed_secs(), 0);

    h.advance(std::time::Duration::from_secs(14)).tick();

    assert_eq!(h.app.pending_spawn.as_ref().unwrap().elapsed_secs(), 14);
    let screen = h.render();
    assert!(
        screen.contains("14s"),
        "elapsed shown in the badge:\n{screen}"
    );
}

/// The placeholder row has no session behind it: it must not be selectable, and
/// the session-list indices must stay a valid range over the real sessions.
#[test]
fn spawn_placeholder_row_is_not_selectable() {
    let mut h = Harness::standard(2);
    h.app.pending_spawn = Some(PendingSpawn::new("feat/x", SpawnPhase::Spawning));
    h.render();

    let max_session_index = h
        .app
        .click_targets
        .iter()
        .filter_map(|t| match t.action {
            ClickAction::SelectSession(i) => Some(i),
            _ => None,
        })
        .max();
    assert_eq!(
        max_session_index,
        Some(1),
        "only the 2 real sessions are clickable — the placeholder records no hitbox"
    );
    assert_invariants(&h.app, "placeholder row present");
}

/// A session being created belongs under the header of the repo it is being cut
/// from — appending it below every group made a new `webapp` session look like
/// it was landing somewhere else entirely.
#[test]
fn spawn_placeholder_renders_inside_its_repo_group() {
    let mut h = Harness::standard(2);
    h.app.sessions[0].info.repo_display_names = vec!["webapp".to_string()];
    h.app.sessions[1].info.repo_display_names = vec!["infra".to_string()];

    let mut pending = PendingSpawn::new("feat/x", SpawnPhase::Spawning);
    pending.repo_display_names = vec!["webapp".to_string()];
    h.app.pending_spawn = Some(pending);

    let screen = h.render();
    let line_of = |needle: &str| {
        screen
            .lines()
            .position(|l| l.contains(needle))
            .unwrap_or_else(|| panic!("{needle:?} missing from:\n{screen}"))
    };

    // Groups render infra-then-webapp (label order); the placeholder must sit
    // in the webapp run, i.e. below the webapp header and after its session.
    assert!(
        line_of("webapp") < line_of("feat/x"),
        "placeholder sits under the webapp header:\n{screen}"
    );
    assert!(
        line_of("infra") < line_of("webapp"),
        "sanity: infra group precedes webapp:\n{screen}"
    );
    assert_invariants(&h.app, "placeholder inside its repo group");
}

/// …and a repo with no sessions yet gets its own header, so the row is filed
/// under a label rather than floating loose at the bottom.
#[test]
fn spawn_placeholder_for_a_new_repo_brings_its_own_header() {
    let mut h = Harness::standard(1);
    h.app.sessions[0].info.repo_display_names = vec!["webapp".to_string()];

    let mut pending = PendingSpawn::new("feat/x", SpawnPhase::Spawning);
    pending.repo_display_names = vec!["brand-new-repo".to_string()];
    h.app.pending_spawn = Some(pending);

    let screen = h.render();
    assert!(
        screen.contains("brand-new-repo"),
        "the new group is labelled:\n{screen}"
    );
    assert_invariants(&h.app, "placeholder opening a new repo group");
}

/// With no sessions at all, the placeholder must replace the "No sessions yet"
/// empty state — otherwise the very first `Ctrl+N` shows nothing happening.
#[test]
fn spawn_placeholder_replaces_the_empty_state() {
    let mut h = Harness::standard(0);
    assert!(h.render().contains("No sessions yet"));

    h.app.pending_spawn = Some(PendingSpawn::new("thurbox", SpawnPhase::Branches));

    let screen = h.render();
    assert!(
        !screen.contains("No sessions yet"),
        "the empty state gives way to the session being created:\n{screen}"
    );
    assert!(screen.contains("Fetching branches…"), "{screen}");
}

#[test]
fn pending_delete_finalizes_after_undo_window() {
    let mut h = Harness::standard(2);
    h.ctrl('d'); // DeleteSession (soft) — starts the undo window
    assert!(h.app.pending_delete.is_some());

    // Inside the window the delete stays pending (undoable).
    h.advance(UNDO_TIMEOUT - std::time::Duration::from_secs(1))
        .tick();
    assert!(
        h.app.pending_delete.is_some(),
        "still undoable inside the window"
    );

    h.advance(std::time::Duration::from_secs(2)).tick();
    assert!(
        h.app.pending_delete.is_none(),
        "the expired window finalizes the delete"
    );

    h.ctrl('z'); // UndoDelete — too late now
    assert_eq!(
        h.app.sessions.len(),
        1,
        "a finalized delete can no longer be undone"
    );
}

#[test]
fn global_search_content_scan_waits_for_debounce() {
    let mut h = Harness::standard(2);
    h.feed_output(1, b"a unique zebra-crossing appears\r\n");

    h.ctrl('/'); // GlobalSearch
    for c in "zebra".chars() {
        h.key(KeyCode::Char(c), KeyModifiers::NONE);
    }
    let content_hit = |app: &App| {
        app.global_search
            .results
            .iter()
            .any(|r| r.kind == search::SearchKind::Session && r.snippet.is_some())
    };

    h.tick();
    assert!(
        !content_hit(&h.app),
        "the expensive buffer scan is debounced — no content hit immediately"
    );
    assert!(h.app.global_search.content_dirty, "a scan is pending");

    h.advance(std::time::Duration::from_millis(
        search::CONTENT_DEBOUNCE_MS + 10,
    ))
    .tick();
    assert!(
        content_hit(&h.app),
        "once the query settles, the tick scans session buffers"
    );
    assert!(!h.app.global_search.content_dirty);
}

#[test]
fn forced_redraw_floor_repaints_after_interval() {
    let mut h = Harness::standard(1);
    h.app.mark_redrawn();
    assert!(
        !h.app.should_redraw(),
        "clean state right after a paint — no redraw needed"
    );

    h.advance(FORCE_REDRAW_INTERVAL);
    assert!(
        h.app.should_redraw(),
        "the forced-redraw floor repaints time-driven UI"
    );
}

// ── Regressions the monkey test originally caught ────────────────────────────

#[test]
fn global_search_on_short_terminal_does_not_panic_session_resize() {
    // The search strip + footer can eat a short terminal's entire height,
    // producing a zero-row content area. `Session::resize` must clamp before
    // vt100's `set_size` (which underflows on 0) — this panicked pre-clamp.
    let mut h = Harness::new(30, 8, 1);
    h.render();
    h.ctrl('/'); // GlobalSearch — resizes sessions to the shrunken content area
    h.render();
    assert!(h.app.global_search.active);
}

#[test]
fn narrow_resize_rescues_task_editor_focus() {
    // Shrinking below 120 cols hides the tasks panel; focus must leave the
    // *editor* too, or it keeps capturing every key for an invisible surface.
    let mut h = Harness::standard(1);
    h.ctrl('w'); // FocusTasks
    h.key(KeyCode::Char('n'), KeyModifiers::NONE); // new task → TaskEditor
    assert!(matches!(h.app.focus, InputFocus::TaskEditor));

    h.resize(100, 40);
    assert!(!h.app.show_tasks_panel, "narrow layout hides the panel");
    assert!(
        matches!(h.app.focus, InputFocus::SessionList),
        "focus is rescued off the hidden panel's editor"
    );
    h.render();
}

// ── Injected agent output: the PTY seam ──────────────────────────────────────

#[test]
fn injected_output_marks_redraw_and_renders() {
    let mut h = Harness::standard(1);
    // Sync the output-change detector, then settle to a clean state.
    h.app.detect_output_redraw();
    h.app.mark_redrawn();
    h.app.detect_output_redraw();
    assert!(!h.app.should_redraw(), "no new output ⇒ no repaint");

    h.feed_output(0, b"MARKER-7f3a output line\r\n");
    h.app.detect_output_redraw();
    assert!(h.app.should_redraw(), "new output marks the UI dirty");

    let screen = h.render();
    assert!(
        screen.contains("MARKER-7f3a"),
        "injected output reaches the rendered terminal pane:\n{screen}"
    );
}

#[test]
fn osc_title_and_bell_reach_the_session() {
    let mut h = Harness::standard(1);

    h.feed_output(0, b"\x1b]0;Reticulating splines\x07");
    assert_eq!(
        h.app.sessions[0].agent_title().as_deref(),
        Some("Reticulating splines"),
        "an OSC 0 title lands in the session's activity text"
    );

    assert!(!h.app.sessions[0].needs_attention());
    h.feed_output(0, b"\x07");
    assert!(
        h.app.sessions[0].needs_attention(),
        "a BEL raises the attention flag"
    );
}

#[test]
fn split_utf8_output_chunks_render_intact() {
    // The reader loop protects vt100 from mid-codepoint chunks with a carry
    // buffer; `feed_output` bypasses the reader, so this documents that a test
    // feeding whole-codepoint chunks renders multi-byte text correctly (the
    // carry logic itself is unit-tested via `utf8_ready_prefix_len`).
    let mut h = Harness::standard(1);
    h.feed_output(0, "boîte — ünïcode ✓\r\n".as_bytes());
    let screen = h.render();
    assert!(
        screen.contains("boîte — ünïcode ✓"),
        "multi-byte output renders intact:\n{screen}"
    );
}

// ── Ctrl+O editor: terminal vs GUI routing ─────────────────────────────

#[test]
fn terminal_editor_stages_pending_run_for_main_loop() {
    // `ttt` is a known terminal editor, so Ctrl+O must NOT fire a null-stdio
    // spawn (which would die with no TTY). Instead it stages an invocation for
    // the main loop to run with a real TTY (popup/suspend).
    let mut h = Harness::standard(0);
    h.app.db.set_editor_command("ttt").unwrap();
    h.app.launch_editor(
        &[std::path::PathBuf::from("/tmp/repo")],
        Some("paths".to_string()),
    );
    let inv = h
        .app
        .take_pending_editor_run()
        .expect("ttt stages a terminal-editor run");
    assert_eq!(inv.program, "ttt");
    assert_eq!(inv.args, ["/tmp/repo".to_string()]);
    // One-shot: a second drain yields nothing.
    assert!(h.app.take_pending_editor_run().is_none());
}

#[test]
fn editor_mode_terminal_forces_tty_even_for_a_gui_editor() {
    // `code` is normally GUI (detached), but `editor mode terminal` overrides:
    // it must stage a terminal run too, keeping extra flags before the paths.
    let mut h = Harness::standard(0);
    h.app.db.set_editor_command("code --wait").unwrap();
    h.app
        .db
        .set_editor_mode(crate::session::settings::EditorMode::Terminal)
        .unwrap();
    h.app.launch_editor(
        &[
            std::path::PathBuf::from("/tmp/repo"),
            std::path::PathBuf::from("/other"),
        ],
        Some("paths".to_string()),
    );
    let inv = h
        .app
        .take_pending_editor_run()
        .expect("terminal mode forces the TTY path even for `code`");
    assert_eq!(inv.program, "code");
    assert_eq!(
        inv.args,
        [
            "--wait".to_string(),
            "/tmp/repo".to_string(),
            "/other".to_string()
        ]
    );
}

// ── Invariant tripwires + deterministic monkey test ──────────────────────────

/// Structural invariants that must hold after *any* event, in any order. The
/// monkey test checks these after every step; when a "weird TUI behavior" is
/// reduced to a rule ("focus never rests on a hidden pane"), add it here and
/// the monkey hunts for a sequence that breaks it.
fn assert_invariants(app: &App, ctx: &str) {
    assert!(
        app.sessions.is_empty() || app.active_index < app.sessions.len(),
        "[{ctx}] active_index {} out of bounds ({} sessions)",
        app.active_index,
        app.sessions.len()
    );
    assert!(
        app.task_ui.filtered_task_indices.is_empty()
            || app.task_ui.task_panel_index < app.task_ui.filtered_task_indices.len(),
        "[{ctx}] task panel selection out of bounds"
    );
    assert!(
        app.automation_ui.cached_automations.is_empty()
            || app.automation_ui.automation_panel_index
                < app.automation_ui.cached_automations.len(),
        "[{ctx}] automation pane selection out of bounds"
    );

    // Panel visibility never outlives its feature flag.
    assert!(
        !app.show_tasks_panel || app.features.tasks,
        "[{ctx}] tasks panel shown with the feature disabled"
    );
    assert!(
        !app.show_file_viewer || app.features.file_viewer,
        "[{ctx}] file viewer shown with the feature disabled"
    );

    // Focus only ever rests on a surface that exists.
    match app.focus {
        #[cfg(feature = "plugins")]
        InputFocus::PluginPane => {}
        InputFocus::TaskList | InputFocus::TaskEditor => assert!(
            app.features.tasks && app.show_tasks_panel,
            "[{ctx}] focus {:?} but the tasks panel is hidden",
            app.focus
        ),
        InputFocus::FileViewer => assert!(
            app.show_file_viewer,
            "[{ctx}] focus on a hidden file viewer"
        ),
        InputFocus::GlobalSearch => assert!(
            app.global_search.active,
            "[{ctx}] focus on a closed search strip"
        ),
        InputFocus::CodeReview | InputFocus::ReviewFiles => {
            assert!(
                app.features.code_review,
                "[{ctx}] review focus with the feature disabled"
            );
            assert!(
                app.active_review().is_some(),
                "[{ctx}] focus {:?} but the active session has no open review",
                app.focus
            );
        }
        InputFocus::Automations
        | InputFocus::AutomationEditor
        | InputFocus::AutomationRunHistory => {
            assert!(
                app.features.automations,
                "[{ctx}] automations focus with the feature disabled"
            );
            assert!(
                app.show_session_list,
                "[{ctx}] automations focus but the left column (list) is hidden"
            );
        }
        InputFocus::SessionList => assert!(
            app.show_session_list,
            "[{ctx}] focus {:?} but the session list is hidden",
            app.focus
        ),
        InputFocus::Terminal => {}
    }

    if app.global_search.active {
        assert!(
            app.features.global_search,
            "[{ctx}] search strip active with the feature disabled"
        );
    }
}

/// Deterministic pseudo-random stream (an LCG — no dev-dependency, and a
/// failing seed reproduces exactly).
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.0 >> 33
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
}

/// `Ctrl` chords the monkey may press. Excluded on purpose:
/// `n` (repo picker `Enter` can reach real-git branch listing on the dev
/// machine), `o` (spawns `$EDITOR`), `v` (reads the system clipboard),
/// `q` (quit is a terminal state with nothing to fuzz behind it).
const MONKEY_CTRL: &[char] = &[
    'a', 'b', 'c', 'd', 'e', 'f', 'g', 'h', 'j', 'k', 'l', 'p', 'r', 's', 't', 'u', 'w', 'x', 'y',
    'z', '/', ',',
];

/// Plain (unmodified) keys the monkey may press: pane-scoped letters, digits,
/// and the structural navigation/editing keys.
const MONKEY_KEYS: &[KeyCode] = &[
    KeyCode::Char('j'),
    KeyCode::Char('k'),
    KeyCode::Char('h'),
    KeyCode::Char('l'),
    KeyCode::Char('g'),
    KeyCode::Char('r'),
    KeyCode::Char('d'),
    KeyCode::Char('e'),
    KeyCode::Char('n'),
    KeyCode::Char('s'),
    KeyCode::Char('w'),
    KeyCode::Char('y'),
    KeyCode::Char('/'),
    KeyCode::Char(' '),
    KeyCode::Char('1'),
    KeyCode::Char('9'),
    KeyCode::Esc,
    KeyCode::Enter,
    KeyCode::Tab,
    KeyCode::Backspace,
    KeyCode::Up,
    KeyCode::Down,
    KeyCode::Left,
    KeyCode::Right,
    KeyCode::PageUp,
    KeyCode::PageDown,
    KeyCode::Home,
    KeyCode::End,
];

/// Realistic agent-output chunks: plain text, SGR colour, OSC title, BEL,
/// OSC 9 notification, unicode, screen clears, and alt-screen flips.
const MONKEY_OUTPUT: &[&[u8]] = &[
    b"compiling foo v0.1.0\r\n",
    b"\x1b[31merror\x1b[0m: something\r\n",
    b"\x1b]0;Agent thinking\x07",
    b"\x07",
    b"\x1b]9;needs input\x07",
    "héllo wörld — \u{2714}\r\n".as_bytes(),
    b"\x1b[2J\x1b[H",
    b"\x1b[?1049h",
    b"\x1b[?1049l",
];

/// Monkey test: drive a real `App` with thousands of pseudo-random events —
/// keys, chords, ticks, clock jumps, resizes, mouse, and injected agent
/// output — rendering after every step and asserting [`assert_invariants`].
/// This is the net for "weird TUI behavior": any panic (in update *or* view)
/// or invariant violation fails with the seed + step for exact replay.
/// Tokio flavor: spawn-adjacent flows (fork/restart on the inert backend) may
/// touch the runtime before erroring.
#[tokio::test]
async fn monkey_random_events_uphold_invariants() {
    for seed in [0xDEADBEEFu64, 42, 20260707] {
        let mut rng = Rng(seed);
        let mut h = Harness::standard(3);
        h.render();

        for step in 0..2500 {
            let ctx = format!("seed {seed:#x} step {step}");
            match rng.below(100) {
                // Plain keys: letters, digits, and structural keys.
                0..=39 => {
                    let code = MONKEY_KEYS[rng.below(MONKEY_KEYS.len())];
                    h.key(code, KeyModifiers::NONE);
                }
                // Ctrl chords (thurbox's global namespace).
                40..=59 => {
                    let c = MONKEY_CTRL[rng.below(MONKEY_CTRL.len())];
                    h.ctrl(c);
                }
                // Shift chords (reorder/sort) and F-keys.
                60..=69 => {
                    if rng.below(2) == 0 {
                        let c = ['j', 'k', 's', 'd'][rng.below(4)];
                        h.shift(c);
                    } else {
                        h.func((rng.below(8) + 1) as u8);
                    }
                }
                // Deterministic tick, sometimes after a clock jump.
                70..=79 => {
                    if rng.below(2) == 0 {
                        let ms = [50, 200, 1_000, 5_000, 11_000][rng.below(5)];
                        h.advance(std::time::Duration::from_millis(ms));
                    }
                    h.tick();
                }
                // Mouse: click / scroll / move at a random point.
                80..=89 => {
                    let size = *h.terminal.backend().buffer().area();
                    let x = (rng.below(size.width.max(1) as usize)) as u16;
                    let y = (rng.below(size.height.max(1) as usize)) as u16;
                    let msg = match rng.below(4) {
                        0 => AppMessage::MouseClick {
                            x,
                            y,
                            modifiers: KeyModifiers::NONE,
                        },
                        1 => AppMessage::MouseScrollUp { x, y },
                        2 => AppMessage::MouseScrollDown { x, y },
                        _ => AppMessage::MouseMove { x, y },
                    };
                    h.app.update(msg);
                }
                // Agent output into a random session.
                90..=94 => {
                    if !h.app.sessions.is_empty() {
                        let idx = rng.below(h.app.sessions.len());
                        let chunk = MONKEY_OUTPUT[rng.below(MONKEY_OUTPUT.len())];
                        h.feed_output(idx, chunk);
                        h.app.detect_output_redraw();
                    }
                }
                // Resize, including below the 80/120 layout breakpoints.
                _ => {
                    let cols = (20 + rng.below(160)) as u16;
                    let rows = (8 + rng.below(43)) as u16;
                    h.resize(cols, rows);
                }
            }

            // Render every step: a draw panic (layout overflow, index OOB in a
            // widget) is as much a bug as an update panic.
            h.render();
            assert_invariants(&h.app, &ctx);
        }
    }
}

#[test]
fn theme_picker_filter_narrows_the_list_and_previews_a_match() {
    // Typing filters the list; the selection lands on the first match and is
    // live-previewed, and `Enter` commits *that* theme (not the entry that
    // happened to share the pre-filter index).
    let mut h = Harness::standard(0);
    h.ctrl('y');
    h.key(KeyCode::Char('/'), KeyModifiers::NONE); // open the filter sub-mode
    for c in "gruvbox".chars() {
        h.key(KeyCode::Char(c), KeyModifiers::NONE);
    }
    let modals::Modal::ThemePicker(ref tp) = h.app.modal else {
        panic!("expected the theme picker");
    };
    let entries = crate::ui::theme::all_theme_entries();
    let names: Vec<&str> = tp
        .matches
        .iter()
        .map(|&i| entries[i].name.as_str())
        .collect();
    assert_eq!(names, vec!["gruvbox-dark", "gruvbox-light"]);
    assert_eq!(tp.index, 0, "selection resets to the first match");
    assert_eq!(
        crate::ui::theme::current(),
        crate::ui::theme::find_theme_entry("gruvbox-dark")
            .unwrap()
            .palette,
        "the first match is previewed"
    );

    h.key(KeyCode::Down, KeyModifiers::NONE);
    h.key(KeyCode::Enter, KeyModifiers::NONE);
    assert_eq!(
        h.app.active_theme.name, "gruvbox-light",
        "Enter commits the selected *match*, not the same-numbered entry"
    );
}

#[test]
fn theme_picker_filter_matching_nothing_keeps_the_modal_usable() {
    let mut h = Harness::standard(0);
    h.ctrl('y');
    h.key(KeyCode::Char('/'), KeyModifiers::NONE); // open the filter sub-mode
    for c in "zzzz".chars() {
        h.key(KeyCode::Char(c), KeyModifiers::NONE);
    }
    let modals::Modal::ThemePicker(ref tp) = h.app.modal else {
        panic!("expected the theme picker");
    };
    assert!(tp.matches.is_empty());
    assert!(tp.selected_entry().is_none());
    // Rendering an empty match set must not panic, and Enter must be a no-op
    // rather than committing a stale index.
    h.render();
    h.key(KeyCode::Enter, KeyModifiers::NONE);
    assert!(!h.app.modal.is_open(), "Enter closes the picker");
    assert_eq!(
        h.app.active_theme.name, "default",
        "no match means nothing is committed"
    );

    // Backspacing back to a real query restores the list.
    let mut h = Harness::standard(0);
    h.ctrl('y');
    h.key(KeyCode::Char('/'), KeyModifiers::NONE); // open the filter sub-mode
    for c in "nordx".chars() {
        h.key(KeyCode::Char(c), KeyModifiers::NONE);
    }
    h.key(KeyCode::Backspace, KeyModifiers::NONE);
    let modals::Modal::ThemePicker(ref tp) = h.app.modal else {
        panic!("expected the theme picker");
    };
    assert_eq!(tp.matches.len(), 1, "'nord' matches exactly one theme");
}

#[test]
fn theme_picker_page_keys_move_by_a_screenful() {
    // PageDown steps by the rendered list height, so a 36-entry list is
    // traversable without holding Down.
    let mut h = Harness::standard(0);
    h.ctrl('y');
    h.render(); // establishes the page height
    let page = h.app.theme_picker_page;
    assert!(page > 1, "the list should render several rows, got {page}");

    h.key(KeyCode::PageDown, KeyModifiers::NONE);
    let modals::Modal::ThemePicker(ref tp) = h.app.modal else {
        panic!("expected the theme picker");
    };
    assert_eq!(tp.index, page);

    h.key(KeyCode::End, KeyModifiers::NONE);
    let modals::Modal::ThemePicker(ref tp) = h.app.modal else {
        panic!("expected the theme picker");
    };
    assert_eq!(
        tp.index,
        crate::ui::theme::all_theme_entries().len() - 1,
        "End jumps to the last theme"
    );

    h.key(KeyCode::Home, KeyModifiers::NONE);
    let modals::Modal::ThemePicker(ref tp) = h.app.modal else {
        panic!("expected the theme picker");
    };
    assert_eq!(tp.index, 0, "Home jumps back to the first");
}

#[test]
fn theme_picker_ctrl_n_p_navigate_and_other_ctrl_chords_dont_type() {
    // Parity with the global-search strip: Ctrl+N/Ctrl+P move the selection.
    // Any *other* Ctrl chord must be swallowed, never inserted as a letter —
    // a stray Ctrl+W would otherwise silently filter the list down to "w".
    let mut h = Harness::standard(0);
    h.ctrl('y');

    h.key(KeyCode::Char('n'), KeyModifiers::CONTROL);
    h.key(KeyCode::Char('n'), KeyModifiers::CONTROL);
    let modals::Modal::ThemePicker(ref tp) = h.app.modal else {
        panic!("expected the theme picker");
    };
    assert_eq!(tp.index, 2, "Ctrl+N moves down");
    assert!(tp.filter_query().is_empty(), "Ctrl+N must not type");

    h.key(KeyCode::Char('p'), KeyModifiers::CONTROL);
    let modals::Modal::ThemePicker(ref tp) = h.app.modal else {
        panic!("expected the theme picker");
    };
    assert_eq!(tp.index, 1, "Ctrl+P moves up");

    h.key(KeyCode::Char('w'), KeyModifiers::CONTROL);
    let modals::Modal::ThemePicker(ref tp) = h.app.modal else {
        panic!("expected the theme picker");
    };
    assert!(
        tp.filter_query().is_empty(),
        "an unhandled Ctrl chord must not leak into the filter"
    );
    assert_eq!(tp.index, 1, "and must not move the selection");
}

#[test]
fn theme_picker_jk_navigate_until_slash_opens_the_filter() {
    // The picker keeps the shared selector keys: `j`/`k` select, and only `/`
    // starts a query. Letters are literal navigation until then, so typing
    // `j` can never silently filter the list.
    let mut h = Harness::standard(0);
    h.ctrl('y');

    h.key(KeyCode::Char('j'), KeyModifiers::NONE);
    h.key(KeyCode::Char('j'), KeyModifiers::NONE);
    let modals::Modal::ThemePicker(ref tp) = h.app.modal else {
        panic!("expected the theme picker");
    };
    assert_eq!(tp.index, 2, "j moves down");
    assert!(tp.filter.is_none(), "j must not open the filter");

    h.key(KeyCode::Char('k'), KeyModifiers::NONE);
    let modals::Modal::ThemePicker(ref tp) = h.app.modal else {
        panic!("expected the theme picker");
    };
    assert_eq!(tp.index, 1, "k moves up");

    // g/G jump to the ends, as in the other list surfaces.
    h.key(KeyCode::Char('G'), KeyModifiers::NONE);
    let modals::Modal::ThemePicker(ref tp) = h.app.modal else {
        panic!("expected the theme picker");
    };
    assert_eq!(tp.index, crate::ui::theme::all_theme_entries().len() - 1);
    h.key(KeyCode::Char('g'), KeyModifiers::NONE);
    let modals::Modal::ThemePicker(ref tp) = h.app.modal else {
        panic!("expected the theme picker");
    };
    assert_eq!(tp.index, 0);

    // `/` switches modes; only now do letters become query text.
    h.key(KeyCode::Char('/'), KeyModifiers::NONE);
    h.key(KeyCode::Char('j'), KeyModifiers::NONE);
    let modals::Modal::ThemePicker(ref tp) = h.app.modal else {
        panic!("expected the theme picker");
    };
    assert_eq!(tp.filter_query(), "j", "after / a letter types");
    // No built-in name contains a `j`, so this also shows the letter really
    // reached the query rather than moving the cursor.
    assert!(tp.matches.is_empty(), "'j' matches no theme name");
}

#[test]
fn theme_picker_esc_closes_filter_first_then_the_modal() {
    // Two Esc levels, like the code-review find: the first leaves the filter
    // sub-mode (restoring the full list), the second cancels the picker.
    let mut h = Harness::standard(0);
    h.ctrl('y');
    h.key(KeyCode::Char('/'), KeyModifiers::NONE);
    for c in "nord".chars() {
        h.key(KeyCode::Char(c), KeyModifiers::NONE);
    }
    let modals::Modal::ThemePicker(ref tp) = h.app.modal else {
        panic!("expected the theme picker");
    };
    assert_eq!(tp.matches.len(), 1, "filtered down to Nord");

    h.key(KeyCode::Esc, KeyModifiers::NONE);
    let modals::Modal::ThemePicker(ref tp) = h.app.modal else {
        panic!("first Esc must keep the picker open");
    };
    assert!(tp.filter.is_none(), "first Esc closes the filter");
    assert_eq!(
        tp.matches.len(),
        crate::ui::theme::all_theme_entries().len(),
        "clearing the filter restores every theme"
    );
    // The cursor stayed on the theme the filter had selected, so leaving the
    // sub-mode doesn't jump the preview somewhere unrelated.
    assert_eq!(
        tp.selected_entry()
            .map(|i| crate::ui::theme::all_theme_entries()[i].name.clone()),
        Some("nord".to_string())
    );

    h.key(KeyCode::Esc, KeyModifiers::NONE);
    assert!(!h.app.modal.is_open(), "second Esc closes the picker");
}

#[test]
fn theme_picker_slash_is_not_query_text() {
    // `/` opens the sub-mode; pressing it again keeps the query rather than
    // inserting a literal slash (no theme name contains one).
    let mut h = Harness::standard(0);
    h.ctrl('y');
    h.key(KeyCode::Char('/'), KeyModifiers::NONE);
    for c in "nord".chars() {
        h.key(KeyCode::Char(c), KeyModifiers::NONE);
    }
    h.key(KeyCode::Char('/'), KeyModifiers::NONE);
    let modals::Modal::ThemePicker(ref tp) = h.app.modal else {
        panic!("expected the theme picker");
    };
    assert_eq!(tp.filter_query(), "nord", "a second / must not type");
}

/// A plugin pane must reach the screen from a cached view tree, without the
/// render path ever calling into a plugin — the whole point of the split
/// between `plugin` (produces trees) and `ui` (draws them).
#[cfg(feature = "plugins")]
#[test]
fn plugin_pane_renders_its_tree_in_a_real_frame() {
    use crate::session::plugin_manifest::PaneSlot;
    use crate::session::view_tree::{StyleToken, TextStyle, ViewNode};

    let mut h = Harness::new(160, 40, 1);
    let mut pane =
        crate::plugin::PluginPane::loading("demo", "board", "Demo", PaneSlot::Right, true);

    // Nothing rendered yet: the pane says so rather than showing an empty box.
    h.app.set_plugin_panes(vec![pane.clone()]);
    let loading = h.render();
    assert!(loading.contains("Demo"), "pane title is drawn:\n{loading}");
    assert!(
        loading.contains("loading"),
        "loading state is drawn:\n{loading}"
    );

    // A successful render replaces it with the plugin's own content.
    pane.apply(Ok(ViewNode::list(vec![
        ViewNode::styled(
            "PLUGIN HEADING",
            TextStyle {
                token: Some(StyleToken::Accent),
                bold: true,
                ..TextStyle::default()
            },
        ),
        ViewNode::Divider,
        ViewNode::text("a plugin drew this"),
    ])));
    h.app.set_plugin_panes(vec![pane.clone()]);
    let drawn = h.render();
    assert!(drawn.contains("PLUGIN HEADING"), "{drawn}");
    assert!(drawn.contains("a plugin drew this"), "{drawn}");
    assert!(!drawn.contains("loading"), "{drawn}");

    // A later failure keeps the last good content and flags the error in the
    // title, rather than blanking the pane.
    pane.apply(Err("render exploded".to_string()));
    h.app.set_plugin_panes(vec![pane]);
    let stale = h.render();
    assert!(
        stale.contains("a plugin drew this"),
        "content must survive a failed render:\n{stale}"
    );
    assert!(stale.contains("error"), "the failure is surfaced:\n{stale}");
}

/// A pane hidden by another process (`thurbox-cli command run
/// <plugin>.<pane>.hide`) has to reach this TUI without a restart. The write is
/// made through the harness's own connection because a second connection to an
/// in-memory database is not reachable; the poll's `data_version` trigger is one
/// line above the call and is exercised by the ordinary sync tests.
#[cfg(feature = "plugins")]
#[test]
fn plugin_pane_visibility_follows_an_external_change() {
    use crate::session::plugin_manifest::PaneSlot;

    let mut h = Harness::new(160, 40, 1);
    let pane = crate::plugin::PluginPane::loading("demo", "board", "Demo", PaneSlot::Right, true);
    h.app.set_plugin_panes(vec![pane]);
    assert_eq!(h.app.visible_plugin_panes(), 1, "seeded visible");

    h.app
        .db
        .set_plugin_pane_visible("demo", "board", false)
        .expect("store the choice");
    assert!(
        h.app.apply_stored_plugin_pane_visibility(),
        "an external hide is a change"
    );
    assert_eq!(h.app.visible_plugin_panes(), 0, "the pane is off screen");
    assert!(
        !h.render().contains("Demo"),
        "a hidden pane must not be drawn"
    );

    // Applying the same store again changes nothing, so the demand-driven loop
    // owes no repaint: one installed plugin must not repaint on every detected
    // database change.
    assert!(!h.app.apply_stored_plugin_pane_visibility());
}

/// Two panes must both reach the screen. The single-slot layout drew only the
/// first visible pane, so a second bundled plugin was invisible however it was
/// configured — the wall the workspace tree exists to remove.
#[cfg(feature = "plugins")]
#[test]
fn two_plugin_panes_both_reach_the_screen() {
    use crate::session::plugin_manifest::PaneSlot;
    use crate::session::view_tree::ViewNode;

    let mut h = Harness::new(200, 40, 1);
    let mut first =
        crate::plugin::PluginPane::loading("alpha", "board", "Alpha", PaneSlot::Right, true);
    let mut second =
        crate::plugin::PluginPane::loading("beta", "board", "Beta", PaneSlot::Right, true);
    first.apply(Ok(ViewNode::text("drawn by alpha")));
    second.apply(Ok(ViewNode::text("drawn by beta")));
    h.app.set_plugin_panes(vec![first, second]);

    let frame = h.render();
    assert!(frame.contains("drawn by alpha"), "{frame}");
    assert!(frame.contains("drawn by beta"), "{frame}");

    // Each got its own region, side by side, and neither overlaps the center.
    let areas = h.app.layout_for(ratatui::layout::Rect::new(0, 0, 200, 40));
    assert_eq!(areas.plugin_panes.len(), 2);
    let (a, b) = (areas.plugin_panes[0], areas.plugin_panes[1]);
    assert_eq!(a.x, areas.terminal.x + areas.terminal.width);
    assert_eq!(b.x, a.x + a.width);
}

/// With one declared pane the bound action is a plain toggle: no picker, and the
/// choice is persisted. This is the shape a stable install with one plugin has,
/// and the reason the picker is not opened unconditionally.
#[cfg(feature = "plugins")]
#[test]
fn plugin_pane_toggle_flips_a_single_pane_without_a_picker() {
    use crate::session::plugin_manifest::PaneSlot;

    let mut h = Harness::new(160, 40, 1);
    h.app
        .set_plugin_panes(vec![crate::plugin::PluginPane::loading(
            "demo",
            "board",
            "Demo",
            PaneSlot::Right,
            false,
        )]);

    h.func(10);
    assert!(
        !h.app.modal.is_open(),
        "one pane leaves nothing to choose, so no picker opens"
    );
    assert_eq!(h.app.visible_plugin_panes(), 1);
    assert_eq!(
        h.app.db.get_plugin_pane_visible("demo", "board").unwrap(),
        Some(true),
        "the choice outranks the manifest seed on the next launch"
    );

    h.func(10);
    assert_eq!(h.app.visible_plugin_panes(), 0);
    assert_eq!(
        h.app.db.get_plugin_pane_visible("demo", "board").unwrap(),
        Some(false)
    );
}

/// The gap this closes: with two declared panes the action reached only the
/// first, so the pane a second bundled plugin declares could not be shown by any
/// key. The picker is how one action reaches N panes.
#[cfg(feature = "plugins")]
#[test]
fn plugin_pane_picker_reaches_a_pane_other_than_the_first() {
    use crate::session::plugin_manifest::PaneSlot;

    let mut h = Harness::new(160, 40, 1);
    h.app.set_plugin_panes(vec![
        crate::plugin::PluginPane::loading("hello", "board", "Hello", PaneSlot::Right, false),
        crate::plugin::PluginPane::loading("info-panel", "info", "Info", PaneSlot::Right, false),
    ]);

    h.func(10);
    let frame = h.render();
    assert!(
        frame.contains("Plugin panes"),
        "the picker opened:\n{frame}"
    );
    assert!(frame.contains("hello.board"), "{frame}");
    assert!(
        frame.contains("info-panel.info"),
        "every declared pane is listed, addressed as the commands address it:\n{frame}"
    );
    assert_eq!(
        h.app.visible_plugin_panes(),
        0,
        "opening the picker changes no visibility"
    );

    // Select the *second* pane and show it: the case the old toggle could not
    // express at all.
    h.key(KeyCode::Char('j'), KeyModifiers::NONE);
    h.key(KeyCode::Char(' '), KeyModifiers::NONE);
    let visible: Vec<bool> = h.app.plugin_panes.iter().map(|p| p.visible).collect();
    assert_eq!(visible, vec![false, true], "only the second pane moved");
    assert_eq!(
        h.app
            .db
            .get_plugin_pane_visible("info-panel", "info")
            .unwrap(),
        Some(true),
        "the picker stores exactly what the pane's own hide/show command stores"
    );
    assert_eq!(
        h.app.db.get_plugin_pane_visible("hello", "board").unwrap(),
        None,
        "a pane nobody touched keeps no stored choice"
    );

    // `Space` keeps the picker open, so both panes can be turned on in one visit,
    // and the row it redraws reflects what actually happened.
    assert!(h.app.modal.is_open());
    let crate::app::modals::Modal::PluginPanes(ref m) = h.app.modal else {
        panic!("the picker is still open");
    };
    assert!(m.rows[1].visible, "the toggled row shows its new state");

    // `Esc` leaves without changing anything further.
    h.key(KeyCode::Esc, KeyModifiers::NONE);
    assert!(!h.app.modal.is_open());
    assert_eq!(h.app.visible_plugin_panes(), 1);
}

/// `Enter` is the "show this one and get out of the way" gesture, and the bound
/// action closes the picker again like every other self-toggling overlay.
#[cfg(feature = "plugins")]
#[test]
fn plugin_pane_picker_enter_toggles_and_closes() {
    use crate::session::plugin_manifest::PaneSlot;

    let mut h = Harness::new(160, 40, 1);
    h.app.set_plugin_panes(vec![
        crate::plugin::PluginPane::loading("a", "one", "A", PaneSlot::Right, false),
        crate::plugin::PluginPane::loading("b", "two", "B", PaneSlot::Right, false),
    ]);

    h.func(10);
    h.key(KeyCode::Enter, KeyModifiers::NONE);
    assert!(!h.app.modal.is_open(), "Enter closes the picker");
    assert_eq!(h.app.visible_plugin_panes(), 1);

    // Re-pressing the opener dismisses it, as with the theme picker.
    h.func(10);
    assert!(h.app.modal.is_open());
    h.func(10);
    assert!(!h.app.modal.is_open());
    assert_eq!(
        h.app.visible_plugin_panes(),
        1,
        "dismissing changes nothing"
    );
}

/// Every pane thurbox *ships* must be reachable from the bound action — the
/// property `migration/phase-4` now requires, driven over the real bundled
/// plugins rather than synthetic panes, because the gap it closes was invisible
/// until a second bundled pane existed. As later Phase 4 panes land, this keeps
/// asserting the same thing about each of them.
#[cfg(feature = "plugins")]
#[test]
fn every_bundled_pane_is_reachable_from_the_keyboard() {
    use crate::session::pane_visibility as pv;

    let _guard = pv::test_lock();
    pv::clear_for_test();

    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/plugin/bundled");
    let mut dirs: Vec<std::path::PathBuf> = std::fs::read_dir(&root)
        .expect("the bundled plugin sources ship in the tree")
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.join("plugin.toml").is_file())
        .collect();
    dirs.sort();
    let outcome = crate::plugin::discovery::discover_in(&dirs, None);
    assert!(
        outcome.problems.is_empty(),
        "every bundled plugin must load: {:?}",
        outcome.problems
    );
    let mut host = crate::plugin::PluginHost::from_discovery(
        outcome,
        crate::plugin::ExecutionBounds::default(),
    );
    host.start_all();
    let panes = host.panes();
    assert!(
        panes.len() >= 2,
        "thurbox ships more than one pane, which is the case the picker exists for"
    );

    let mut h = Harness::new(160, 40, 1);
    h.app.set_plugin_panes(panes.clone());
    h.func(10);
    let frame = h.render();
    for pane in &panes {
        assert!(
            frame.contains(&format!("{}.{}", pane.plugin, pane.id)),
            "{}.{} must be listed:\n{frame}",
            pane.plugin,
            pane.id
        );
    }

    // Each of them, in turn, can be shown or hidden from that one key.
    for (i, pane) in panes.iter().enumerate() {
        let before = h
            .app
            .plugin_pane_visible(&pane.plugin, &pane.id)
            .expect("the pane is declared");
        for _ in 0..panes.len() {
            h.key(KeyCode::Char('k'), KeyModifiers::NONE);
        }
        for _ in 0..i {
            h.key(KeyCode::Char('j'), KeyModifiers::NONE);
        }
        h.key(KeyCode::Char(' '), KeyModifiers::NONE);
        assert_eq!(
            h.app.plugin_pane_visible(&pane.plugin, &pane.id),
            Some(!before),
            "{}.{} did not answer the keyboard",
            pane.plugin,
            pane.id
        );
    }

    host.stop_all();
    pv::clear_for_test();
}

/// A plugin reload replaces the whole pane set, and it can land while the picker
/// is open — so a row addresses its pane by name, never by list position.
#[cfg(feature = "plugins")]
#[test]
fn plugin_pane_picker_toggles_the_pane_it_names_after_a_reload() {
    use crate::session::plugin_manifest::PaneSlot;

    let mut h = Harness::new(160, 40, 1);
    let a = crate::plugin::PluginPane::loading("a", "one", "A", PaneSlot::Right, false);
    let b = crate::plugin::PluginPane::loading("b", "two", "B", PaneSlot::Right, false);
    h.app.set_plugin_panes(vec![a.clone(), b.clone()]);

    h.func(10);
    h.key(KeyCode::Char('j'), KeyModifiers::NONE);

    // A reload publishes the same panes in the other order; the selected row
    // still names `b.two`.
    h.app.set_plugin_panes(vec![b, a]);
    h.key(KeyCode::Char(' '), KeyModifiers::NONE);

    assert_eq!(
        h.app.plugin_pane_visible("b", "two"),
        Some(true),
        "the pane the row names is the one that moved"
    );
    assert_eq!(h.app.plugin_pane_visible("a", "one"), Some(false));
}

/// The action is bound whether or not a plugin is installed, so with no declared
/// pane it must do nothing at all — not open an empty picker.
#[cfg(feature = "plugins")]
#[test]
fn plugin_pane_toggle_with_no_panes_is_silent() {
    let mut h = Harness::new(160, 40, 1);
    h.func(10);
    assert!(!h.app.modal.is_open());
    assert!(h.app.status_message.is_none(), "and raises no error");
}

/// The render worker must be told which panes to skip, and told *only* when the
/// answer moved: the publication runs on the tick, so a per-tick write would put
/// a lock and a clone on the idle loop for every install with a plugin pane.
#[cfg(feature = "plugins")]
#[test]
fn plugin_pane_visibility_publication_is_change_gated() {
    use crate::session::pane_visibility as pv;
    use crate::session::plugin_manifest::PaneSlot;

    let _guard = pv::test_lock();
    pv::clear_for_test();

    let mut h = Harness::new(160, 40, 1);
    h.app.set_plugin_panes(vec![
        crate::plugin::PluginPane::loading("a", "one", "A", PaneSlot::Right, true),
        crate::plugin::PluginPane::loading("b", "two", "B", PaneSlot::Right, false),
    ]);

    // Nothing is published while no running plugin declares a pane: a build with
    // no plugin host pays one atomic load per tick and returns.
    for _ in 0..5 {
        h.tick();
    }
    assert_eq!(h.app.perf_counters().pane_visibility_publishes, 0);
    assert!(pv::hidden().is_empty());

    pv::set_panes_present(true);
    h.tick();
    assert_eq!(h.app.perf_counters().pane_visibility_publishes, 1);
    assert!(pv::is_hidden("b", "two"), "the hidden pane is published");
    assert!(!pv::is_hidden("a", "one"), "the visible one is not");

    // Steady state: the set has not moved, so nothing is written.
    for _ in 0..20 {
        h.tick();
    }
    assert_eq!(
        h.app.perf_counters().pane_visibility_publishes,
        1,
        "an unchanged hidden set costs no publication"
    );

    // A toggle moves it, and exactly one publication follows.
    h.app.set_plugin_pane_visible("a", "one", false);
    h.tick();
    assert_eq!(h.app.perf_counters().pane_visibility_publishes, 2);
    assert!(pv::is_hidden("a", "one"));
    assert!(pv::is_hidden("b", "two"));

    pv::clear_for_test();
}

/// A pane the user has never chosen for keeps whatever the manifest seeded, so
/// applying stored visibility is not a way for the store to erase a seed.
#[cfg(feature = "plugins")]
#[test]
fn plugin_pane_visibility_leaves_an_unstored_pane_alone() {
    use crate::session::plugin_manifest::PaneSlot;

    let mut h = Harness::new(160, 40, 1);
    h.app.set_plugin_panes(vec![
        crate::plugin::PluginPane::loading("a", "shown", "A", PaneSlot::Right, true),
        crate::plugin::PluginPane::loading("b", "hidden", "B", PaneSlot::Right, false),
    ]);

    assert!(!h.app.apply_stored_plugin_pane_visibility());
    let visible: Vec<bool> = h.app.plugin_panes.iter().map(|p| p.visible).collect();
    assert_eq!(visible, vec![true, false]);
}

/// Reduced motion is whole-app, not plugin-only: thurbox's own working spinner
/// holds a single glyph too. This is what makes the setting honest in a build
/// with no plugin host at all.
#[test]
fn reduced_motion_freezes_thurboxs_own_spinner() {
    let mut h = Harness::standard(1);
    for _ in 0..40 {
        h.tick();
    }
    assert_ne!(h.app.spinner_frame(), 0, "the spinner advances by default");

    let mut h = Harness::standard(1);
    h.app.motion_settings.reduce_motion = true;
    for _ in 0..40 {
        h.tick();
    }
    assert_eq!(
        h.app.spinner_frame(),
        0,
        "reduced motion holds a single glyph"
    );
}

/// Turning reduced motion on mid-spin must land on the same glyph as having had
/// it on since boot — the setting promises the *first* frame, so a spinner that
/// froze wherever it happened to be would contradict the row that toggles it.
#[test]
fn reduced_motion_settles_a_spinning_glyph_onto_its_first_frame() {
    let mut h = Harness::standard(1);
    for _ in 0..40 {
        h.tick();
    }
    assert_ne!(h.app.spinner_frame(), 0);

    let mut settings = crate::session::settings::Settings::default();
    settings.motion.reduce_motion = true;
    h.app.apply_live_settings(&settings);
    h.tick();
    assert_eq!(h.app.spinner_frame(), 0, "it settles on the first frame");

    // And then stays there without asking for further repaints.
    let before = h.app.perf_counters().redraws_requested;
    for _ in 0..40 {
        h.tick();
    }
    assert_eq!(h.app.spinner_frame(), 0);
    assert_eq!(
        h.app.perf_counters().redraws_requested,
        before,
        "a frozen spinner requests no repaints"
    );
}

/// `docs/CONFIG.md` promises reduced motion applies live, with no restart. That
/// promise is a wiring detail — `apply_live_settings` mirroring `[motion]` onto
/// the app — and the two reduced-motion tests here poke `motion_settings`
/// directly, so they would still pass if the wiring were dropped. This is the
/// test that would not.
#[test]
fn reduced_motion_applies_live_without_a_restart() {
    let mut h = Harness::standard(1);
    assert!(!h.app.motion_settings.reduce_motion, "off by default");

    let mut settings = crate::session::settings::Settings::default();
    settings.motion.reduce_motion = true;
    h.app.apply_live_settings(&settings);
    assert!(h.app.motion_settings.reduce_motion);

    // And it is a live setting, not one the panel must report as restart-only.
    assert!(
        !crate::session::settings::Settings::default().restart_only_differs(&settings),
        "a motion change must not be reported as needing a restart"
    );
}

/// Build a pane whose tree animates `frames` frames at `fps`, under a stable id.
#[cfg(feature = "plugins")]
fn animated_pane(visible: bool, fps: u8, frames: usize) -> crate::plugin::PluginPane {
    use crate::session::motion::Motion;
    use crate::session::plugin_manifest::PaneSlot;
    use crate::session::view_tree::ViewNode;

    let mut pane =
        crate::plugin::PluginPane::loading("demo", "board", "Demo", PaneSlot::Right, visible);
    pane.apply(Ok(ViewNode::Motion {
        key: "spinner".to_string(),
        keyed_by_id: true,
        motion: Motion::cycle(
            (0..frames)
                .map(|i| ViewNode::text(format!("frame-{i}")))
                .collect(),
            fps,
            true,
        ),
    }));
    pane
}

/// The exit criterion for declared motion: an animated pane repaints at the
/// rate it declared, not at the tick loop's. The tick loop runs ~100×/s; an
/// 8 fps cycle must cost ~8 paints per simulated second, not ~100.
///
/// Asserted on `motion_frames` — a wall-clock-free counter — so this measures
/// the property rather than the machine (ADR-P2).
#[cfg(feature = "plugins")]
#[test]
fn motion_repaints_at_its_declared_rate_not_the_tick_rate() {
    let mut h = Harness::new(160, 40, 1);
    h.app.set_plugin_panes(vec![animated_pane(true, 8, 4)]);

    // One simulated second of a 10 ms tick loop.
    for _ in 0..100 {
        h.advance(std::time::Duration::from_millis(10));
        h.tick();
    }

    let frames = h.app.perf_counters().motion_frames;
    assert!(
        (7..=10).contains(&frames),
        "an 8 fps animation must cost ~8 paints per second, not one per tick: {frames}"
    );
    assert_eq!(
        h.app.perf_counters().motion_leases,
        1,
        "one pane, one lease"
    );
}

/// A hidden animated pane must cost exactly nothing — the case a plugin cannot
/// detect for itself, and the reason motion is declared rather than pushed.
#[cfg(feature = "plugins")]
#[test]
fn a_hidden_animated_pane_leaves_the_idle_loop_untouched() {
    let mut h = Harness::new(160, 40, 1);

    // Baseline: an idle app with no plugin at all.
    for _ in 0..100 {
        h.advance(std::time::Duration::from_millis(10));
        h.tick();
    }
    let idle_redraws = h.app.perf_counters().redraws_requested;

    let mut h = Harness::new(160, 40, 1);
    h.app.set_plugin_panes(vec![animated_pane(false, 30, 8)]);
    for _ in 0..100 {
        h.advance(std::time::Duration::from_millis(10));
        h.tick();
    }

    assert_eq!(
        h.app.perf_counters().motion_frames,
        0,
        "a hidden pane never animates"
    );
    assert_eq!(h.app.perf_counters().motion_leases, 0, "and holds no lease");
    assert!(
        h.app.perf_counters().motion_denied > 0,
        "and the counters say why"
    );
    assert_eq!(
        h.app.perf_counters().redraws_requested,
        idle_redraws,
        "the idle paint rate is identical to having no animation at all"
    );
}

/// The rule that would otherwise pin every plugin's spinner to frame 0: a
/// plugin re-rendering on unrelated state must not restart its animation.
#[cfg(feature = "plugins")]
#[test]
fn an_identical_re_push_continues_the_animation() {
    let mut h = Harness::new(160, 40, 1);
    h.app.set_plugin_panes(vec![animated_pane(true, 8, 4)]);
    h.tick();
    assert!(h.render().contains("frame-0"));

    h.advance(std::time::Duration::from_millis(250));
    // Exactly the tree that was already there, pushed again.
    h.app.set_plugin_panes(vec![animated_pane(true, 8, 4)]);
    h.tick();

    let drawn = h.render();
    assert!(
        drawn.contains("frame-2"),
        "the animation must continue, not restart:\n{drawn}"
    );
}

/// Reduced motion is the accessibility switch, and it is whole-app: a declared
/// animation renders its first frame and stops costing repaints entirely.
#[cfg(feature = "plugins")]
#[test]
fn reduced_motion_pins_every_animation_to_its_first_frame() {
    let mut h = Harness::new(160, 40, 1);
    h.app.motion_settings.reduce_motion = true;
    h.app.set_plugin_panes(vec![animated_pane(true, 8, 4)]);

    for _ in 0..100 {
        h.advance(std::time::Duration::from_millis(10));
        h.tick();
    }

    let drawn = h.render();
    assert!(
        drawn.contains("frame-0"),
        "frame 0 is the only frame a reduced-motion user sees:\n{drawn}"
    );
    assert_eq!(h.app.perf_counters().motion_frames, 0);
    assert_eq!(h.app.perf_counters().motion_leases, 0);
    assert!(h.app.perf_counters().motion_denied > 0);
}

/// With no plugin panes the layout must be byte-identical to a build that has
/// no plugin host at all — the guarantee that installing nothing costs nothing.
#[cfg(feature = "plugins")]
#[test]
fn no_plugin_panes_means_no_layout_change() {
    let mut h = Harness::new(160, 40, 1);
    let before = h.render();
    h.app.set_plugin_panes(Vec::new());
    assert_eq!(h.render(), before);
    assert_eq!(h.app.visible_plugin_panes(), 0);
}

/// A pane whose tree does not change must not dirty the UI: one installed
/// plugin cannot be allowed to return the app to painting every tick.
#[cfg(feature = "plugins")]
#[test]
fn an_unchanged_plugin_tree_does_not_request_a_repaint() {
    use crate::session::plugin_manifest::PaneSlot;
    use crate::session::view_tree::ViewNode;

    let mut h = Harness::new(160, 40, 1);
    let mut pane =
        crate::plugin::PluginPane::loading("demo", "board", "Demo", PaneSlot::Right, true);
    pane.apply(Ok(ViewNode::text("steady")));

    assert!(
        h.app.set_plugin_panes(vec![pane.clone()]),
        "first set changes"
    );
    assert!(
        !h.app.set_plugin_panes(vec![pane]),
        "an identical pane set must not report a change"
    );
}

/// The pane toggle is kernel state: it hides and shows, persists the choice,
/// and a hidden pane leaves a layout identical to having no plugin at all.
#[cfg(feature = "plugins")]
#[test]
fn plugin_pane_toggles_and_persists() {
    use crate::session::plugin_manifest::PaneSlot;
    use crate::session::view_tree::ViewNode;

    let mut h = Harness::new(160, 40, 1);
    let mut pane =
        crate::plugin::PluginPane::loading("demo", "board", "Demo", PaneSlot::Right, true);
    pane.apply(Ok(ViewNode::text("PLUGIN BODY")));
    h.app.set_plugin_panes(vec![pane]);

    let shown = h.render();
    assert!(shown.contains("PLUGIN BODY"), "{shown}");

    h.app.toggle_plugin_pane();
    let hidden = h.render();
    assert!(
        !hidden.contains("PLUGIN BODY"),
        "toggle must hide it:\n{hidden}"
    );
    assert_eq!(h.app.visible_plugin_panes(), 0);

    // The choice is persisted, so re-publishing the same pane set (as the
    // render worker does every cycle) must not resurrect it.
    let mut fresh =
        crate::plugin::PluginPane::loading("demo", "board", "Demo", PaneSlot::Right, true);
    fresh.apply(Ok(ViewNode::text("PLUGIN BODY")));
    h.app.set_plugin_panes(vec![fresh]);
    assert!(
        h.app.visible_plugin_panes() == 0,
        "a stored choice must outrank the manifest seed"
    );

    h.app.toggle_plugin_pane();
    assert_eq!(
        h.app.visible_plugin_panes(),
        1,
        "toggling back shows it again"
    );
}

/// A hidden plugin pane must cost no layout space at all.
#[cfg(feature = "plugins")]
#[test]
fn a_hidden_plugin_pane_leaves_the_layout_untouched() {
    use crate::session::plugin_manifest::PaneSlot;

    let mut h = Harness::new(160, 40, 1);
    let baseline = h.render();

    let pane = crate::plugin::PluginPane::loading("demo", "board", "Demo", PaneSlot::Right, false);
    h.app.set_plugin_panes(vec![pane]);

    assert_eq!(h.app.visible_plugin_panes(), 0);
    assert_eq!(h.render(), baseline);
}

/// Toggling with nothing installed is a no-op, not an error — the key is bound
/// whether or not a plugin declares a pane.
#[cfg(feature = "plugins")]
#[test]
fn toggling_with_no_plugin_pane_is_a_no_op() {
    let mut h = Harness::new(160, 40, 1);
    let before = h.render();
    h.app.toggle_plugin_pane();
    assert_eq!(h.render(), before);
}

/// Focus reaches a plugin pane only when its plugin asked for input — a
/// read-only pane stays visible but unfocusable, so cycling never lands
/// somewhere that ignores every key.
#[cfg(feature = "plugins")]
#[test]
fn focus_ring_includes_a_plugin_pane_only_when_it_takes_input() {
    use crate::session::plugin_manifest::PaneSlot;

    let mut h = Harness::new(160, 40, 1);

    let mut readonly =
        crate::plugin::PluginPane::loading("demo", "board", "Demo", PaneSlot::Right, true);
    readonly.accepts_input = false;
    h.app.set_plugin_panes(vec![readonly]);
    assert!(h.app.focusable_plugin_pane().is_none());

    let mut interactive =
        crate::plugin::PluginPane::loading("demo", "board", "Demo", PaneSlot::Right, true);
    interactive.accepts_input = true;
    h.app.set_plugin_panes(vec![interactive]);
    assert!(h.app.focusable_plugin_pane().is_some());
}

/// A hidden pane is never focusable, however its plugin is declared.
#[cfg(feature = "plugins")]
#[test]
fn a_hidden_plugin_pane_is_not_focusable() {
    use crate::session::plugin_manifest::PaneSlot;

    let mut h = Harness::new(160, 40, 1);
    let mut pane =
        crate::plugin::PluginPane::loading("demo", "board", "Demo", PaneSlot::Right, false);
    pane.accepts_input = true;
    h.app.set_plugin_panes(vec![pane]);

    assert!(h.app.focusable_plugin_pane().is_none());
}

/// With no worker attached, offering a key must report "not consumed" rather
/// than blocking — the fallback that keeps a missing plugin host harmless.
#[cfg(feature = "plugins")]
#[test]
fn offering_a_key_without_a_worker_does_not_block() {
    use crate::session::plugin_manifest::PaneSlot;

    let mut h = Harness::new(160, 40, 1);
    let mut pane =
        crate::plugin::PluginPane::loading("demo", "board", "Demo", PaneSlot::Right, true);
    pane.accepts_input = true;
    h.app.set_plugin_panes(vec![pane]);

    let started = std::time::Instant::now();
    assert!(!h.app.offer_key_to_plugin("j".to_string()));
    assert!(started.elapsed() < std::time::Duration::from_millis(500));
}

// ── the published pane context (ADR-27) ──────────────────────────────────────
//
// The snapshot a plugin pane reads is kernel state, so these run in both build
// configurations: the publisher is not gated on the plugin feature, and the two
// gates on it are what stop an installed plugin costing the idle loop a rebuild
// per tick.

/// Sets reader demand for one test and clears both process-wide slots on drop,
/// so a later test asserting "nothing is built" cannot fail because of this one.
/// Holds the crate-wide pane-context test lock for its lifetime.
struct DemandGuard(#[allow(dead_code)] std::sync::MutexGuard<'static, ()>);

impl DemandGuard {
    fn new(present: bool) -> Self {
        let guard = crate::session::pane_context::test_lock();
        crate::session::pane_context::clear_for_test();
        crate::session::pane_context::set_readers_present(present);
        Self(guard)
    }
}

impl Drop for DemandGuard {
    fn drop(&mut self) {
        crate::session::pane_context::clear_for_test();
    }
}

#[test]
fn pane_context_is_not_built_without_a_reader() {
    let _demand = DemandGuard::new(false);
    let mut h = Harness::standard(2);
    for _ in 0..5 {
        h.tick();
    }
    assert_eq!(
        h.app.perf_counters().pane_context_builds,
        0,
        "no plugin can read kernel state, so none may be gathered"
    );
    assert_eq!(h.app.perf_counters().pane_context_publishes, 0);
}

#[test]
fn pane_context_publishes_once_while_unchanged() {
    let _demand = DemandGuard::new(true);
    let mut h = Harness::standard(2);
    for _ in 0..5 {
        h.tick();
    }
    let p = h.app.perf_counters();
    assert_eq!(
        p.pane_context_builds, 5,
        "a reader exists, so each tick gathers"
    );
    assert_eq!(
        p.pane_context_publishes, 1,
        "unchanged state must not be republished"
    );
}

#[test]
fn pane_context_publishes_again_when_the_state_moves() {
    let _demand = DemandGuard::new(true);
    let mut h = Harness::standard(2);
    h.tick();
    assert_eq!(h.app.perf_counters().pane_context_publishes, 1);

    h.app.sessions[h.app.active_index].info.name = "renamed".to_string();
    h.tick();
    assert_eq!(
        h.app.perf_counters().pane_context_publishes,
        2,
        "a changed session name is a changed snapshot"
    );
    assert_eq!(
        crate::session::pane_context::published()
            .and_then(|c| c.session)
            .map(|s| s.name),
        Some("renamed".to_string())
    );
}

#[test]
fn publishing_the_pane_context_does_not_repaint() {
    let _demand = DemandGuard::new(true);
    let mut h = Harness::standard(2);
    // The publisher on its own, not a whole tick: a tick has a dozen other
    // reasons to mark the interface dirty, and the claim under test is about
    // this one step.
    h.app.detect_output_redraw(); // prime the output baseline
    h.app.mark_redrawn();
    h.app.sessions[h.app.active_index].info.name = "renamed".to_string();
    h.app.publish_pane_context();
    assert_eq!(
        h.app.perf_counters().pane_context_publishes,
        1,
        "the state moved, so it was published"
    );
    assert!(
        !h.app.should_redraw(),
        "kernel state a plugin may read is not a reason to paint: the pane \
         repaints when its own tree changes"
    );
}

/// The snapshot must resolve what a sandboxed plugin cannot: the active
/// session's identity, its status in drawable form, and a repo name rather than
/// a path.
#[test]
fn pane_context_describes_the_active_session() {
    let _demand = DemandGuard::new(true);
    let h = Harness::standard(2);
    let context = h.app.build_pane_context();
    let session = context.session.expect("a session is active");
    assert_eq!(session.name, h.app.sessions[h.app.active_index].info.name);
    assert_eq!(
        session.status.icon,
        h.app.sessions[h.app.active_index].info.status.icon()
    );
    assert!(
        !session.status.token.is_empty(),
        "the kernel names the token, so a plugin never maps a status to a colour"
    );
    assert!(
        session
            .repo_name
            .as_deref()
            .is_none_or(|n| !n.contains('/')),
        "a repo reaches a plugin as a display name, not a path: {:?}",
        session.repo_name
    );
}

/// The task section carries the rows a pane draws plus the three view facts a
/// plugin cannot observe: which row the cursor is on, what a search dimmed, and
/// what it matched.
#[test]
fn pane_context_describes_the_task_list() {
    let _demand = DemandGuard::new(true);
    let mut h = Harness::standard(1);
    for title in ["write it", "ship it"] {
        h.app
            .db
            .create_task(&crate::storage::tasks::NewTask::local(title))
            .unwrap();
    }
    h.app.refresh_tasks();

    // In the pane's own order, which is the list's order — not sorted here, so
    // a plugin draws the rows the native pane draws in the sequence it draws
    // them.
    let bare = h.app.build_pane_context().tasks;
    let published: Vec<&str> = bare.entries.iter().map(|t| t.title.as_str()).collect();
    let pane: Vec<String> = h
        .app
        .task_pane_entries()
        .into_iter()
        .map(|e| e.title)
        .collect();
    assert_eq!(
        published,
        pane.iter().map(String::as_str).collect::<Vec<_>>()
    );
    assert_eq!(published.len(), 2);
    assert_eq!(bare.entries[0].status, "todo");
    assert!(
        bare.entries.iter().all(|t| !t.selected),
        "an unfocused pane marks no row, so a plugin does not draw a cursor \
         where the user cannot move one"
    );
    assert!(!bare.focused);

    // Focusing the pane is what puts the cursor on screen.
    h.key(KeyCode::F(5), KeyModifiers::NONE);
    let focused = h.app.build_pane_context().tasks;
    assert!(focused.focused);
    assert!(focused.entries[0].selected);
    assert!(!focused.entries[1].selected);
}

/// With the feature off thurbox draws no task list at all, so a pane advertising
/// one would surface something the user switched off.
#[test]
fn pane_context_publishes_no_tasks_when_the_feature_is_off() {
    let _demand = DemandGuard::new(true);
    let mut h = Harness::standard(1);
    h.app
        .db
        .create_task(&crate::storage::tasks::NewTask::local("hidden"))
        .unwrap();
    h.app.refresh_tasks();
    assert_eq!(h.app.build_pane_context().tasks.entries.len(), 1);

    h.app.features.tasks = false;
    assert!(
        h.app.build_pane_context().tasks.entries.is_empty(),
        "the section follows its feature flag, like the automation section"
    );
}

/// The bound is on the section, so a list far longer than any pane can show
/// cannot make a plugin's render exceed the view tree's node budget.
#[test]
fn pane_context_bounds_how_many_task_rows_it_publishes() {
    let _demand = DemandGuard::new(true);
    let mut h = Harness::standard(1);
    let over = crate::session::pane_context::MAX_TASK_ROWS + 7;
    for i in 0..over {
        h.app
            .db
            .create_task(&crate::storage::tasks::NewTask::local(format!("t{i}")))
            .unwrap();
    }
    h.app.refresh_tasks();
    assert_eq!(
        h.app.build_pane_context().tasks.entries.len(),
        crate::session::pane_context::MAX_TASK_ROWS
    );
}

/// A changed task list is a changed snapshot; an unchanged one still publishes
/// once, so adding the section did not defeat the change gate.
#[test]
fn a_changed_task_list_republishes_and_an_unchanged_one_does_not() {
    let _demand = DemandGuard::new(true);
    let mut h = Harness::standard(1);
    h.tick();
    let before = h.app.perf_counters().pane_context_publishes;

    h.app
        .db
        .create_task(&crate::storage::tasks::NewTask::local("new work"))
        .unwrap();
    h.app.refresh_tasks();
    h.tick();
    assert_eq!(
        h.app.perf_counters().pane_context_publishes,
        before + 1,
        "a task the user added is state that moved"
    );
    for _ in 0..3 {
        h.tick();
    }
    assert_eq!(
        h.app.perf_counters().pane_context_publishes,
        before + 1,
        "a still task list must not be republished every tick"
    );
}

/// The file section publishes the tree the viewer has open — with the cursor's
/// row as an index, basenames rather than paths, and no rendering.
#[test]
fn pane_context_describes_the_open_file_tree() {
    let _demand = DemandGuard::new(true);
    let mut h = Harness::standard(1);

    // Nothing is published before the pane has a tree: `FileViewerState` is
    // filled lazily by the pane that owns it, so an untouched viewer is an empty
    // section rather than a directory read on the tick.
    assert!(h.app.build_pane_context().files.nodes.is_empty());
    assert_eq!(h.app.build_pane_context().files.selected, None);

    // Give the session a directory with something in it, then open the viewer the
    // way a user does.
    let repo = tempfile::tempdir().unwrap();
    std::fs::create_dir(repo.path().join("src")).unwrap();
    std::fs::write(repo.path().join("README.md"), "hi").unwrap();
    let idx = h.app.active_index;
    h.app.sessions[idx].info.cwd = Some(repo.path().to_path_buf());
    h.app.rebuild_file_viewer_for_active();

    let files = h.app.build_pane_context().files;
    let names: Vec<&str> = files.nodes.iter().map(|n| n.name.as_str()).collect();
    // The root, then its entries — directories before files, which is the order
    // the pane lists them in.
    assert_eq!(names.len(), 3, "{names:?}");
    assert_eq!(names[1], "src");
    assert_eq!(names[2], "README.md");
    assert_eq!(files.nodes[0].depth, 0);
    assert!(files.nodes[0].is_dir && files.nodes[0].expanded);
    assert_eq!(files.nodes[1].depth, 1);
    assert!(files.nodes[1].is_dir && !files.nodes[1].expanded);
    assert!(!files.nodes[2].is_dir);
    assert!(
        files.nodes.iter().all(|n| n.matched),
        "no search is running, so every row matches"
    );
    assert_eq!(
        files.selected,
        Some(0),
        "the cursor is an index into the rows, in the form the list node takes"
    );
    // And a basename, not a path: the whole capability rests on this.
    assert!(
        !names.iter().any(|n| n.contains(std::path::MAIN_SEPARATOR)),
        "{names:?}"
    );
}

/// With the feature off thurbox draws no file viewer, so a pane advertising one
/// would surface something the user switched off.
#[test]
fn pane_context_publishes_no_files_when_the_feature_is_off() {
    let _demand = DemandGuard::new(true);
    let mut h = Harness::standard(1);
    let repo = tempfile::tempdir().unwrap();
    std::fs::write(repo.path().join("a.txt"), "x").unwrap();
    let idx = h.app.active_index;
    h.app.sessions[idx].info.cwd = Some(repo.path().to_path_buf());
    h.app.rebuild_file_viewer_for_active();
    assert!(!h.app.build_pane_context().files.nodes.is_empty());

    h.app.features.file_viewer = false;
    let files = h.app.build_pane_context().files;
    assert!(
        files.nodes.is_empty() && files.selected.is_none(),
        "the section follows its feature flag, like the task and automation sections"
    );
}

/// The bound is on the section, so a tree far longer than any pane can show
/// cannot make a plugin's render exceed the view tree's node budget — and the
/// cursor is dropped rather than published as an index into rows that were not.
#[test]
fn pane_context_bounds_how_many_file_rows_it_publishes() {
    let _demand = DemandGuard::new(true);
    let mut h = Harness::standard(1);
    let repo = tempfile::tempdir().unwrap();
    let over = crate::session::pane_context::MAX_FILE_ROWS + 7;
    for i in 0..over {
        std::fs::write(repo.path().join(format!("f{i:05}.txt")), "x").unwrap();
    }
    let idx = h.app.active_index;
    h.app.sessions[idx].info.cwd = Some(repo.path().to_path_buf());
    h.app.rebuild_file_viewer_for_active();
    // Put the cursor past the bound, which is the case the drop rule is for.
    h.app.file_viewer.select_index(over);

    let files = h.app.build_pane_context().files;
    assert_eq!(
        files.nodes.len(),
        crate::session::pane_context::MAX_FILE_ROWS
    );
    assert_eq!(
        files.selected, None,
        "a cursor past the published rows is dropped, not clamped: an index into \
         rows that were not published would make the kernel's windowing meaningless"
    );
}

/// A changed file tree is a changed snapshot; an unchanged one still publishes
/// once, so adding the section did not defeat the change gate.
#[test]
fn a_changed_file_tree_republishes_and_an_unchanged_one_does_not() {
    let _demand = DemandGuard::new(true);
    let mut h = Harness::standard(1);
    let repo = tempfile::tempdir().unwrap();
    std::fs::write(repo.path().join("a.txt"), "x").unwrap();
    let idx = h.app.active_index;
    h.app.sessions[idx].info.cwd = Some(repo.path().to_path_buf());
    h.tick();
    let before = h.app.perf_counters().pane_context_publishes;

    h.app.rebuild_file_viewer_for_active();
    h.tick();
    assert_eq!(
        h.app.perf_counters().pane_context_publishes,
        before + 1,
        "a tree the user opened is state that moved"
    );
    for _ in 0..3 {
        h.tick();
    }
    assert_eq!(
        h.app.perf_counters().pane_context_publishes,
        before + 1,
        "a still file tree must not be republished every tick"
    );
}

#[test]
fn pane_context_has_no_session_when_none_is_open() {
    let _demand = DemandGuard::new(true);
    let h = Harness::standard(0);
    assert!(
        h.app.build_pane_context().session.is_none(),
        "an empty thurbox is the normal case, not an error"
    );
}
