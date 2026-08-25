//! Property tests for the render pipeline's crash invariants.
//!
//! The display bugs that hurt are the ones no example-based test thought to
//! write: a pane resolved one cell wide, a selection dragged past the grid, a
//! screen full of double-width glyphs. Each property here is a class of input
//! the pipeline must survive *whole* — the assertion is usually "does not
//! panic", because a panic on the render path costs the frame (or, on a
//! reader thread, poisons a parser mutex for the life of the process).
//!
//! `tests/kernel_limits.rs` owns the instruction/memory ceilings;
//! `agent::control_mode`'s own proptests own byte transparency. This file is
//! the geometry: sizes, positions and glyph widths.

use proptest::prelude::*;
use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui::Terminal;

use thurbox::kernel::host::{LuaHost, RenderContext};
use thurbox::kernel::layout::resolve;
use thurbox::kernel::paint::{normalize_ambiguous_width, render, PlaceholderSurfaces};
use thurbox::kernel::selection::{
    extract_text_from_buffer, extract_text_from_screen, highlight_buffer, PaneBounds, Selection,
    TermPos,
};
use thurbox::kernel::snapshot::{SessionRow, Snapshot};

fn host() -> LuaHost {
    let host = LuaHost::new(std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("ui"));
    assert!(
        host.error.is_none(),
        "bundled plugins must load: {:?}",
        host.error
    );
    host
}

fn row(name: &str, status: &str) -> SessionRow {
    SessionRow {
        id: format!("{name}-0000-0000-0000-000000000000"),
        name: name.to_string(),
        agent: "claude".to_string(),
        status: status.to_string(),
        cwd: None,
        repo: Some("thurbox".to_string()),
        repos: vec!["thurbox".to_string()],
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

fn publish(host: &LuaHost, rows: Vec<SessionRow>) {
    let themes = thurbox::kernel::theme::Themes::load(None);
    let mut registry = thurbox::kernel::registry::Registry::default();
    let (bindings, settings) = host.declarations();
    registry.declare(bindings, settings);
    let diffs = thurbox::kernel::diff::DiffStore::new();
    let repos = thurbox::kernel::repos::RepoStore::with_hosts(Default::default());
    let snapshot = Snapshot {
        sessions: rows,
        taken_at_ms: 1_700_000_000_000,
        ..Snapshot::default()
    };
    host.publish(&thurbox::kernel::host::Published {
        epoch: thurbox::kernel::host::Epoch::always_fresh(),
        snapshot: &snapshot,
        attach_errors: &Default::default(),
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

#[test]
fn bundled_panes_render_and_paint_at_any_size() {
    // The size a pane is handed is the layout's business, not the plugin's —
    // so every bundled pane must render and paint at whatever it gets, down
    // to a single cell. A Lua error here would blank the pane in the running
    // interface; a paint panic would cost the frame.
    let host = host();
    publish(
        &host,
        vec![
            row("fix-osc52", "working"),
            row("add-wsl-tests", "blocked"),
            row("a-name-much-longer-than-most-panes-are-wide", "idle"),
        ],
    );
    let panes: Vec<usize> = ["sessions", "agent", "search"]
        .iter()
        .map(|name| {
            host.plugins
                .iter()
                .position(|p| &p.name == name)
                .unwrap_or_else(|| panic!("no plugin named {name}"))
        })
        .collect();

    proptest!(ProptestConfig::with_cases(64), |(width in 1u16..=200, height in 1u16..=80)| {
        for &index in &panes {
            let node = host
                .render(
                    index,
                    RenderContext {
                        width,
                        height,
                        focused: true,
                        elapsed: 1.0,
                        frame: 1,
                    },
                )
                .unwrap_or_else(|e| panic!("{}x{}: {e}", width, height))
                .node;
            let mut terminal =
                Terminal::new(TestBackend::new(width, height)).expect("terminal");
            terminal
                .draw(|frame| render(frame, frame.area(), &node, &PlaceholderSurfaces))
                .expect("draw");
        }
    });
}

#[test]
fn resolved_slots_stay_inside_the_screen() {
    // The arrangement is Lua arithmetic; `resolve` must hand out rects that
    // fit the area whatever that arithmetic returns, or the paint indexes out
    // of the buffer.
    let host = host();
    proptest!(ProptestConfig::with_cases(128), |(width in 1u16..=300, height in 1u16..=100)| {
        let area = Rect { x: 0, y: 0, width, height };
        let placed = resolve(
            &host.arrangement(width, height).expect("layout"),
            area,
        );
        for slot in &placed {
            prop_assert!(
                slot.rect.x + slot.rect.width <= width
                    && slot.rect.y + slot.rect.height <= height,
                "slot {} at {:?} escapes a {width}x{height} screen",
                slot.slot,
                slot.rect
            );
        }
    });
}

/// Arbitrary printable content, weighted toward the widths that break column
/// math: ASCII, CJK (double-width), emoji, and the variation selector that
/// `normalize_ambiguous_width` exists for.
fn glyph_soup() -> impl Strategy<Value = String> {
    proptest::collection::vec(
        prop_oneof![
            4 => proptest::char::range(' ', '~'),
            1 => proptest::char::range('\u{4e00}', '\u{4eff}'),
            1 => Just('🚀'),
            1 => Just('\u{FE0F}'),
        ],
        0..80,
    )
    .prop_map(|chars| chars.into_iter().collect())
}

/// Paint `lines` one per row into a fresh buffer of the given size.
fn buffer_of(lines: &[String], width: u16, height: u16) -> ratatui::buffer::Buffer {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).expect("terminal");
    terminal
        .draw(|frame| {
            for (i, line) in lines.iter().enumerate().take(height as usize) {
                frame.render_widget(
                    ratatui::widgets::Paragraph::new(line.as_str()),
                    Rect {
                        x: 0,
                        y: i as u16,
                        width,
                        height: 1,
                    },
                );
            }
        })
        .expect("draw");
    terminal.backend().buffer().clone()
}

#[test]
fn selection_over_any_buffer_never_panics() {
    // The pane rect, the drag endpoints and the buffer contents are three
    // independent coordinate systems; every combination — including endpoints
    // far outside the pane and the buffer — must extract and highlight
    // without panicking, and extraction must stay inside the pane it was
    // confined to.
    proptest!(|(
        lines in proptest::collection::vec(glyph_soup(), 1..20),
        pane_x in 0u16..40, pane_y in 0u16..20,
        pane_w in 1u16..40, pane_h in 1u16..20,
        a_row in 0usize..80, a_col in 0usize..200,
        c_row in 0usize..80, c_col in 0usize..200,
    )| {
        let mut buffer = buffer_of(&lines, 60, 24);

        let pane = PaneBounds::from_rect(Rect {
            x: pane_x,
            y: pane_y,
            width: pane_w,
            height: pane_h,
        });
        let mut selection = Selection::new(TermPos { row: a_row, col: a_col }, pane);
        selection.cursor = TermPos { row: c_row, col: c_col };

        let text = extract_text_from_buffer(&buffer, &selection);
        // Confinement: rows are clamped to the pane's bottom edge, so however
        // far the drag reached, extraction cannot yield more lines than fit
        // above it.
        prop_assert!(
            text.lines().count() <= (pane_y + pane_h) as usize,
            "extraction escaped the pane: {} lines from a pane ending at row {}",
            text.lines().count(),
            pane_y + pane_h
        );
        highlight_buffer(
            &mut buffer,
            &selection,
            ratatui::style::Style::default().bg(ratatui::style::Color::Blue),
        );
    });
}

#[test]
fn selection_over_any_vt100_stream_never_panics() {
    // The terminal-pane half: whatever bytes an agent emits — control
    // sequences, half a UTF-8 glyph, wide characters straddling the last
    // column — the grid that results must be selectable. This is the reader
    // whose panic poisons the parser mutex, so "never panics" is the whole
    // point.
    proptest!(ProptestConfig::with_cases(64), |(
        bytes in proptest::collection::vec(any::<u8>(), 0..2048),
        rows in 2u16..40, cols in 2u16..120,
        a_row in 0usize..60, a_col in 0usize..160,
        c_row in 0usize..60, c_col in 0usize..160,
    )| {
        let mut parser = vt100::Parser::new(rows, cols, 50);
        parser.process(&bytes);

        let pane = PaneBounds::from_rect(Rect { x: 1, y: 1, width: cols, height: rows });
        let mut selection = Selection::new(TermPos { row: a_row, col: a_col }, pane);
        selection.cursor = TermPos { row: c_row, col: c_col };

        let _ = extract_text_from_screen(parser.screen(), &selection, (1, 1));
    });
}

#[test]
fn normalizing_ambiguous_width_only_strips_the_selector() {
    // The one disagreement `normalize_ambiguous_width` resolves is U+FE0F;
    // it must strip every occurrence and change nothing else about the
    // buffer's shape.
    proptest!(|(lines in proptest::collection::vec(glyph_soup(), 1..10))| {
        let width = 40u16;
        let height = lines.len() as u16;
        let mut buffer = buffer_of(&lines, width, height);
        let area_before = buffer.area;

        normalize_ambiguous_width(&mut buffer);

        prop_assert_eq!(buffer.area, area_before);
        for y in 0..height {
            for x in 0..width {
                prop_assert!(
                    !buffer[(x, y)].symbol().contains('\u{FE0F}'),
                    "U+FE0F survived normalization at ({x},{y})"
                );
            }
        }
    });
}
