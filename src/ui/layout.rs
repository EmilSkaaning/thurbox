use ratatui::layout::{Constraint, Direction, Layout, Rect};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PanelAreas {
    pub header: Rect,
    /// Session list area (top of the left column).
    pub left_panel: Option<Rect>,
    /// Automations pane, below the session list in the left column. Present
    /// (even with zero automations) as long as the automations feature is
    /// enabled and the column is tall enough to fit both lists; its height
    /// grows with the automation count.
    pub automations_panel: Option<Rect>,
    pub info_panel: Option<Rect>,
    /// Tasks panel — a toggleable column on the right, between the terminal and
    /// the file viewer (behaves like the file viewer).
    pub tasks_panel: Option<Rect>,
    pub file_viewer: Option<Rect>,
    /// A plugin-contributed pane, in the right column after the file viewer.
    /// Present only in the wide layout, exactly like the native side panels.
    pub plugin_pane: Option<Rect>,
    /// Global search strip — full-width, docked along the bottom (above the
    /// footer) when active.
    pub global_search: Option<Rect>,
    /// Full-width transient band for the active status/error message (or the
    /// sync spinner), docked directly above the footer. Present only while a
    /// message is showing, so nothing is clipped by the footer pills.
    pub status_message: Option<Rect>,
    pub terminal: Rect,
    pub footer: Rect,
}

/// Rows the global-search strip occupies: a 2-row border around a query line, a
/// per-scope match summary, a scrollable result list (~7 rows), and a key-hint
/// line. Matches also highlight live in the panels behind the strip.
const GLOBAL_SEARCH_HEIGHT: u16 = 12;

/// Max rows (including borders) the automations pane may occupy.
const AUTOMATIONS_PANE_MAX_ROWS: u16 = 10;
/// Minimum rows the automations pane occupies (border + one content row), so it
/// stays visible even with zero automations.
const AUTOMATIONS_PANE_MIN_ROWS: u16 = 3;
/// Minimum rows the session list keeps when the automations pane is shown.
const SESSIONS_MIN_ROWS: u16 = 3;

/// Split a left-column rect into (sessions, automations). The automations pane
/// is always present (its height grows with `automation_count`, with a minimum
/// so an empty pane still shows) unless the column is too short for both lists.
fn split_left_column(col: Rect, automation_count: usize) -> (Rect, Option<Rect>) {
    let desired =
        (automation_count as u16 + 2).clamp(AUTOMATIONS_PANE_MIN_ROWS, AUTOMATIONS_PANE_MAX_ROWS);
    let auto_h = desired.min(col.height.saturating_sub(SESSIONS_MIN_ROWS));
    if auto_h < AUTOMATIONS_PANE_MIN_ROWS {
        return (col, None); // not enough vertical room — keep sessions only
    }
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(SESSIONS_MIN_ROWS),
            Constraint::Length(auto_h),
        ])
        .split(col);
    (rows[0], Some(rows[1]))
}

/// Vertical bands carved from the full area: header, content region, optional
/// global-search strip, optional status-message row, and footer.
struct VerticalBands {
    header: Rect,
    content: Rect,
    global_search: Option<Rect>,
    status_message: Option<Rect>,
    footer: Rect,
}

/// Split the full area into header / content / global-search / status-message /
/// footer bands.
fn split_vertical(area: Rect, show_global_search: bool, show_status_row: bool) -> VerticalBands {
    // Compact mode: when the terminal is shorter than 20 rows, drop the
    // header line entirely so the content + footer get every row available.
    let header_height = if area.height < 20 { 0 } else { 1 };

    // The global-search strip is carved from the bottom of the content region
    // (full width, above the footer) so every column shrinks to make room — the
    // same way the optional right-side panels share the content width.
    let search_height = if show_global_search {
        GLOBAL_SEARCH_HEIGHT.min(area.height.saturating_sub(header_height + 1))
    } else {
        0
    };

    // One transient row for the active status/error message, directly above the
    // footer (keeping the pills pinned to the bottom edge). Carved only while a
    // message is showing, so content shrinks by 1 only transiently.
    let status_height = if show_status_row { 1 } else { 0 };

    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(header_height),
            Constraint::Min(1),
            Constraint::Length(search_height),
            Constraint::Length(status_height),
            Constraint::Length(1),
        ])
        .split(area);

    VerticalBands {
        header: vertical[0],
        content: vertical[1],
        global_search: (search_height > 0).then_some(vertical[2]),
        status_message: (status_height > 0).then_some(vertical[3]),
        footer: vertical[4],
    }
}

/// Split a left-column rect into (session list, automations pane) honouring the
/// `show_automations_pane` flag.
fn left_column_split(
    col: Rect,
    show_automations_pane: bool,
    automation_count: usize,
) -> (Rect, Option<Rect>) {
    if show_automations_pane {
        split_left_column(col, automation_count)
    } else {
        (col, None)
    }
}

/// Build the wide (≥ three_panel_min_cols) layout with optional info / tasks /
/// file-viewer columns. Column order: list? | info? | terminal | tasks? |
/// Which panels the layout should place, as a named structure.
///
/// Named rather than positional because the old signature took **nine**
/// arguments and every new pane widened it, forcing an edit at all 36 call
/// sites. With a struct plus `Default`, adding a panel is a field that existing
/// callers never mention — which is the whole reason a plugin can contribute a
/// pane without touching this file.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct LayoutParams {
    /// Show the session list (and, with it, the whole left column).
    pub show_session_list: bool,
    /// Show the info panel, left of the terminal.
    pub show_info_panel: bool,
    /// Show the tasks panel in the right column.
    pub show_tasks_panel: bool,
    /// Show the file viewer in the right column.
    pub show_file_viewer: bool,
    /// Show a plugin-contributed pane in the right column.
    pub show_plugin_pane: bool,
    /// Show the full-width global-search strip above the footer.
    pub show_global_search: bool,
    /// Show the automations pane beneath the session list.
    pub show_automations_pane: bool,
    /// How many automations there are; drives the automations pane's height.
    pub automation_count: usize,
    /// Show the transient full-width status-message band above the footer.
    pub show_status_row: bool,
}

/// One occupant of the right-hand column.
///
/// The column is built as an **ordered list** of these rather than a fixed set
/// of named rects, so a hidden occupant leaves no gap and a new one is a list
/// entry rather than another branch threaded through the split.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RightSlot {
    Tasks,
    FileViewer,
    Plugin,
}

impl LayoutParams {
    /// The right column's occupants, in the order they are drawn.
    ///
    /// Order is fixed by the host (tasks, then file viewer, then plugin panes)
    /// — a pane picks a column, not a position, so two panes can never
    /// disagree about who comes first.
    fn right_slots(&self) -> Vec<RightSlot> {
        let mut slots = Vec::new();
        if self.show_tasks_panel {
            slots.push(RightSlot::Tasks);
        }
        if self.show_file_viewer {
            slots.push(RightSlot::FileViewer);
        }
        if self.show_plugin_pane {
            slots.push(RightSlot::Plugin);
        }
        slots
    }

    /// Whether anything wants the wide three-column layout.
    fn wants_side_columns(&self) -> bool {
        self.show_info_panel || !self.right_slots().is_empty()
    }
}

/// file_viewer?. The list column is omitted entirely when
/// `show_session_list` is false (the terminal expands to fill the freed width),
/// but the right-side columns are unaffected.
fn three_panel_layout(bands: &VerticalBands, content: Rect, p: LayoutParams) -> PanelAreas {
    let right = p.right_slots();
    let mut constraints: Vec<Constraint> = Vec::new();
    if p.show_session_list {
        constraints.push(Constraint::Percentage(18));
    }
    if p.show_info_panel {
        constraints.push(Constraint::Percentage(15));
    }
    // terminal takes the remainder
    let terminal_idx = constraints.len();
    constraints.push(Constraint::Min(0));
    for _ in &right {
        constraints.push(Constraint::Percentage(20));
    }

    let horizontal = Layout::default()
        .direction(Direction::Horizontal)
        .constraints(constraints)
        .split(content);

    // Walk the split left→right. The list column (when present) is index 0,
    // followed by info; the terminal sits at `terminal_idx` regardless of
    // whether the list column was emitted.
    let mut idx = 0;
    let (left_panel, automations_panel) = if p.show_session_list {
        let (lp, ap) =
            left_column_split(horizontal[idx], p.show_automations_pane, p.automation_count);
        idx += 1;
        (Some(lp), ap)
    } else {
        (None, None)
    };
    let info_panel = p.show_info_panel.then(|| {
        let r = horizontal[idx];
        idx += 1;
        r
    });
    let terminal = horizontal[terminal_idx];

    // The right column is assigned by walking its occupant list, so a hidden
    // occupant simply is not in the list and leaves no gap behind it.
    let mut tasks_panel = None;
    let mut file_viewer = None;
    let mut plugin_pane = None;
    for (offset, slot) in right.iter().enumerate() {
        let rect = horizontal[terminal_idx + 1 + offset];
        match slot {
            RightSlot::Tasks => tasks_panel = Some(rect),
            RightSlot::FileViewer => file_viewer = Some(rect),
            RightSlot::Plugin => plugin_pane = Some(rect),
        }
    }

    PanelAreas {
        header: bands.header,
        left_panel,
        automations_panel,
        info_panel,
        tasks_panel,
        file_viewer,
        plugin_pane,
        global_search: bands.global_search,
        status_message: bands.status_message,
        terminal,
        footer: bands.footer,
    }
}

/// Build the 2-panel layout: 25% list | 75% terminal. When the session list
/// is hidden the terminal takes the full content width (no list column).
fn two_panel_layout(
    bands: &VerticalBands,
    content: Rect,
    show_session_list: bool,
    show_automations_pane: bool,
    automation_count: usize,
) -> PanelAreas {
    if !show_session_list {
        return PanelAreas {
            header: bands.header,
            left_panel: None,
            automations_panel: None,
            info_panel: None,
            tasks_panel: None,
            file_viewer: None,
            plugin_pane: None,
            global_search: bands.global_search,
            status_message: bands.status_message,
            terminal: content,
            footer: bands.footer,
        };
    }
    let horizontal = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(25), Constraint::Percentage(75)])
        .split(content);

    let (left_panel, automations_panel) =
        left_column_split(horizontal[0], show_automations_pane, automation_count);
    PanelAreas {
        header: bands.header,
        left_panel: Some(left_panel),
        automations_panel,
        info_panel: None,
        tasks_panel: None,
        file_viewer: None,
        plugin_pane: None,
        global_search: bands.global_search,
        status_message: bands.status_message,
        terminal: horizontal[1],
        footer: bands.footer,
    }
}

/// Compute panel layout areas based on terminal dimensions and optional
/// right-side panel visibility.
///
/// At width ≥ 120, the layout becomes
/// `list? | info? | terminal | tasks? | file_viewer?` with info (15%), tasks
/// (20%), and file_viewer (20%) appearing only when requested. The tasks panel
/// sits between the terminal and the file viewer (both right-side columns). The
/// left column is further split into a session list and an automations pane
/// beneath it (whenever the column is tall enough and `show_automations_pane`
/// is set — false when the `automations` feature flag is off);
/// `automation_count` only sizes that pane. When `show_session_list` is false
/// the whole left column (sessions + automations) is dropped and the terminal
/// expands — the right-side panels are unaffected.
///
/// `show_status_row` carves a transient full-width 1-row band directly above the
/// footer for the active status/error message (or the sync spinner), so a long
/// message is never clipped by the right-aligned footer pills. It shrinks the
/// content region by one row while shown (mirroring `show_global_search`).
pub fn compute_layout(area: Rect, p: LayoutParams) -> PanelAreas {
    let bands = split_vertical(area, p.show_global_search, p.show_status_row);
    let content = bands.content;

    let settings = crate::session::settings::global();
    if area.width < settings.two_panel_min_cols {
        return PanelAreas {
            header: bands.header,
            left_panel: None,
            automations_panel: None,
            info_panel: None,
            tasks_panel: None,
            file_viewer: None,
            plugin_pane: None,
            global_search: bands.global_search,
            status_message: bands.status_message,
            terminal: content,
            footer: bands.footer,
        };
    }

    // At width ≥ three_panel_min_cols (default 120), support optional info /
    // tasks / file-viewer columns.
    if area.width >= settings.three_panel_min_cols && p.wants_side_columns() {
        return three_panel_layout(&bands, content, p);
    }

    two_panel_layout(
        &bands,
        content,
        p.show_session_list,
        p.show_automations_pane,
        p.automation_count,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn area(width: u16, height: u16) -> Rect {
        Rect::new(0, 0, width, height)
    }

    #[test]
    fn narrow_terminal_hides_left_panel() {
        let areas = compute_layout(
            area(79, 24),
            LayoutParams {
                show_session_list: true,
                show_automations_pane: true,
                ..Default::default()
            },
        );
        assert!(areas.left_panel.is_none());
        assert!(areas.info_panel.is_none());
        assert!(areas.file_viewer.is_none());
    }

    #[test]
    fn normal_width_shows_two_panels() {
        let areas = compute_layout(
            area(100, 24),
            LayoutParams {
                show_session_list: true,
                show_automations_pane: true,
                ..Default::default()
            },
        );
        assert!(areas.left_panel.is_some());
        assert!(areas.info_panel.is_none());
        assert!(areas.file_viewer.is_none());
    }

    #[test]
    fn wide_terminal_with_info_panel_shows_three_panels() {
        let areas = compute_layout(
            area(120, 24),
            LayoutParams {
                show_session_list: true,
                show_info_panel: true,
                show_automations_pane: true,
                ..Default::default()
            },
        );
        assert!(areas.left_panel.is_some());
        assert!(areas.info_panel.is_some());
        assert!(areas.file_viewer.is_none());
    }

    #[test]
    fn wide_terminal_without_info_panel_shows_two_panels() {
        let areas = compute_layout(
            area(120, 24),
            LayoutParams {
                show_session_list: true,
                show_automations_pane: true,
                ..Default::default()
            },
        );
        assert!(areas.left_panel.is_some());
        assert!(areas.info_panel.is_none());
        assert!(areas.file_viewer.is_none());
    }

    #[test]
    fn wide_terminal_with_file_viewer_only() {
        let areas = compute_layout(
            area(160, 24),
            LayoutParams {
                show_session_list: true,
                show_file_viewer: true,
                show_automations_pane: true,
                ..Default::default()
            },
        );
        assert!(areas.left_panel.is_some());
        assert!(areas.info_panel.is_none());
        assert!(areas.file_viewer.is_some());
    }

    #[test]
    fn wide_terminal_with_info_and_file_viewer() {
        let areas = compute_layout(
            area(160, 24),
            LayoutParams {
                show_session_list: true,
                show_info_panel: true,
                show_file_viewer: true,
                show_automations_pane: true,
                ..Default::default()
            },
        );
        assert!(areas.left_panel.is_some());
        assert!(areas.info_panel.is_some());
        assert!(areas.file_viewer.is_some());
        let term = areas.terminal;
        let fv = areas.file_viewer.unwrap();
        assert!(fv.x >= term.x + term.width);
    }

    #[test]
    fn wide_terminal_with_tasks_panel_only() {
        let areas = compute_layout(
            area(160, 24),
            LayoutParams {
                show_session_list: true,
                show_tasks_panel: true,
                show_automations_pane: true,
                ..Default::default()
            },
        );
        assert!(areas.left_panel.is_some());
        assert!(areas.info_panel.is_none());
        assert!(areas.tasks_panel.is_some());
        assert!(areas.file_viewer.is_none());
        let term = areas.terminal;
        let tp = areas.tasks_panel.unwrap();
        assert!(tp.x >= term.x + term.width);
    }

    #[test]
    fn tasks_panel_sits_left_of_file_viewer() {
        let areas = compute_layout(
            area(180, 24),
            LayoutParams {
                show_session_list: true,
                show_tasks_panel: true,
                show_file_viewer: true,
                show_automations_pane: true,
                ..Default::default()
            },
        );
        let term = areas.terminal;
        let tp = areas.tasks_panel.expect("tasks panel shown");
        let fv = areas.file_viewer.expect("file viewer shown");
        assert!(tp.x >= term.x + term.width, "tasks right of terminal");
        assert!(fv.x >= tp.x + tp.width, "file viewer right of tasks");
    }

    #[test]
    fn tasks_panel_ignored_below_120_cols() {
        let areas = compute_layout(
            area(119, 24),
            LayoutParams {
                show_session_list: true,
                show_tasks_panel: true,
                show_automations_pane: true,
                ..Default::default()
            },
        );
        assert!(areas.tasks_panel.is_none());
    }

    #[test]
    fn global_search_strip_absent_by_default() {
        let areas = compute_layout(
            area(120, 40),
            LayoutParams {
                show_session_list: true,
                show_automations_pane: true,
                ..Default::default()
            },
        );
        assert!(areas.global_search.is_none());
    }

    #[test]
    fn global_search_strip_present_when_active() {
        let areas = compute_layout(
            area(120, 40),
            LayoutParams {
                show_session_list: true,
                show_global_search: true,
                show_automations_pane: true,
                ..Default::default()
            },
        );
        let strip = areas.global_search.expect("strip shown when active");
        // Full width, carved directly above the footer.
        assert_eq!(strip.width, 120);
        assert_eq!(strip.x, 0);
        assert_eq!(strip.y + strip.height, areas.footer.y);
        assert_eq!(strip.height, GLOBAL_SEARCH_HEIGHT);
    }

    #[test]
    fn global_search_strip_shrinks_content() {
        let without = compute_layout(
            area(120, 40),
            LayoutParams {
                show_session_list: true,
                show_automations_pane: true,
                ..Default::default()
            },
        )
        .terminal;
        let with = compute_layout(
            area(120, 40),
            LayoutParams {
                show_session_list: true,
                show_global_search: true,
                show_automations_pane: true,
                ..Default::default()
            },
        )
        .terminal;
        // The terminal (content) region loses the strip's rows.
        assert_eq!(without.height - with.height, GLOBAL_SEARCH_HEIGHT);
    }

    #[test]
    fn status_row_absent_by_default() {
        let areas = compute_layout(
            area(120, 40),
            LayoutParams {
                show_session_list: true,
                show_automations_pane: true,
                ..Default::default()
            },
        );
        assert!(areas.status_message.is_none());
        assert_eq!(areas.footer.height, 1);
    }

    #[test]
    fn status_row_present_when_active() {
        let areas = compute_layout(
            area(120, 40),
            LayoutParams {
                show_session_list: true,
                show_automations_pane: true,
                show_status_row: true,
                ..Default::default()
            },
        );
        let row = areas
            .status_message
            .expect("row shown when a message is active");
        // Full width, one row, docked directly above the footer.
        assert_eq!(row.width, 120);
        assert_eq!(row.x, 0);
        assert_eq!(row.height, 1);
        assert_eq!(row.y + row.height, areas.footer.y);
    }

    #[test]
    fn status_row_shrinks_content_by_one() {
        let without = compute_layout(
            area(120, 40),
            LayoutParams {
                show_session_list: true,
                show_automations_pane: true,
                ..Default::default()
            },
        )
        .terminal;
        let with = compute_layout(
            area(120, 40),
            LayoutParams {
                show_session_list: true,
                show_automations_pane: true,
                show_status_row: true,
                ..Default::default()
            },
        )
        .terminal;
        assert_eq!(without.height - with.height, 1);
    }

    #[test]
    fn status_row_stacks_below_global_search() {
        // Both strips active: search on top, status row just above the footer.
        let areas = compute_layout(
            area(120, 40),
            LayoutParams {
                show_session_list: true,
                show_global_search: true,
                show_automations_pane: true,
                show_status_row: true,
                ..Default::default()
            },
        );
        let gs = areas.global_search.expect("search strip shown");
        let sm = areas.status_message.expect("status row shown");
        assert!(
            sm.y >= gs.y + gs.height,
            "status row sits below the search strip"
        );
        assert_eq!(
            sm.y + sm.height,
            areas.footer.y,
            "status row sits above the footer"
        );
    }

    #[test]
    fn header_and_footer_are_one_line() {
        let areas = compute_layout(
            area(100, 24),
            LayoutParams {
                show_session_list: true,
                show_automations_pane: true,
                ..Default::default()
            },
        );
        assert_eq!(areas.header.height, 1);
        assert_eq!(areas.footer.height, 1);
    }

    #[test]
    fn compact_mode_hides_header_below_20_rows() {
        let areas = compute_layout(
            area(100, 19),
            LayoutParams {
                show_session_list: true,
                show_automations_pane: true,
                ..Default::default()
            },
        );
        assert_eq!(areas.header.height, 0);
        assert_eq!(areas.footer.height, 1);
        assert!(areas.left_panel.is_some());
    }

    #[test]
    fn header_returns_at_20_rows() {
        let areas = compute_layout(
            area(100, 20),
            LayoutParams {
                show_session_list: true,
                show_automations_pane: true,
                ..Default::default()
            },
        );
        assert_eq!(areas.header.height, 1);
    }

    #[test]
    fn info_panel_ignored_below_120_cols() {
        let areas = compute_layout(
            area(119, 24),
            LayoutParams {
                show_session_list: true,
                show_info_panel: true,
                show_automations_pane: true,
                ..Default::default()
            },
        );
        assert!(areas.info_panel.is_none());
    }

    #[test]
    fn file_viewer_ignored_below_120_cols() {
        let areas = compute_layout(
            area(119, 24),
            LayoutParams {
                show_session_list: true,
                show_file_viewer: true,
                show_automations_pane: true,
                ..Default::default()
            },
        );
        assert!(areas.file_viewer.is_none());
    }

    fn terminal_inner(width: u16, height: u16, show_info: bool) -> (u16, u16) {
        use ratatui::widgets::{Block, Borders};
        let terminal = compute_layout(
            area(width, height),
            LayoutParams {
                show_session_list: true,
                show_info_panel: show_info,
                show_automations_pane: true,
                ..Default::default()
            },
        )
        .terminal;
        let inner = Block::default().borders(Borders::ALL).inner(terminal);
        (inner.height, inner.width)
    }

    #[test]
    fn two_panel_terminal_width_at_160_cols() {
        let (rows, cols) = terminal_inner(160, 40, false);
        assert_eq!(cols, 118);
        assert_eq!(rows, 36);
    }

    #[test]
    fn two_panel_terminal_width_at_80_cols() {
        let (rows, cols) = terminal_inner(80, 24, false);
        assert_eq!(cols, 58);
        assert_eq!(rows, 20);
    }

    #[test]
    fn three_panel_terminal_width_at_160_cols() {
        // 160 cols, list(18%)+info(15%)=33% reserved, terminal ≈ 67% (107) - 2 borders
        let (rows, cols) = terminal_inner(160, 40, true);
        assert!((100..=110).contains(&cols));
        assert_eq!(rows, 36);
    }

    #[test]
    fn narrow_terminal_uses_full_width() {
        let (rows, cols) = terminal_inner(60, 24, false);
        assert_eq!(cols, 58);
        assert_eq!(rows, 20);
    }

    #[test]
    fn automations_pane_present_even_when_empty() {
        // Zero automations still get a minimum-height pane (so it's discoverable).
        let areas = compute_layout(
            area(100, 24),
            LayoutParams {
                show_session_list: true,
                show_automations_pane: true,
                ..Default::default()
            },
        );
        assert!(areas.left_panel.is_some());
        let autos = areas.automations_panel.expect("empty pane still shown");
        assert_eq!(autos.height, AUTOMATIONS_PANE_MIN_ROWS);
    }

    #[test]
    fn automations_pane_appears_below_sessions() {
        let areas = compute_layout(
            area(100, 30),
            LayoutParams {
                show_session_list: true,
                show_automations_pane: true,
                automation_count: 2,
                ..Default::default()
            },
        );
        let sessions = areas.left_panel.unwrap();
        let autos = areas.automations_panel.expect("automations pane shown");
        assert_eq!(sessions.x, autos.x);
        assert_eq!(sessions.width, autos.width);
        assert_eq!(autos.y, sessions.y + sessions.height);
        // 2 automations + 2 border rows = 4 rows tall.
        assert_eq!(autos.height, 4);
        assert!(sessions.height >= SESSIONS_MIN_ROWS);
    }

    #[test]
    fn automations_pane_height_is_capped() {
        let areas = compute_layout(
            area(100, 60),
            LayoutParams {
                show_session_list: true,
                show_automations_pane: true,
                automation_count: 50,
                ..Default::default()
            },
        );
        assert_eq!(
            areas.automations_panel.unwrap().height,
            AUTOMATIONS_PANE_MAX_ROWS
        );
    }

    #[test]
    fn automations_pane_hidden_when_feature_disabled() {
        let with = compute_layout(
            area(100, 30),
            LayoutParams {
                show_session_list: true,
                show_automations_pane: true,
                automation_count: 2,
                ..Default::default()
            },
        );
        let without = compute_layout(
            area(100, 30),
            LayoutParams {
                show_session_list: true,
                automation_count: 2,
                ..Default::default()
            },
        );
        assert!(without.automations_panel.is_none());
        // The session list absorbs the whole left column.
        let full = without.left_panel.unwrap();
        let split = with.left_panel.unwrap();
        assert_eq!(
            full.height,
            split.height + with.automations_panel.unwrap().height
        );
    }

    #[test]
    fn automations_pane_hidden_when_column_too_short() {
        // Content height ≈ 4 rows leaves no room for both lists.
        let areas = compute_layout(
            area(100, 6),
            LayoutParams {
                show_session_list: true,
                show_automations_pane: true,
                automation_count: 3,
                ..Default::default()
            },
        );
        assert!(areas.left_panel.is_some());
        assert!(areas.automations_panel.is_none());
    }

    #[test]
    fn hidden_session_list_drops_left_column_two_panel() {
        // Two-panel width: hiding the list gives the terminal the full content
        // width (no 25% list column).
        let shown = compute_layout(
            area(100, 24),
            LayoutParams {
                show_session_list: true,
                show_automations_pane: true,
                ..Default::default()
            },
        );
        let hidden = compute_layout(
            area(100, 24),
            LayoutParams {
                show_automations_pane: true,
                ..Default::default()
            },
        );
        assert!(shown.left_panel.is_some());
        assert!(hidden.left_panel.is_none());
        assert!(hidden.automations_panel.is_none());
        assert!(hidden.terminal.width > shown.terminal.width);
    }

    #[test]
    fn hidden_session_list_keeps_right_side_panels() {
        // Hiding the left column must not drop the right-side panels — the
        // terminal expands into the list's freed width while info/tasks/files
        // stay put.
        let areas = compute_layout(
            area(160, 24),
            LayoutParams {
                show_info_panel: true,
                show_tasks_panel: true,
                show_file_viewer: true,
                show_automations_pane: true,
                ..Default::default()
            },
        );
        assert!(areas.left_panel.is_none(), "list column dropped");
        assert!(areas.automations_panel.is_none());
        assert!(areas.info_panel.is_some(), "info panel survives");
        assert!(areas.tasks_panel.is_some(), "tasks panel survives");
        assert!(areas.file_viewer.is_some(), "file viewer survives");
        let term = areas.terminal;
        // Terminal sits between the info column (left edge now) and tasks.
        let info = areas.info_panel.unwrap();
        let tasks = areas.tasks_panel.unwrap();
        assert!(
            term.x >= info.x + info.width,
            "terminal right of info: {term:?} vs {info:?}"
        );
        assert!(
            tasks.x >= term.x + term.width,
            "tasks right of terminal: {tasks:?} vs {term:?}"
        );
    }

    #[test]
    fn hidden_session_list_three_panel_terminal_widens() {
        // With info open and the list hidden, the terminal reclaims the 18%
        // the list would have reserved.
        let with_list = compute_layout(
            area(160, 24),
            LayoutParams {
                show_session_list: true,
                show_info_panel: true,
                show_automations_pane: true,
                ..Default::default()
            },
        )
        .terminal;
        let no_list = compute_layout(
            area(160, 24),
            LayoutParams {
                show_info_panel: true,
                show_automations_pane: true,
                ..Default::default()
            },
        )
        .terminal;
        assert!(
            no_list.width > with_list.width,
            "terminal widens when the list is hidden: {} vs {}",
            no_list.width,
            with_list.width
        );
    }

    #[test]
    fn a_plugin_pane_sits_after_the_file_viewer() {
        let areas = compute_layout(
            area(200, 24),
            LayoutParams {
                show_session_list: true,
                show_tasks_panel: true,
                show_file_viewer: true,
                show_plugin_pane: true,
                ..Default::default()
            },
        );
        let tp = areas.tasks_panel.expect("tasks panel shown");
        let fv = areas.file_viewer.expect("file viewer shown");
        let pp = areas.plugin_pane.expect("plugin pane shown");
        assert!(fv.x >= tp.x + tp.width, "file viewer right of tasks");
        assert!(pp.x >= fv.x + fv.width, "plugin pane right of file viewer");
    }

    #[test]
    fn a_hidden_occupant_leaves_no_gap() {
        // Tasks hidden: the plugin pane must move left into the space rather
        // than leaving a hole where tasks would have been.
        let with_tasks = compute_layout(
            area(200, 24),
            LayoutParams {
                show_tasks_panel: true,
                show_plugin_pane: true,
                ..Default::default()
            },
        );
        let without_tasks = compute_layout(
            area(200, 24),
            LayoutParams {
                show_plugin_pane: true,
                ..Default::default()
            },
        );

        let term = without_tasks.terminal;
        let pp = without_tasks.plugin_pane.expect("plugin pane shown");
        assert!(without_tasks.tasks_panel.is_none());
        // The pane butts straight up against the terminal: nothing is left
        // where the hidden occupant would have been.
        assert_eq!(pp.x, term.x + term.width, "no gap left behind");
        // The freed width goes to the terminal (it holds the `Min(0)` slot),
        // so the remaining occupants keep their size and position rather than
        // sliding left.
        assert!(
            term.width > with_tasks.terminal.width,
            "the terminal absorbs a hidden occupant's width"
        );
        assert_eq!(pp.width, with_tasks.plugin_pane.unwrap().width);
    }

    #[test]
    fn a_plugin_pane_alone_still_gets_the_wide_layout() {
        let areas = compute_layout(
            area(200, 24),
            LayoutParams {
                show_session_list: true,
                show_plugin_pane: true,
                ..Default::default()
            },
        );
        assert!(areas.plugin_pane.is_some());
        assert!(areas.tasks_panel.is_none());
        assert!(areas.file_viewer.is_none());
    }

    #[test]
    fn a_plugin_pane_is_dropped_below_the_wide_threshold() {
        // Same rule the native side panels follow: no room, not shown.
        let areas = compute_layout(
            area(119, 24),
            LayoutParams {
                show_session_list: true,
                show_plugin_pane: true,
                ..Default::default()
            },
        );
        assert!(areas.plugin_pane.is_none());
    }

    #[test]
    fn a_plugin_pane_never_overlaps_a_native_panel() {
        let areas = compute_layout(
            area(200, 40),
            LayoutParams {
                show_session_list: true,
                show_info_panel: true,
                show_tasks_panel: true,
                show_file_viewer: true,
                show_plugin_pane: true,
                show_automations_pane: true,
                ..Default::default()
            },
        );
        let mut rects: Vec<Rect> = vec![areas.terminal];
        for r in [
            areas.left_panel,
            areas.info_panel,
            areas.tasks_panel,
            areas.file_viewer,
            areas.plugin_pane,
        ]
        .into_iter()
        .flatten()
        {
            rects.push(r);
        }
        for (i, a) in rects.iter().enumerate() {
            for b in rects.iter().skip(i + 1) {
                let overlap = a.x < b.x + b.width
                    && b.x < a.x + a.width
                    && a.y < b.y + b.height
                    && b.y < a.y + a.height;
                assert!(!overlap, "{a:?} overlaps {b:?}");
            }
        }
    }

    #[test]
    fn default_params_show_no_optional_panel() {
        let areas = compute_layout(area(200, 24), LayoutParams::default());
        assert!(areas.left_panel.is_none());
        assert!(areas.info_panel.is_none());
        assert!(areas.tasks_panel.is_none());
        assert!(areas.file_viewer.is_none());
        assert!(areas.plugin_pane.is_none());
    }

    #[test]
    fn omitting_a_panel_matches_disabling_it_explicitly() {
        let implicit = compute_layout(
            area(200, 24),
            LayoutParams {
                show_session_list: true,
                ..Default::default()
            },
        );
        let explicit = compute_layout(
            area(200, 24),
            LayoutParams {
                show_session_list: true,
                show_info_panel: false,
                show_tasks_panel: false,
                show_file_viewer: false,
                show_plugin_pane: false,
                show_global_search: false,
                show_automations_pane: false,
                automation_count: 0,
                show_status_row: false,
            },
        );
        assert_eq!(implicit, explicit);
    }
}
