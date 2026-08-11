//! Focusable tasks panel — a toggleable right-side column (between the terminal
//! and the file viewer), shown/hidden with F5/`Ctrl+W` like the file viewer.
//! Renders a checkbox list of task titles (☐/◐/☑), with global-search matches
//! highlighted; while focused it shows a compact action footer. Filtering is
//! handled by the global `Ctrl+/` search, not a per-pane box.
//!
//! Like the file viewer and the automations pane, this one renders through the
//! **view tree**: it is
//! ordinary in-process Rust on the UI thread, but its rows are a
//! [`ViewNode`] that [`super::plugin_pane::render_tree`] paints. The split is
//! sharp on purpose, because it is the split a plugin lives on:
//!
//! - [`task_rows`] is the **width** step — how wide a title may be and where the
//!   trailing marker's room comes from. A plugin never learns its pane's width,
//!   so this stays the kernel's.
//! - [`tasks_tree`] is the **presentation** step and carries no geometry at all:
//!   status to glyph, status to colour token, the selected/dimmed/matched
//!   emphasis, the marker, the empty-state line. That is the part
//!   `tests/bundled_tasks_panel.rs` asserts a bundled Luau plugin reproduces
//!   exactly.
//!
//! **Height is not in either step.** The tree carries every row plus the index of
//! the cursor's, and the *renderer* resolves the window — so the native pane and
//! a plugin's copy scroll through one implementation
//! (`ui::file_viewer::visible_window`, crate-private) instead of two that could
//! disagree about which rows sit beside the cursor. The pane calls it a second
//! time for its click hitboxes, which need the window as numbers rather than as a
//! paint; same function, so the rows a user can click cannot drift from the rows
//! that were drawn. Mirrors `ui::file_viewer::render`.

use ratatui::{layout::Rect, widgets::Paragraph, Frame};
// Only the retained pre-port oracle (`legacy_line` and friends) still assembles
// ratatui primitives and reaches for a palette colour by hand.
#[cfg(test)]
use ratatui::{
    style::Style,
    text::{Line, Span},
};

use crate::session::motion::FrameTable;
use crate::session::view_tree::{StyleToken, TextStyle, ViewNode};
use crate::session::TaskStatus;

#[cfg(test)]
use super::theme::Theme;
use super::{focus_block, truncate_ellipsis, FocusLevel};

/// One row in the tasks panel (view data built by the app layer).
pub struct TaskPaneEntry {
    /// The task's row id.
    ///
    /// Nothing in the rendering uses it: it is here because the entry list is
    /// what the published snapshot is built from, and a pane that may *change* a
    /// task addresses it by id (ADR-35). Resolving it a second time from the
    /// cached tasks would be the same lookup with a second chance to disagree
    /// about which row is which.
    pub id: i64,
    pub title: String,
    pub status: TaskStatus,
    /// Byte offsets in `title` matched by the active global-search query. Empty
    /// when there's no global search (or this row didn't match).
    pub match_positions: Vec<usize>,
    /// When a global search is active, rows that don't match are dimmed.
    pub dimmed: bool,
    /// The task has at least one currently-open related session (a spawned
    /// `<title> · #<id>` window or a Send target). Drawn as a trailing `⇄`
    /// marker so a live task is glanceable in the list; press `o` to jump to it.
    pub linked: bool,
}

/// Trailing marker shown on rows whose task has an open related session.
const LINKED_MARKER: &str = " ⇄";

pub struct TaskPaneState<'a> {
    pub entries: &'a [TaskPaneEntry],
    pub selected: usize,
    pub focus: FocusLevel,
    /// While global search previews a task here, show the selected row
    /// highlighted (not dimmed) even though the panel isn't focused.
    pub preview_selected: bool,
}

/// One row as the pane's view tree describes it: the title fitted to the column,
/// its status, and the three facts that pick its style.
///
/// Distinct from [`TaskPaneEntry`] because this is the *resolved* row — `title`
/// has been fitted to a width and `selected` has folded in the pane's focus — so
/// building the tree from it needs no geometry and no focus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskRow {
    /// Display title, already fitted to the column.
    pub title: String,
    pub status: TaskStatus,
    /// Byte offsets in `title` a running search matched.
    pub match_positions: Vec<usize>,
    /// A running search filtered this row out.
    pub dimmed: bool,
    /// The user's cursor is on this row *and* it is visible as such.
    pub selected: bool,
    /// The task has an open related session.
    pub linked: bool,
}

pub fn render_tasks_panel(
    frame: &mut Frame,
    area: Rect,
    state: &TaskPaneState<'_>,
) -> Vec<super::RowHitbox> {
    // Shared focus block: highlighted title + accent/rounded border when focused,
    // matching the session list and file viewer panes.
    let block = focus_block(" Tasks ", state.focus);
    let mut inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.height == 0 {
        return Vec::new();
    }

    // While focused, reserve the bottom row for a compact action footer that
    // mirrors the full-screen preview's hints (so `r run` is discoverable here
    // too, not only in the central pane).
    if matches!(state.focus, FocusLevel::Focused) && inner.height > 2 {
        let hint_area = Rect::new(inner.x, inner.y + inner.height - 1, inner.width, 1);
        frame.render_widget(
            // Same relative order as the central preview footer (e · r · n).
            Paragraph::new(super::key_hint_line(&[
                ("e", " edit "),
                ("r", " run "),
                ("n", " new "),
            ])),
            hint_area,
        );
        inner.height -= 1;
    }

    // The renderer windows the list around the cursor (the hint footer, when
    // present, was already subtracted from `inner`), so the returned hitboxes are
    // indexed in entry space over just the visible window.
    render_task_list(frame, inner, state)
}

fn render_task_list(
    frame: &mut Frame,
    area: Rect,
    state: &TaskPaneState<'_>,
) -> Vec<super::RowHitbox> {
    let rows = task_rows(state, area.width);
    let cursor = cursor_row(state);
    let tree = tasks_tree(&rows, cursor, matches!(state.focus, FocusLevel::Focused));
    // A fresh frame table every paint: nothing in this pane animates, and a
    // motion the kernel is not driving draws frame 0 anyway.
    super::plugin_pane::render_tree(
        &tree,
        area,
        &super::theme::current(),
        &FrameTable::default(),
        frame.buffer_mut(),
    );

    // One single-line hitbox per visible row, indexed in entry space. The second
    // call to the windowing helper: the paint above resolved the same window, and
    // the hitboxes need it as numbers.
    let (start, end) =
        super::file_viewer::visible_window(rows.len(), cursor.unwrap_or(0), area.height as usize);
    super::windowed_row_hitboxes(area, start, end)
}

/// Which row the cursor is on, clamped into the list.
///
/// `None` only for an empty list. Clamping rather than dropping an out-of-range
/// index is what the pane has always done — a stale selection left by a shortened
/// list still has to scroll somewhere — and the published snapshot clamps with it
/// so the two panes cannot window differently.
fn cursor_row(state: &TaskPaneState<'_>) -> Option<usize> {
    (!state.entries.is_empty()).then(|| state.selected.min(state.entries.len() - 1))
}

/// Resolve how every row looks at a pane of `width`.
///
/// The one width-dependent step in the pane: each title's fitted form and the
/// room the trailing marker needs. A plugin reproducing this pane has no width, so
/// it draws its titles whole and lets the renderer clip them — the one divergence
/// `tests/bundled_tasks_panel.rs` still enumerates.
///
/// Every row is resolved, not just the visible ones, because the window is the
/// renderer's to choose from the tree's declared cursor. At most
/// `MAX_TASK_ROWS`-worth of `truncate_ellipsis` calls, on a pane that paints only
/// when the UI is dirty.
pub fn task_rows(state: &TaskPaneState<'_>, width: u16) -> Vec<TaskRow> {
    let focused = matches!(state.focus, FocusLevel::Focused);
    let width = width as usize;
    state
        .entries
        .iter()
        .enumerate()
        .map(|(i, e)| {
            // Reserve room for the trailing link marker so it never pushes the
            // title off-row; the glyph plus its space is the other two columns.
            let reserved = if e.linked {
                2 + LINKED_MARKER.chars().count()
            } else {
                2
            };
            TaskRow {
                title: truncate_ellipsis(&e.title, width.saturating_sub(reserved)),
                status: e.status,
                match_positions: e.match_positions.clone(),
                dimmed: e.dimmed,
                // Highlight the selected row when the panel is focused, OR when
                // a global-search preview points here (so the moving cursor is
                // visible even though focus is in the search box). Distinct from
                // the tree's cursor index, which is the scroll anchor and is
                // declared whether or not the cursor is drawn.
                selected: (focused || state.preview_selected) && i == state.selected,
                linked: e.linked,
            }
        })
        .collect()
}

/// Build the pane's view tree from every row plus the cursor's index.
///
/// Carries no geometry: every row is one line whose runs take their width from
/// their content, and no row is windowed away — the list's `cursor` is what lets
/// the *renderer* scroll it to that row from the height it has, so the same tree
/// paints correctly at any size. `focused` is here because it changes *content* —
/// the empty-state line names the key that adds a task only when the pane can
/// receive it.
///
/// `cursor` is not required to be in range: an index past the last row highlights
/// nothing and windows to the end, which is [`super::file_viewer::file_tree`]'s
/// rule too.
pub fn tasks_tree(rows: &[TaskRow], cursor: Option<usize>, focused: bool) -> ViewNode {
    if rows.is_empty() {
        let text = if focused {
            "no tasks — n to add"
        } else {
            "no tasks"
        };
        return ViewNode::list(vec![ViewNode::token(text, StyleToken::Muted)]);
    }
    ViewNode::selectable_list(rows.iter().map(task_row_node).collect(), cursor)
}

/// One task row: `<glyph> <title>` with global-search matches emphasised, plus a
/// trailing `⇄` marker when the task has a live related session.
fn task_row_node(row: &TaskRow) -> ViewNode {
    let base = row_base_style(row);
    // The glyph is its own run; the title follows so highlight byte offsets
    // (which index `title`) line up.
    let mut runs = vec![ViewNode::styled(
        format!("{} ", status_glyph(row.status)),
        base,
    )];

    // Emphasise matched characters on every non-dimmed row (including the
    // selected one); a dimmed row did not match, so it has no positions.
    let positions: &[usize] = if row.dimmed {
        &[]
    } else {
        &row.match_positions
    };
    for (range, matched) in super::highlight::highlight_runs(&row.title, positions) {
        let style = if matched { MATCHED_STYLE } else { base };
        runs.push(ViewNode::styled(&row.title[range], style));
    }

    // Trailing accent marker for tasks with a live session. Dimmed rows
    // (non-matches during a search) keep the dim tone for consistency.
    if row.linked {
        let style = if row.dimmed {
            base
        } else {
            TextStyle {
                token: Some(StyleToken::Accent),
                ..TextStyle::default()
            }
        };
        runs.push(ViewNode::styled(LINKED_MARKER, style));
    }
    ViewNode::Line(runs)
}

/// How a matched character is drawn: accent, bold and underlined, over whatever
/// the row's base style was.
///
/// A constant rather than a function of the base, because the layering only ever
/// resolves to this: the base's colour is replaced and its two attributes added,
/// and the one base that carries an attribute of its own (dimmed) never has
/// matches to emphasise.
const MATCHED_STYLE: TextStyle = TextStyle {
    token: Some(StyleToken::Accent),
    bold: true,
    dim: false,
    underline: true,
    selected: false,
    tint: None,
};

/// A row's resting style, in the precedence the session list and automations
/// pane share: selected wins over dimmed, which wins over the status colour.
///
/// The token spellings are the tree's names for what
/// [`super::highlight::row_base_style`] resolves to — `Theme::selected_item()`
/// is accent + bold, a dimmed row is muted + dim — and the differential test in
/// this module is what holds the two together.
fn row_base_style(row: &TaskRow) -> TextStyle {
    if row.selected {
        TextStyle {
            token: Some(StyleToken::Accent),
            bold: true,
            ..TextStyle::default()
        }
    } else if row.dimmed {
        TextStyle {
            token: Some(StyleToken::Muted),
            dim: true,
            ..TextStyle::default()
        }
    } else {
        TextStyle {
            token: status_token(row.status),
            ..TextStyle::default()
        }
    }
}

fn status_glyph(status: TaskStatus) -> &'static str {
    match status {
        TaskStatus::Todo => "☐",
        TaskStatus::InProgress => "◐",
        TaskStatus::Done => "☑",
    }
}

/// The colour role a status is drawn in. `None` is the theme's primary text,
/// which is what an untouched todo has always been.
fn status_token(status: TaskStatus) -> Option<StyleToken> {
    match status {
        TaskStatus::Todo => None,
        TaskStatus::InProgress => Some(StyleToken::Accent),
        TaskStatus::Done => Some(StyleToken::Muted),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{backend::TestBackend, buffer::Buffer, Terminal};

    fn entry(title: &str) -> TaskPaneEntry {
        TaskPaneEntry {
            id: 1,
            title: title.into(),
            status: TaskStatus::Todo,
            match_positions: vec![],
            dimmed: false,
            linked: false,
        }
    }

    fn hitboxes(focus: FocusLevel, count: usize) -> Vec<super::super::RowHitbox> {
        hitboxes_sel(focus, count, 0)
    }

    fn hitboxes_sel(
        focus: FocusLevel,
        count: usize,
        selected: usize,
    ) -> Vec<super::super::RowHitbox> {
        let backend = TestBackend::new(20, 6);
        let mut terminal = Terminal::new(backend).unwrap();
        let entries: Vec<TaskPaneEntry> = (0..count).map(|i| entry(&format!("t{i}"))).collect();
        let mut rows = Vec::new();
        terminal
            .draw(|f| {
                rows = render_tasks_panel(
                    f,
                    Rect::new(0, 0, 20, 6),
                    &TaskPaneState {
                        entries: &entries,
                        selected,
                        focus,
                        preview_selected: false,
                    },
                );
            })
            .unwrap();
        rows
    }

    #[test]
    fn row_hitboxes_start_below_border() {
        let rows = hitboxes(FocusLevel::Inactive, 2);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].rect, Rect::new(1, 1, 18, 1));
        assert_eq!(rows[1].rect, Rect::new(1, 2, 18, 1));
        assert_eq!(rows[1].index, 1);
    }

    #[test]
    fn focused_panel_hitboxes_exclude_hint_footer() {
        // 6 outer rows → 4 inner; while focused the bottom inner row is the
        // action footer, so only 3 rows are clickable.
        let rows = hitboxes(FocusLevel::Focused, 10);
        assert_eq!(rows.len(), 3);
        assert_eq!(rows.last().unwrap().rect.y, 3);
    }

    #[test]
    fn selection_below_viewport_scrolls_into_view() {
        // 10 tasks but only 3 visible rows (focused → footer reserved). A
        // selection near the bottom must be inside the rendered window, not
        // clipped off-screen.
        let rows = hitboxes_sel(FocusLevel::Focused, 10, 9);
        assert_eq!(rows.len(), 3);
        let visible: Vec<usize> = rows.iter().map(|r| r.index).collect();
        assert!(
            visible.contains(&9),
            "selected row 9 should be windowed into view, got {visible:?}"
        );
        // Rows above the window are scrolled off (not row 0).
        assert!(!visible.contains(&0));
    }

    // ── the pre-port renderer, retained as an oracle ────────────────────────
    //
    // The span-building row builder the pane used before it rendered through the
    // view tree. Kept so the port is a *checked* refactor: the differential test
    // below asserts the tree paints the same cells, which is the only way to know
    // that moving a pane onto the tree changed nothing a user sees.

    fn legacy_line(row: &TaskRow) -> Line<'_> {
        let base = super::super::highlight::row_base_style(
            row.selected,
            row.dimmed,
            Style::default().fg(legacy_status_color(row.status)),
        );
        let mut spans = vec![Span::styled(format!("{} ", status_glyph(row.status)), base)];
        let positions: &[usize] = if row.dimmed {
            &[]
        } else {
            &row.match_positions
        };
        spans.extend(super::super::highlight::highlighted_spans_owned(
            &row.title, positions, base,
        ));
        if row.linked {
            let marker_style = if row.dimmed {
                base
            } else {
                Style::default().fg(Theme::accent())
            };
            spans.push(Span::styled(LINKED_MARKER, marker_style));
        }
        Line::from(spans)
    }

    fn legacy_status_color(status: TaskStatus) -> ratatui::style::Color {
        match status {
            TaskStatus::Todo => Theme::text_primary(),
            TaskStatus::InProgress => Theme::accent(),
            TaskStatus::Done => Theme::text_muted(),
        }
    }

    fn legacy_body(rows: &[TaskRow], focused: bool) -> Paragraph<'_> {
        if rows.is_empty() {
            let text = if focused {
                "no tasks — n to add"
            } else {
                "no tasks"
            };
            return Paragraph::new(Line::from(Span::styled(
                text,
                Style::default().fg(Theme::text_muted()),
            )));
        }
        Paragraph::new(rows.iter().map(legacy_line).collect::<Vec<Line>>())
    }

    /// The differential cases: every row appearance the pane has.
    fn differential_rows() -> Vec<(&'static str, Vec<TaskRow>, bool)> {
        let row = |title: &str, status, selected, dimmed, linked, matches: Vec<usize>| TaskRow {
            title: title.to_string(),
            status,
            match_positions: matches,
            dimmed,
            selected,
            linked,
        };
        vec![
            ("empty and unfocused", Vec::new(), false),
            ("empty and focused", Vec::new(), true),
            (
                "one row per status",
                vec![
                    row("write it", TaskStatus::Todo, false, false, false, vec![]),
                    row(
                        "ship it",
                        TaskStatus::InProgress,
                        false,
                        false,
                        false,
                        vec![],
                    ),
                    row("done it", TaskStatus::Done, false, false, false, vec![]),
                ],
                false,
            ),
            (
                "the selected row",
                vec![
                    row("first", TaskStatus::Todo, true, false, false, vec![]),
                    row("second", TaskStatus::Done, false, false, false, vec![]),
                ],
                true,
            ),
            (
                "a search: matched and dimmed rows",
                vec![
                    row(
                        "host tests",
                        TaskStatus::Todo,
                        false,
                        false,
                        false,
                        vec![0, 5],
                    ),
                    row("nothing", TaskStatus::Todo, false, true, false, vec![]),
                    row(
                        "selected and matched",
                        TaskStatus::InProgress,
                        true,
                        false,
                        false,
                        vec![0, 9],
                    ),
                ],
                true,
            ),
            (
                "linked rows, including a dimmed one",
                vec![
                    row(
                        "live one",
                        TaskStatus::InProgress,
                        false,
                        false,
                        true,
                        vec![],
                    ),
                    row("dim link", TaskStatus::Todo, false, true, true, vec![]),
                ],
                false,
            ),
            (
                "a multi-byte title with a match inside it",
                vec![row(
                    "héllo — wörld",
                    TaskStatus::Todo,
                    false,
                    false,
                    false,
                    vec![0, 6],
                )],
                false,
            ),
        ]
    }

    fn render_both(rows: &[TaskRow], focused: bool, w: u16, h: u16) -> (Buffer, Buffer) {
        let area = Rect::new(0, 0, w, h);
        let mut tree_buf = Buffer::empty(area);
        // The anchor the real pane declares. Every case here fits its height, so
        // the window is the identity and the comparison stays about appearance —
        // which is the point: the pre-port renderer had no window to compare to.
        let cursor = rows.iter().position(|r| r.selected);
        super::super::plugin_pane::render_tree(
            &tasks_tree(rows, cursor, focused),
            area,
            &super::super::theme::current(),
            &FrameTable::default(),
            &mut tree_buf,
        );

        let mut legacy_buf = Buffer::empty(area);
        let mut terminal = Terminal::new(TestBackend::new(w, h)).unwrap();
        terminal
            .draw(|f| f.render_widget(legacy_body(rows, focused), area))
            .unwrap();
        legacy_buf.merge(terminal.backend().buffer());
        (tree_buf, legacy_buf)
    }

    /// The port's own guarantee: the tree paints the pane the pre-port renderer
    /// painted, cell for cell and style for style.
    #[test]
    fn the_view_tree_paints_what_the_span_renderer_painted() {
        for (name, rows, focused) in differential_rows() {
            let (tree, legacy) = render_both(&rows, focused, 30, 6);
            assert_eq!(
                tree, legacy,
                "the tree diverges from the span renderer for `{name}`"
            );
        }
    }

    /// The tree must be a description of content only: a pane's rows are fitted
    /// by [`task_rows`], so nothing in the tree may depend on a width — which
    /// shows up as a row whose text was padded to fill one.
    #[test]
    fn the_tree_carries_no_geometry() {
        let (_, rows, focused) = differential_rows().remove(4);
        fn assert_no_padding(node: &ViewNode) {
            if let ViewNode::Text { content, .. } = node {
                assert!(
                    !content.ends_with("  "),
                    "a padded run means the tree resolved a width: {content:?}"
                );
            }
            node.children().iter().for_each(assert_no_padding);
        }
        assert_no_padding(&tasks_tree(&rows, Some(0), focused));
    }

    /// `task_rows` is the only place a *width* is consulted, so what it does with
    /// one — fit a title, reserve the marker's room — is asserted here rather than
    /// through a rendered frame. Height is not consulted at all: the window is the
    /// renderer's, resolved from the tree's cursor.
    #[test]
    fn task_rows_fits_titles_and_reserves_the_marker() {
        let mut linked = entry("a very long task title indeed");
        linked.linked = true;
        let entries = vec![entry("a very long task title indeed"), linked];
        let state = TaskPaneState {
            entries: &entries,
            selected: 0,
            focus: FocusLevel::Inactive,
            preview_selected: false,
        };
        let rows = task_rows(&state, 12);
        // 12 columns less the glyph and its space: 10 characters of title.
        assert_eq!(rows[0].title.chars().count(), 10);
        assert!(rows[0].title.ends_with('…'));
        // The linked row gives up the marker's width as well, so the marker is
        // never pushed off the row.
        assert_eq!(
            rows[1].title.chars().count(),
            10 - LINKED_MARKER.chars().count()
        );
    }

    /// The tree hands the renderer the *whole* list plus the cursor's index, which
    /// is what lets a plugin reproducing this pane scroll without being told a
    /// height. A tree that windowed here would put the rule in two places.
    #[test]
    fn the_tree_carries_every_row_and_its_cursor() {
        let entries: Vec<TaskPaneEntry> = (0..10).map(|i| entry(&format!("t{i}"))).collect();
        let state = TaskPaneState {
            entries: &entries,
            selected: 9,
            focus: FocusLevel::Focused,
            preview_selected: false,
        };
        let rows = task_rows(&state, 20);
        assert_eq!(rows.len(), 10, "no row is windowed away");
        assert!(rows[9].selected, "the cursor's row is marked");
        assert!(!rows[0].selected);
        match tasks_tree(&rows, cursor_row(&state), true) {
            ViewNode::List {
                children, selected, ..
            } => {
                assert_eq!(children.len(), 10);
                assert_eq!(selected, Some(9), "the list declares its cursor");
            }
            other => panic!("expected a list, got {}", other.kind_name()),
        }
    }

    /// A selection left behind by a shortened list still has to scroll somewhere,
    /// so the anchor clamps rather than vanishing — the published snapshot clamps
    /// with it, which is what keeps the two panes windowing alike.
    #[test]
    fn the_cursor_clamps_into_a_shortened_list() {
        let entries = vec![entry("only")];
        let state = TaskPaneState {
            entries: &entries,
            selected: 7,
            focus: FocusLevel::Focused,
            preview_selected: false,
        };
        assert_eq!(cursor_row(&state), Some(0));
        let empty = TaskPaneState {
            entries: &[],
            ..state
        };
        assert_eq!(cursor_row(&empty), None);
    }

    /// An unfocused pane with no preview marks no row, so the cursor is invisible
    /// until the pane can be driven — the rule the tree cannot see for itself.
    #[test]
    fn an_unfocused_pane_marks_no_selected_row() {
        let entries = vec![entry("a"), entry("b")];
        let state = TaskPaneState {
            entries: &entries,
            selected: 1,
            focus: FocusLevel::Inactive,
            preview_selected: false,
        };
        assert!(task_rows(&state, 20).iter().all(|r| !r.selected));
        let previewed = TaskPaneState {
            preview_selected: true,
            ..state
        };
        assert!(task_rows(&previewed, 20)[1].selected);
        // The *anchor* is there either way: it is the row a list scrolls to, not
        // the row that is drawn as the cursor's.
        assert_eq!(cursor_row(&state), Some(1));
    }
}
