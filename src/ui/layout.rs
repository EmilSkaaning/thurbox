use ratatui::layout::{Constraint, Direction, Layout, Rect};

use crate::session::workspace_tree::{Axis, Node, Region, RegionId, Sizing};

#[derive(Debug, Clone, PartialEq, Eq)]
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
    /// Plugin-contributed panes, in the right column after the file viewer, in
    /// publication order. Present only in the wide layout, exactly like the
    /// native side panels — and shorter than the visible-pane count when the
    /// terminal has room for only some of them.
    pub plugin_panes: Vec<Rect>,
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

/// Narrowest the center may become before an **additional** plugin column is
/// dropped rather than placed.
///
/// It gates only the second pane onward: with one plugin pane the right column
/// is exactly the set the single-slot layout produced, narrow centers included,
/// and re-gating that would change a layout that already ships. Panes past the
/// first are new capacity, and four 20% columns take the center to zero — an
/// agent terminal that silently vanished when a plugin published a pane.
const CENTER_MIN_COLS: u16 = 20;

/// Which panels the layout should place, as a named structure.
///
/// Named rather than positional because the old signature took **nine**
/// arguments and every new pane widened it, forcing an edit at all 36 call
/// sites. With a struct plus `Default`, adding a panel is a field that existing
/// callers never mention — which is the whole reason a plugin can contribute a
/// pane without touching this file.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct LayoutParams {
    /// Show the left column, whose upper seat the session list occupies.
    ///
    /// "Show the seat", not "show the kernel's pane": since ADR-46 a plugin pane
    /// may occupy it instead, and the caller sets this when *either* occupant
    /// wants it. The geometry is the same either way, which is why seating
    /// changed no rect.
    pub show_session_list: bool,
    /// Show the column left of the terminal, whose seat the info panel occupies.
    pub show_info_panel: bool,
    /// Show the tasks panel in the right column.
    pub show_tasks_panel: bool,
    /// Show the file viewer in the right column.
    pub show_file_viewer: bool,
    /// How many plugin panes the **right column** should seat. Each gets its own
    /// region there: a plugin may publish several panes, so the layout counts
    /// them rather than asking whether there is one. A pane in any other slot is
    /// a single seat this struct already carries a flag for (ADR-46), so it is
    /// not counted here.
    pub plugin_panes: usize,
    /// Show the full-width global-search strip above the footer.
    pub show_global_search: bool,
    /// Show the band beneath the session list, whose seat the automations pane
    /// occupies.
    pub show_automations_pane: bool,
    /// Content rows that band's occupant has; drives its height. The automation
    /// count for the native pane, or the row count of a plugin pane seated there
    /// — the height policy is the kernel's either way (ADR-46).
    pub automation_count: usize,
    /// Show the transient full-width status-message band above the footer.
    pub show_status_row: bool,
}

impl LayoutParams {
    /// The right column's occupants, in the order they are drawn.
    ///
    /// Order is fixed by the host (tasks, then file viewer, then plugin panes)
    /// — a pane picks a column, not a position, so two panes can never
    /// disagree about who comes first. `plugin_columns` is how many plugin
    /// panes the terminal has room for, which is at most [`Self::plugin_panes`].
    fn right_regions(&self, plugin_columns: usize) -> Vec<RegionId> {
        let mut regions = Vec::new();
        if self.show_tasks_panel {
            regions.push(RegionId::Tasks);
        }
        if self.show_file_viewer {
            regions.push(RegionId::FileViewer);
        }
        regions.extend((0..plugin_columns).map(RegionId::Plugin));
        regions
    }

    /// Whether anything wants the wide three-column layout.
    fn wants_side_columns(&self) -> bool {
        self.show_info_panel
            || self.show_tasks_panel
            || self.show_file_viewer
            || self.plugin_panes > 0
    }
}

/// Every region the tree placed, in draw order.
///
/// A region id appears at most once, so a lookup can never get two answers.
struct Placements(Vec<(RegionId, Rect)>);

impl Placements {
    /// The rect the tree gave `id`, or `None` when the tree did not place it.
    fn rect(&self, id: RegionId) -> Option<Rect> {
        self.0.iter().find(|(r, _)| *r == id).map(|(_, rect)| *rect)
    }

    /// A band the tree placed but sized to nothing.
    ///
    /// The vertical bands are always in the tree — a hidden one is a zero-cell
    /// child rather than an omitted one, so the solver keeps seeing the same
    /// five constraints — and it is this projection that reads zero extent as
    /// "not shown".
    fn band(&self, id: RegionId) -> Option<Rect> {
        self.rect(id).filter(|r| r.height > 0)
    }
}

/// Solve a workspace tree against `area`, yielding each leaf's rect.
///
/// Each branch hands its children's [`Sizing`] to ratatui as the matching
/// [`Constraint`], so the arithmetic — including how percentages round — is the
/// same call the fixed-rect layout made. That is why the synthesized default
/// preset reproduces the previous geometry by construction rather than by
/// inspection.
fn solve(node: &Node, area: Rect) -> Placements {
    let mut out = Vec::new();
    place(node, area, &mut out);
    Placements(out)
}

fn place(node: &Node, area: Rect, out: &mut Vec<(RegionId, Rect)>) {
    match &node.region {
        Region::Pane(id) => out.push((*id, area)),
        Region::Split { axis, children } => {
            let constraints: Vec<Constraint> =
                children.iter().map(|c| constraint_for(c.sizing)).collect();
            let direction = match axis {
                Axis::Horizontal => Direction::Horizontal,
                Axis::Vertical => Direction::Vertical,
            };
            let rects = Layout::default()
                .direction(direction)
                .constraints(constraints)
                .split(area);
            for (child, rect) in children.iter().zip(rects.iter()) {
                place(child, *rect, out);
            }
        }
    }
}

fn constraint_for(sizing: Sizing) -> Constraint {
    match sizing {
        Sizing::Cells(n) => Constraint::Length(n),
        Sizing::Percent(n) => Constraint::Percentage(n),
        Sizing::Fill { min } => Constraint::Min(min),
    }
}

/// Rows the header band takes: none below 20 rows, so a short terminal gives
/// every row it has to the content and footer.
fn header_rows(area: Rect) -> u16 {
    if area.height < 20 {
        0
    } else {
        1
    }
}

/// Rows the global-search strip takes, clamped so the content band and footer
/// always keep a row between them.
fn global_search_rows(area: Rect, header: u16, show: bool) -> u16 {
    if show {
        GLOBAL_SEARCH_HEIGHT.min(area.height.saturating_sub(header + 1))
    } else {
        0
    }
}

/// The height the content band will resolve to.
///
/// Needed *before* the tree is finished, because the automations pane's height
/// is a function of the left column's height. It runs the same split the solver
/// will run — ratatui caches it — so the preset and the solve can never
/// disagree about how tall the column is.
fn content_band_rows(area: Rect, header: u16, search: u16, status: u16) -> u16 {
    Layout::default()
        .direction(Direction::Vertical)
        .constraints(band_constraints(header, search, status))
        .split(area)[1]
        .height
}

/// The five vertical bands, in order: header, content, search strip, status
/// row, footer. A hidden band is `Length(0)` rather than absent so the
/// constraint list never changes shape.
fn band_constraints(header: u16, search: u16, status: u16) -> [Constraint; 5] {
    [
        Constraint::Length(header),
        Constraint::Min(1),
        Constraint::Length(search),
        Constraint::Length(status),
        Constraint::Length(1),
    ]
}

/// The left column: its upper seat, with the lower band beneath it when that band
/// is enabled and the column is tall enough for both.
///
/// The band's height grows with `automation_count` — its occupant's content rows,
/// whichever pane that is — between a minimum (so an empty band is still
/// discoverable) and a cap; below the minimum the column is the upper seat alone
/// rather than two unusably short lists.
fn left_column(sizing: Sizing, p: LayoutParams, column_rows: u16) -> Node {
    if !p.show_automations_pane {
        return Node::pane(sizing, RegionId::SessionList);
    }
    let desired =
        (p.automation_count as u16 + 2).clamp(AUTOMATIONS_PANE_MIN_ROWS, AUTOMATIONS_PANE_MAX_ROWS);
    let rows = desired.min(column_rows.saturating_sub(SESSIONS_MIN_ROWS));
    if rows < AUTOMATIONS_PANE_MIN_ROWS {
        return Node::pane(sizing, RegionId::SessionList);
    }
    Node::split(
        sizing,
        Axis::Vertical,
        vec![
            Node::pane(
                Sizing::Fill {
                    min: SESSIONS_MIN_ROWS,
                },
                RegionId::SessionList,
            ),
            Node::pane(Sizing::Cells(rows), RegionId::Automations),
        ],
    )
}

/// The wide content band's children: `list? | info? | center | right…`.
///
/// `plugin_columns` is how many plugin panes to seat; the center holds the
/// remainder, so a hidden occupant's width goes there rather than leaving a gap.
fn wide_columns(p: LayoutParams, column_rows: u16, plugin_columns: usize) -> Vec<Node> {
    let mut children = Vec::new();
    if p.show_session_list {
        children.push(left_column(Sizing::Percent(18), p, column_rows));
    }
    if p.show_info_panel {
        children.push(Node::pane(Sizing::Percent(15), RegionId::Info));
    }
    children.push(Node::pane(Sizing::Fill { min: 0 }, RegionId::Center));
    for region in p.right_regions(plugin_columns) {
        children.push(Node::pane(Sizing::Percent(20), region));
    }
    children
}

/// How many of the visible plugin panes the wide layout can seat.
///
/// Each right-column occupant takes 20% and the center takes the remainder, so
/// over-subscribing starves the **center** — the fallback view, which must never
/// be hidden. The check is therefore "does the center survive", made by solving
/// the candidate column and dropping the last plugin pane until it does, per
/// [`CENTER_MIN_COLS`]. The first pane is never dropped this way: it is the
/// layout the single-slot version already produced.
fn plugin_columns_that_fit(area: Rect, p: LayoutParams, column_rows: u16) -> usize {
    let mut columns = p.plugin_panes;
    let band = Rect::new(area.x, area.y, area.width, column_rows);
    while columns > 1 {
        let candidate = Node::split(
            Sizing::Fill { min: 1 },
            Axis::Horizontal,
            wide_columns(p, column_rows, columns),
        );
        let center = solve(&candidate, band)
            .rect(RegionId::Center)
            .unwrap_or_default();
        if center.width >= CENTER_MIN_COLS {
            break;
        }
        columns -= 1;
    }
    columns
}

/// The content band: the columns between the header and the bottom strips.
///
/// Three shapes, picked by width exactly as before: the center alone below the
/// two-panel minimum, `list | center` in between, and
/// `list? | info? | center | right…` at or above the three-panel minimum when
/// something actually wants a side column.
fn content_band(area: Rect, p: LayoutParams, column_rows: u16) -> Node {
    let sizing = Sizing::Fill { min: 1 };
    let settings = crate::session::settings::global();

    if area.width < settings.two_panel_min_cols {
        return Node::pane(sizing, RegionId::Center);
    }

    if area.width >= settings.three_panel_min_cols && p.wants_side_columns() {
        let columns = plugin_columns_that_fit(area, p, column_rows);
        return Node::split(
            sizing,
            Axis::Horizontal,
            wide_columns(p, column_rows, columns),
        );
    }

    if !p.show_session_list {
        return Node::pane(sizing, RegionId::Center);
    }
    Node::split(
        sizing,
        Axis::Horizontal,
        vec![
            left_column(Sizing::Percent(25), p, column_rows),
            Node::pane(Sizing::Percent(75), RegionId::Center),
        ],
    )
}

/// Synthesize the workspace tree for `area`.
///
/// This is the **default preset**: the tree that reproduces the fixed-rect
/// layout exactly. Every responsive rule lives here — the compact-mode header,
/// the two width thresholds, the automations pane's minimum column height —
/// leaving the solver to do nothing but divide rects.
fn default_preset(area: Rect, p: LayoutParams) -> Node {
    let header = header_rows(area);
    let search = global_search_rows(area, header, p.show_global_search);
    // One transient row for the active status/error message, directly above the
    // footer (keeping the pills pinned to the bottom edge). Carved only while a
    // message is showing, so content shrinks by 1 only transiently.
    let status = u16::from(p.show_status_row);
    let column_rows = content_band_rows(area, header, search, status);

    Node::split(
        Sizing::Fill { min: 0 },
        Axis::Vertical,
        vec![
            Node::pane(Sizing::Cells(header), RegionId::Header),
            content_band(area, p, column_rows),
            // Full width, above the footer, so every column shrinks to make
            // room — the same way the optional right-side panels share the
            // content width.
            Node::pane(Sizing::Cells(search), RegionId::GlobalSearch),
            Node::pane(Sizing::Cells(status), RegionId::StatusMessage),
            Node::pane(Sizing::Cells(1), RegionId::Footer),
        ],
    )
}

/// Compute panel layout areas based on terminal dimensions and optional
/// right-side panel visibility.
///
/// Geometry is described by a **workspace tree** (`session::workspace_tree`):
/// `default_preset` synthesizes it from `p`, `solve` divides the rects, and
/// [`PanelAreas`] is the projection the view reads. Adding a pane is a leaf in
/// the tree rather than a rect field and a branch through the split.
///
/// At width ≥ 120, the layout becomes
/// `list? | info? | terminal | tasks? | file_viewer?` with info (15%), tasks
/// (20%), and file_viewer (20%) appearing only when requested. The tasks panel
/// sits between the terminal and the file viewer (both right-side columns). The
/// left column is further split into an upper seat (the session list) and a band
/// beneath it (the automations pane) whenever the column is tall enough and
/// `show_automations_pane` is set — false when the `automations` feature flag is
/// off and nothing else claims that seat; `automation_count` only sizes the band.
/// When `show_session_list` is false the whole left column (both seats) is dropped
/// and the terminal expands — the right-side panels are unaffected.
///
/// `show_status_row` carves a transient full-width 1-row band directly above the
/// footer for the active status/error message (or the sync spinner), so a long
/// message is never clipped by the right-aligned footer pills. It shrinks the
/// content region by one row while shown (mirroring `show_global_search`).
pub fn compute_layout(area: Rect, p: LayoutParams) -> PanelAreas {
    let placed = solve(&default_preset(area, p), area);
    PanelAreas {
        // Header, center and footer are in every tree the preset builds (the
        // header is simply zero rows tall in compact mode), which
        // `preset_always_places_the_mandatory_regions` pins. A zero rect rather
        // than an unwrap keeps a preset bug a blank band instead of a panic
        // halfway through painting a frame.
        header: placed.rect(RegionId::Header).unwrap_or_default(),
        left_panel: placed.rect(RegionId::SessionList),
        automations_panel: placed.rect(RegionId::Automations),
        info_panel: placed.rect(RegionId::Info),
        tasks_panel: placed.rect(RegionId::Tasks),
        file_viewer: placed.rect(RegionId::FileViewer),
        plugin_panes: (0..)
            .map_while(|i| placed.rect(RegionId::Plugin(i)))
            .collect(),
        global_search: placed.band(RegionId::GlobalSearch),
        status_message: placed.band(RegionId::StatusMessage),
        terminal: placed.rect(RegionId::Center).unwrap_or_default(),
        footer: placed.rect(RegionId::Footer).unwrap_or_default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn area(width: u16, height: u16) -> Rect {
        Rect::new(0, 0, width, height)
    }

    /// The single plugin region, asserting there is exactly one.
    fn only_plugin_pane(areas: &PanelAreas) -> Rect {
        assert_eq!(
            areas.plugin_panes.len(),
            1,
            "expected one plugin region: {:?}",
            areas.plugin_panes
        );
        areas.plugin_panes[0]
    }

    #[test]
    fn solver_nests_a_split_inside_its_parent() {
        let tree = Node::split(
            Sizing::Fill { min: 0 },
            Axis::Vertical,
            vec![
                Node::pane(Sizing::Cells(1), RegionId::Header),
                Node::split(
                    Sizing::Fill { min: 1 },
                    Axis::Horizontal,
                    vec![
                        Node::pane(Sizing::Percent(25), RegionId::SessionList),
                        Node::pane(Sizing::Fill { min: 0 }, RegionId::Center),
                    ],
                ),
            ],
        );
        let placed = solve(&tree, area(80, 24));
        let header = placed.rect(RegionId::Header).unwrap();
        let list = placed.rect(RegionId::SessionList).unwrap();
        let center = placed.rect(RegionId::Center).unwrap();

        // The inner split's children share the band below the header, side by
        // side, and cover it exactly.
        assert_eq!(header.height, 1);
        assert_eq!((list.y, list.height), (1, 23));
        assert_eq!((center.y, center.height), (1, 23));
        assert_eq!(list.x, 0);
        assert_eq!(center.x, list.x + list.width);
        assert_eq!(center.x + center.width, 80);
    }

    #[test]
    fn solver_keeps_a_fixed_child_while_a_fill_sibling_grows() {
        let tree = Node::split(
            Sizing::Fill { min: 0 },
            Axis::Vertical,
            vec![
                Node::pane(Sizing::Cells(3), RegionId::Automations),
                Node::pane(Sizing::Fill { min: 0 }, RegionId::Center),
            ],
        );
        let short = solve(&tree, area(40, 10));
        let tall = solve(&tree, area(40, 30));
        assert_eq!(short.rect(RegionId::Automations).unwrap().height, 3);
        assert_eq!(tall.rect(RegionId::Automations).unwrap().height, 3);
        assert_eq!(short.rect(RegionId::Center).unwrap().height, 7);
        assert_eq!(tall.rect(RegionId::Center).unwrap().height, 27);
    }

    #[test]
    fn solver_gives_a_zero_cell_child_no_extent() {
        let tree = Node::split(
            Sizing::Fill { min: 0 },
            Axis::Vertical,
            vec![
                Node::pane(Sizing::Cells(0), RegionId::Header),
                Node::pane(Sizing::Fill { min: 1 }, RegionId::Center),
                Node::pane(Sizing::Cells(1), RegionId::Footer),
            ],
        );
        let placed = solve(&tree, area(40, 10));
        // Zero rows, and the content starts at the top as if it were absent.
        assert_eq!(placed.rect(RegionId::Header).unwrap().height, 0);
        assert_eq!(placed.rect(RegionId::Center).unwrap().y, 0);
        assert_eq!(placed.rect(RegionId::Center).unwrap().height, 9);
    }

    #[test]
    fn solving_is_deterministic() {
        let tree = default_preset(
            area(160, 40),
            LayoutParams {
                show_session_list: true,
                show_info_panel: true,
                show_file_viewer: true,
                show_automations_pane: true,
                automation_count: 3,
                ..Default::default()
            },
        );
        let first = solve(&tree, area(160, 40)).0;
        let second = solve(&tree, area(160, 40)).0;
        assert_eq!(first, second);
    }

    #[test]
    fn preset_always_places_the_mandatory_regions() {
        // Whatever is hidden, the three regions `compute_layout` projects
        // unconditionally must be in the tree — over a spread of sizes that
        // covers compact mode and both width thresholds.
        for (w, h) in [(40, 10), (79, 19), (100, 24), (120, 40), (200, 60)] {
            for params in [
                LayoutParams::default(),
                LayoutParams {
                    show_session_list: true,
                    show_info_panel: true,
                    show_tasks_panel: true,
                    show_file_viewer: true,
                    plugin_panes: 1,
                    show_global_search: true,
                    show_automations_pane: true,
                    automation_count: 4,
                    show_status_row: true,
                },
            ] {
                let ids = default_preset(area(w, h), params).region_ids();
                for required in [RegionId::Header, RegionId::Center, RegionId::Footer] {
                    assert!(
                        ids.contains(&required),
                        "{w}x{h} {params:?} lost {required:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn preset_names_each_region_at_most_once() {
        let ids = default_preset(
            area(200, 60),
            LayoutParams {
                show_session_list: true,
                show_info_panel: true,
                show_tasks_panel: true,
                show_file_viewer: true,
                plugin_panes: 1,
                show_global_search: true,
                show_automations_pane: true,
                automation_count: 4,
                show_status_row: true,
            },
        )
        .region_ids();
        let mut unique = ids.clone();
        unique.sort();
        unique.dedup();
        assert_eq!(ids.len(), unique.len(), "a region is placed twice: {ids:?}");
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
                plugin_panes: 1,
                ..Default::default()
            },
        );
        let tp = areas.tasks_panel.expect("tasks panel shown");
        let fv = areas.file_viewer.expect("file viewer shown");
        let pp = only_plugin_pane(&areas);
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
                plugin_panes: 1,
                ..Default::default()
            },
        );
        let without_tasks = compute_layout(
            area(200, 24),
            LayoutParams {
                plugin_panes: 1,
                ..Default::default()
            },
        );

        let term = without_tasks.terminal;
        let pp = only_plugin_pane(&without_tasks);
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
        assert_eq!(pp.width, only_plugin_pane(&with_tasks).width);
    }

    #[test]
    fn a_plugin_pane_alone_still_gets_the_wide_layout() {
        let areas = compute_layout(
            area(200, 24),
            LayoutParams {
                show_session_list: true,
                plugin_panes: 1,
                ..Default::default()
            },
        );
        assert_eq!(areas.plugin_panes.len(), 1);
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
                plugin_panes: 1,
                ..Default::default()
            },
        );
        assert!(areas.plugin_panes.is_empty());
    }

    #[test]
    fn two_visible_plugin_panes_get_two_regions() {
        let areas = compute_layout(
            area(200, 24),
            LayoutParams {
                show_session_list: true,
                plugin_panes: 2,
                ..Default::default()
            },
        );
        assert_eq!(areas.plugin_panes.len(), 2, "both panes are seated");
        let (first, second) = (areas.plugin_panes[0], areas.plugin_panes[1]);
        let term = areas.terminal;
        assert_eq!(
            first.x,
            term.x + term.width,
            "first pane right of the center"
        );
        assert_eq!(second.x, first.x + first.width, "second right of the first");
        assert_eq!(second.x + second.width, 200, "the pair ends at the edge");
    }

    #[test]
    fn hiding_one_plugin_pane_leaves_no_gap() {
        let two = compute_layout(
            area(200, 24),
            LayoutParams {
                plugin_panes: 2,
                ..Default::default()
            },
        );
        let one = compute_layout(
            area(200, 24),
            LayoutParams {
                plugin_panes: 1,
                ..Default::default()
            },
        );
        assert_eq!(one.plugin_panes.len(), 1);
        let pane = one.plugin_panes[0];
        // The remaining pane keeps its width and butts against the center,
        // which absorbed the hidden one (it holds the `Min(0)` slot).
        assert_eq!(pane.width, two.plugin_panes[0].width);
        assert_eq!(pane.x, one.terminal.x + one.terminal.width, "no gap");
        assert!(one.terminal.width > two.terminal.width);
    }

    #[test]
    fn extra_plugin_columns_are_dropped_when_the_center_would_starve() {
        // Five 20% occupants would leave the center nothing at any width.
        let areas = compute_layout(
            area(200, 24),
            LayoutParams {
                show_tasks_panel: true,
                show_file_viewer: true,
                plugin_panes: 3,
                ..Default::default()
            },
        );
        assert_eq!(areas.plugin_panes.len(), 2, "the third column is dropped");
        assert!(
            areas.terminal.width >= CENTER_MIN_COLS,
            "the center survives: {}",
            areas.terminal.width
        );
    }

    #[test]
    fn widening_restores_a_dropped_plugin_column() {
        // Three 20% occupants beside the list and info leave the center 7% of
        // the width — under the floor at 200 cols, over it at 400.
        let params = LayoutParams {
            show_session_list: true,
            show_info_panel: true,
            show_tasks_panel: true,
            plugin_panes: 2,
            ..Default::default()
        };
        assert_eq!(compute_layout(area(200, 24), params).plugin_panes.len(), 1);
        assert_eq!(compute_layout(area(400, 24), params).plugin_panes.len(), 2);
    }

    #[test]
    fn one_plugin_pane_is_placed_even_when_the_center_starves() {
        // The single-slot layout allowed this (four occupants at the wide
        // threshold), so the floor must not retroactively drop it.
        let areas = compute_layout(
            area(120, 40),
            LayoutParams {
                show_session_list: true,
                show_info_panel: true,
                show_tasks_panel: true,
                show_file_viewer: true,
                plugin_panes: 1,
                ..Default::default()
            },
        );
        assert_eq!(areas.plugin_panes.len(), 1);
        assert!(
            areas.terminal.width < CENTER_MIN_COLS,
            "the case only proves anything while the center is starved: {}",
            areas.terminal.width
        );
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
                plugin_panes: 1,
                show_automations_pane: true,
                ..Default::default()
            },
        );
        let mut rects: Vec<Rect> = vec![areas.terminal];
        rects.extend(
            [
                areas.left_panel,
                areas.info_panel,
                areas.tasks_panel,
                areas.file_viewer,
            ]
            .into_iter()
            .flatten(),
        );
        rects.extend(areas.plugin_panes.iter().copied());
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
        assert!(areas.plugin_panes.is_empty());
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
                plugin_panes: 0,
                show_global_search: false,
                show_automations_pane: false,
                automation_count: 0,
                show_status_row: false,
            },
        );
        assert_eq!(implicit, explicit);
    }
}
