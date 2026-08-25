//! Frame snapshots of the bundled surfaces — the regression net for "the frame
//! looked wrong".
//!
//! The other `v2_*` files assert cells and substrings, which is precise but
//! sparse: a misaligned border, a ghost cell after a reflow, a broken group
//! header or a mis-dimmed row all pass a "contains this string" check. Here the
//! whole painted frame is the assertion, reviewed with `cargo insta review`
//! when a change is intentional.
//!
//! Everything is pinned so the frames are deterministic: a fixed snapshot
//! (fixed `taken_at_ms`), a fixed `elapsed` (the working spinner picks its
//! frame from it), and the `default` theme selected explicitly rather than
//! whatever the environment resolves. Keep it that way — v1's snapshot suite
//! died of nondeterminism churn, and a snapshot that moves on every run is
//! worse than none.
//!
//! Deliberately absent: the floats (creation flow, confirm, restore). Each
//! opens through store state a real interaction writes, so their frames belong
//! to interaction tests, not to a pinned-data suite.

use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui::Terminal;

use thurbox::kernel::host::{KeyPress, LuaHost, RenderContext};
use thurbox::kernel::layout::resolve;
use thurbox::kernel::paint::{render, PlaceholderSurfaces};
use thurbox::kernel::registry::Registry;
use thurbox::kernel::snapshot::{SessionRow, Snapshot};
use thurbox::kernel::theme::Themes;

/// The pinned theme: the `default` preset by name, never the environment's
/// active choice — the style snapshot below records real colours.
fn themes() -> Themes {
    let mut themes = Themes::load(None);
    themes
        .preview("default")
        .expect("the default preset exists");
    themes
}

fn registry(host: &LuaHost) -> Registry {
    let mut registry = Registry::default();
    let (bindings, settings) = host.declarations();
    registry.declare(bindings, settings);
    registry
}

fn publish(host: &LuaHost, snapshot: &Snapshot) {
    publish_with(host, snapshot, &Default::default());
}

fn publish_with(
    host: &LuaHost,
    snapshot: &Snapshot,
    attach_errors: &std::collections::HashMap<String, String>,
) {
    let themes = themes();
    let registry = registry(host);
    let diffs = thurbox::kernel::diff::DiffStore::new();
    let repos = thurbox::kernel::repos::RepoStore::with_hosts(Default::default());
    host.publish(&thurbox::kernel::host::Published {
        epoch: thurbox::kernel::host::Epoch::always_fresh(),
        snapshot,
        attach_errors,
        inflight: &[],
        themes: &themes,
        registry: &registry,
        diffs: &diffs,
        links: &Default::default(),
        content: &Default::default(),
        meta: &Default::default(),
        metrics: &Default::default(),
        status_rows: 0,
        can_open: true,
        inventory: &[],
        ui_dir: "ui",
        settings: &Default::default(),
        repos: &repos,
        wants: &Default::default(),
        focus: None,
        hovered: None,
    })
    .expect("publish");
}

fn host() -> LuaHost {
    let host = LuaHost::new(std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("ui"));
    assert!(
        host.error.is_none(),
        "bundled plugins must load: {:?}",
        host.error
    );
    host
}

fn row(name: &str, repo: &str, status: &str) -> SessionRow {
    SessionRow {
        id: format!("{name}-0000-0000-0000-000000000000"),
        name: name.to_string(),
        agent: "claude".to_string(),
        status: status.to_string(),
        cwd: None,
        repo: Some(repo.to_string()),
        repos: vec![repo.to_string()],
        branch: Some(format!("feat/{name}")),
        base_branch: None,
        backend: "local-tmux".to_string(),
        backend_id: Some("%1".to_string()),
        agent_session_id: None,
        remote_host: None,
        parent_id: None,
        display_order: None,
        worktree_count: 1,
        git: None,
        hook_state: None,
        shell_backend_id: None,
        member_dirs: Vec::new(),
    }
}

fn snapshot(rows: Vec<SessionRow>) -> Snapshot {
    Snapshot {
        sessions: rows,
        taken_at_ms: 1_700_000_000_000,
        ..Snapshot::default()
    }
}

/// The one sample every snapshot draws from: two repo groups, every status,
/// and a parent → child pair so the tree prefix is on record.
fn sample() -> Snapshot {
    let mut rows = vec![
        row("fix-osc52", "thurbox", "working"),
        row("add-wsl-tests", "thurbox", "blocked"),
        row("perf-cache", "thurbox", "done"),
        row("update-deps", "website", "idle"),
    ];
    let mut child = row("fix-osc52-tests", "thurbox", "idle");
    child.parent_id = Some(rows[0].id.clone());
    rows.push(child);
    snapshot(rows)
}

fn ctx(width: u16, height: u16, focused: bool) -> RenderContext {
    RenderContext {
        width,
        height,
        focused,
        // Fixed, so the working spinner picks the same frame every run.
        elapsed: 1.0,
        frame: 1,
    }
}

fn index_of(host: &LuaHost, name: &str) -> usize {
    host.plugins
        .iter()
        .position(|plugin| plugin.name == name)
        .unwrap_or_else(|| panic!("no plugin named {name}"))
}

fn buffer_lines(buffer: &ratatui::buffer::Buffer, width: u16, height: u16) -> String {
    (0..height)
        .map(|y| {
            (0..width)
                .map(|x| buffer[(x, y)].symbol().to_string())
                .collect::<String>()
                .trim_end()
                .to_string()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Render one plugin at a size and return the frame as text.
fn paint(host: &LuaHost, name: &str, width: u16, height: u16) -> String {
    let node = host
        .render(index_of(host, name), ctx(width, height, true))
        .unwrap_or_else(|e| panic!("plugin should render: {e}"))
        .node;
    let mut terminal = Terminal::new(TestBackend::new(width, height)).expect("terminal");
    terminal
        .draw(|frame| render(frame, frame.area(), &node, &PlaceholderSurfaces))
        .expect("draw");
    buffer_lines(terminal.backend().buffer(), width, height)
}

/// Press a key exactly as the binary routes it: registry first, then raw
/// `on_key` — copied from `kernel_mvp.rs` so cursor movement is the real path.
fn press_key(host: &LuaHost, plugin: &str, ch: char) {
    let registry = registry(host);
    let key = KeyPress {
        name: ch.to_lowercase().to_string(),
        ch: Some(ch),
        ..KeyPress::default()
    };
    if let Some(binding) = registry.resolve(&key, Some(plugin)) {
        if let Some(index) = host.index_of(&binding.plugin) {
            if host.on_action(index, &binding.action).expect("action") {
                return;
            }
        }
    }
    host.on_key(index_of(host, plugin), &key).expect("key");
}

// --- the session list -------------------------------------------------------

#[test]
fn session_list_grouped() {
    let host = host();
    publish(&host, &sample());
    insta::assert_snapshot!("session_list_grouped", paint(&host, "sessions", 40, 14));
}

#[test]
fn session_list_narrow() {
    // Near the floor: names truncate, the frame stays coherent.
    let host = host();
    publish(&host, &sample());
    insta::assert_snapshot!("session_list_narrow", paint(&host, "sessions", 24, 10));
}

#[test]
fn session_list_windowed() {
    // More rows than height: the window and its overflow marker are the frame.
    let host = host();
    let rows: Vec<SessionRow> = (0..40)
        .map(|n| row(&format!("session-{n:02}"), "thurbox", "idle"))
        .collect();
    publish(&host, &snapshot(rows));
    insta::assert_snapshot!("session_list_windowed", paint(&host, "sessions", 40, 10));
}

#[test]
fn session_list_wide_glyph_names() {
    // Double-width characters in names are where column budgets go wrong: a
    // CJK or emoji name that miscounts its width shears the whole row.
    let host = host();
    publish(
        &host,
        &snapshot(vec![
            row("修复终端宽度", "thurbox", "idle"),
            row("emoji-🚀-name", "thurbox", "blocked"),
            row("plain-name", "thurbox", "idle"),
        ]),
    );
    insta::assert_snapshot!(
        "session_list_wide_glyph_names",
        paint(&host, "sessions", 40, 10)
    );
}

// --- the agent pane ---------------------------------------------------------

#[test]
fn agent_pane_empty_state() {
    let host = host();
    publish(&host, &snapshot(Vec::new()));
    insta::assert_snapshot!("agent_pane_empty_state", paint(&host, "agent", 50, 8));
}

#[test]
fn agent_pane_detached_surface() {
    // PlaceholderSurfaces has nothing live behind it, so the surface draws the
    // detached notice — the frame a fresh boot shows before the first attach.
    let host = host();
    publish(&host, &sample());
    host.render(index_of(&host, "sessions"), ctx(40, 12, true))
        .expect("render list");
    insta::assert_snapshot!("agent_pane_detached_surface", paint(&host, "agent", 60, 10));
}

#[test]
fn agent_pane_attach_error() {
    let host = host();
    let errors: std::collections::HashMap<String, String> = sample()
        .sessions
        .iter()
        .map(|row| (row.id.clone(), "can't find pane: %45".to_string()))
        .collect();
    publish_with(&host, &sample(), &errors);
    host.render(index_of(&host, "sessions"), ctx(40, 12, true))
        .expect("render list");
    insta::assert_snapshot!("agent_pane_attach_error", paint(&host, "agent", 70, 10));
}

// --- styles -----------------------------------------------------------------

/// One line of the frame as style *runs*: consecutive cells sharing a style
/// collapse into `⟨fg/bg/mods⟩text`. Compact enough to review, precise enough
/// that a lost selection highlight, a dropped dim or a recoloured status dot
/// changes the snapshot.
fn style_runs(buffer: &ratatui::buffer::Buffer, width: u16, height: u16) -> String {
    let mut lines = Vec::new();
    for y in 0..height {
        let mut line = String::new();
        let mut last: Option<String> = None;
        for x in 0..width {
            let cell = &buffer[(x, y)];
            let style = format!("{:?}/{:?}/{:?}", cell.fg, cell.bg, cell.modifier);
            if last.as_deref() != Some(style.as_str()) {
                line.push_str(&format!("⟨{style}⟩"));
                last = Some(style);
            }
            line.push_str(cell.symbol());
        }
        lines.push(line.trim_end().to_string());
    }
    lines.join("\n")
}

#[test]
fn session_list_selection_styles() {
    // The selection is a STYLE, not a glyph (v1's rule), so only a styled
    // snapshot can see it move — and only one can see a status dot's colour.
    let host = host();
    publish(&host, &sample());
    let index = index_of(&host, "sessions");
    host.render(index, ctx(40, 10, true)).expect("render");
    press_key(&host, "sessions", 'j');

    let node = host.render(index, ctx(40, 10, true)).expect("render").node;
    let mut terminal = Terminal::new(TestBackend::new(40, 10)).expect("terminal");
    terminal
        .draw(|frame| render(frame, frame.area(), &node, &PlaceholderSurfaces))
        .expect("draw");
    insta::assert_snapshot!(
        "session_list_selection_styles",
        style_runs(terminal.backend().buffer(), 40, 10)
    );
}

// --- the whole arrangement --------------------------------------------------

/// Resolve the real arrangement at a size and paint every placed slot's first
/// occupant into its rect — the closest an in-process test gets to the frame
/// the binary flushes.
fn paint_arrangement(host: &LuaHost, width: u16, height: u16) -> String {
    let area = Rect {
        x: 0,
        y: 0,
        width,
        height,
    };
    let placed = resolve(&host.arrangement(width, height).expect("layout"), area);
    let mut panes = Vec::new();
    for slot in &placed {
        let members = host.in_slot(&slot.slot);
        let Some(&index) = members.first() else {
            continue;
        };
        let focused = host.plugins[index].name == "sessions";
        let node = host
            .render(index, ctx(slot.rect.width, slot.rect.height, focused))
            .unwrap_or_else(|e| panic!("plugin should render: {e}"))
            .node;
        panes.push((slot.rect, node));
    }
    let mut terminal = Terminal::new(TestBackend::new(width, height)).expect("terminal");
    terminal
        .draw(|frame| {
            for (rect, node) in &panes {
                render(frame, *rect, node, &PlaceholderSurfaces);
            }
        })
        .expect("draw");
    buffer_lines(terminal.backend().buffer(), width, height)
}

#[test]
fn full_frame_wide() {
    let host = host();
    publish(&host, &sample());
    host.render(index_of(&host, "sessions"), ctx(40, 12, true))
        .expect("render list");
    insta::assert_snapshot!("full_frame_wide", paint_arrangement(&host, 120, 30));
}

#[test]
fn full_frame_narrow() {
    // Below the breakpoint the side column is gone; this frame is the record
    // of what survives and where it lands.
    let host = host();
    publish(&host, &sample());
    host.render(index_of(&host, "sessions"), ctx(40, 12, true))
        .expect("render list");
    insta::assert_snapshot!("full_frame_narrow", paint_arrangement(&host, 60, 24));
}
