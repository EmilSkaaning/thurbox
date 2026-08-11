//! Persistent, focusable automations pane shown beneath the session list.
//!
//! Read-and-act: it mirrors `cached_automations` and, when focused, drives the
//! same toggle/run/edit/delete actions as the Ctrl+P modal.
//!
//! Like [`super::tasks_panel`], the pane renders through the **view tree**: it is
//! ordinary in-process Rust on the UI thread, but its rows are a
//! [`ViewNode`] that [`super::plugin_pane::render_tree`] paints. The split is the
//! one a plugin lives on:
//!
//! - [`resolve_rows`] is the **width** step — how wide a name may be once the
//!   marker and the summary tail have taken their columns. A plugin never learns
//!   its pane's width, so this stays the kernel's.
//! - [`automations_tree`] is the **presentation** step and carries no geometry:
//!   the enabled marker, the colour role, the selected/dimmed/resting precedence,
//!   the matched-run emphasis, the empty-state line. That is the part
//!   `tests/bundled_automations_panel.rs` asserts a bundled Luau plugin
//!   reproduces exactly.
//! - [`row_summary`] is the **composition** step, and it is `pub` because two
//!   consumers must not disagree about it: this pane and the `Ctrl+P` list modal
//!   both compose `<schedule> · <action> · <when>` from the same parts, and the
//!   plugin's own copy is compared against this function rather than against a
//!   third one written in a test.
//!
//! **Height is in none of them.** The tree carries every row plus the index of
//! the cursor's, and the *renderer* resolves the window — so the native pane and
//! a plugin's copy scroll through one implementation
//! (`ui::file_viewer::visible_window`) instead of two that could disagree. The
//! pane calls it a second time for its click hitboxes, which need the window as
//! numbers rather than as a paint.

use ratatui::{
    layout::Rect,
    style::Style,
    widgets::{Block, Borders},
    Frame,
};
// Only the retained pre-port oracle (`legacy_*`) still assembles the rows out of
// ratatui primitives and reaches for a palette colour by hand.
#[cfg(test)]
use ratatui::{
    text::{Line, Span},
    widgets::Paragraph,
};

use crate::session::motion::FrameTable;
use crate::session::view_tree::{StyleToken, TextStyle, ViewNode};

use super::theme::Theme;
use super::{truncate_ellipsis, FocusLevel};

/// One row in the automations pane (view data built by the app layer).
///
/// The summary arrives as its **parts** rather than as a finished string: the
/// kernel resolves what a sandboxed pane cannot (a cron expression's human label,
/// and a countdown, since a plugin VM has no clock) and the composition itself is
/// [`row_summary`]'s. Publishing the assembled string would make a pane an
/// arrangement of text the kernel formatted, which is the one thing
/// `session::pane_context` exists not to do.
pub struct AutomationPaneEntry {
    /// The automation's row id.
    ///
    /// Nothing in the rendering uses it: it is here because the entry list is what
    /// the published snapshot is built from, and a pane that may *change* an
    /// automation addresses it by id (ADR-35). Resolving it a second time from the
    /// cached automations would be the same lookup with a second chance to
    /// disagree about which row is which.
    pub id: i64,
    pub name: String,
    /// The schedule's resolved human label — `once`, `daily 09:00`, or the raw
    /// cron expression when it maps to no preset.
    pub schedule: String,
    /// The action's stable wire name (`send` / `spawn` / `exec`).
    pub action: &'static str,
    pub enabled: bool,
    /// Whole seconds until the next run, or `None` when there is none.
    pub due_in_secs: Option<u64>,
    /// Byte offsets in `name` matched by the active global-search query.
    pub match_positions: Vec<usize>,
    /// When a global search is active, rows that don't match are dimmed.
    pub dimmed: bool,
}

pub struct AutomationsPaneState<'a> {
    pub entries: &'a [AutomationPaneEntry],
    pub selected: usize,
    pub focus: FocusLevel,
    /// While global search previews an automation here, show the selected row
    /// highlighted (not dimmed) even though the pane isn't focused.
    pub preview_selected: bool,
}

/// One row as the pane's view tree describes it: the name fitted to the column,
/// its summary composed, and the three facts that pick its style.
///
/// Distinct from [`AutomationPaneEntry`] because this is the *resolved* row — the
/// name has been fitted to a width, the summary composed, and `selected` has
/// folded in the pane's focus — so building the tree from it needs no geometry and
/// no focus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AutomationRow {
    /// Display name, already fitted to the column.
    pub name: String,
    /// The composed `<schedule> · <action> · <when>` tail.
    pub summary: String,
    pub enabled: bool,
    /// Byte offsets in `name` a running search matched.
    pub match_positions: Vec<usize>,
    /// A running search filtered this row out.
    pub dimmed: bool,
    /// The user's cursor is on this row *and* it is visible as such.
    pub selected: bool,
}

/// Compose a row's summary tail from its parts.
///
/// The one rule, shared by this pane, the `Ctrl+P` list modal (through
/// `app::automation::format_automation_summary`) and — as a reproduction compared
/// against it — the bundled plugin. The three-way `when` precedence is the whole
/// content: a disabled automation says so rather than counting down to a run that
/// will not happen, an enabled one with a next run counts down, and an enabled one
/// with none shows a placeholder.
pub fn row_summary(
    schedule: &str,
    action: &str,
    enabled: bool,
    due_in_secs: Option<u64>,
) -> String {
    let when = if !enabled {
        "disabled".to_string()
    } else if let Some(secs) = due_in_secs {
        // `format_countdown` already includes the "in " prefix.
        super::format_countdown(secs)
    } else {
        "—".to_string()
    };
    format!("{schedule} · {action} · {when}")
}

pub fn render_automations_pane(
    frame: &mut Frame,
    area: Rect,
    state: &AutomationsPaneState<'_>,
) -> Vec<super::RowHitbox> {
    // The pane's own bordered block, unchanged by the port: the tree is the
    // *rows'* IR, and swapping the frame for the shared `focus_block` would be a
    // visible change to a pane this change is only supposed to re-plumb.
    let border_color = match state.focus {
        FocusLevel::Focused => Theme::border_focused(),
        _ => Theme::border_unfocused(),
    };
    let block = Block::default()
        .title(" Automations ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color));
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if inner.height == 0 || inner.width == 0 {
        return Vec::new();
    }

    let rows = resolve_rows(state, inner.width);
    let cursor = cursor_row(state);
    let tree = automations_tree(&rows, cursor, matches!(state.focus, FocusLevel::Focused));
    // A fresh frame table every paint: nothing in this pane animates, and a
    // motion the kernel is not driving draws frame 0 anyway.
    super::plugin_pane::render_tree(
        &tree,
        inner,
        &super::theme::current(),
        &FrameTable::default(),
        frame.buffer_mut(),
    );

    if rows.is_empty() {
        return Vec::new();
    }
    // One single-line hitbox per visible row, indexed in entry space. The second
    // call to the windowing helper: the paint above resolved the same window, and
    // the hitboxes need it as numbers.
    let (start, end) =
        super::file_viewer::visible_window(rows.len(), cursor.unwrap_or(0), inner.height as usize);
    super::windowed_row_hitboxes(inner, start, end)
}

/// Which row the cursor is on, clamped into the list. `None` only for an empty
/// list.
///
/// **One** clamped index, where the pre-port pane had two rules for it: it windowed
/// on the clamped index while comparing the *unclamped* one to decide which row
/// looked selected, so a stale selection left by a shortened list scrolled to the
/// last row and highlighted nothing. Only publishing a single anchor made that
/// visible, and a single anchor is what the plugin host will accept — it refuses a
/// list whose cursor is not an index into its children, so "the cursor is drawn, at
/// a row that does not exist" is not a state a pane can express.
///
/// So the appearance follows the anchor: a focused pane with a stale selection now
/// highlights its last row. See
/// `a_stale_selection_now_highlights_the_last_row_rather_than_none` for the one
/// cell-level difference this makes against the retained pre-port oracle.
fn cursor_row(state: &AutomationsPaneState<'_>) -> Option<usize> {
    (!state.entries.is_empty()).then(|| state.selected.min(state.entries.len() - 1))
}

/// Resolve how every row looks at a pane of `width`.
///
/// The one width-dependent step in the pane: the name's fitted form, with the
/// marker prefix and the whole summary tail reserved so neither is pushed off the
/// row. A plugin reproducing this pane has no width, so it draws names whole and
/// lets the renderer clip them — the one divergence
/// `tests/bundled_automations_panel.rs` enumerates.
///
/// Every row is resolved, not just the visible ones, because the window is the
/// renderer's to choose from the tree's declared cursor.
pub fn resolve_rows(state: &AutomationsPaneState<'_>, width: u16) -> Vec<AutomationRow> {
    let focused = matches!(state.focus, FocusLevel::Focused);
    // The same clamped index the tree anchors on, so the row that looks selected is
    // the row the list scrolls to. See [`cursor_row`].
    let cursor = cursor_row(state);
    let width = width as usize;
    state
        .entries
        .iter()
        .enumerate()
        .map(|(i, e)| {
            let summary = row_summary(&e.schedule, e.action, e.enabled, e.due_in_secs);
            let budget = width
                .saturating_sub(row_prefix(e.enabled).chars().count())
                .saturating_sub(row_tail(&summary).chars().count());
            AutomationRow {
                name: truncate_ellipsis(&e.name, budget),
                summary,
                enabled: e.enabled,
                match_positions: e.match_positions.clone(),
                dimmed: e.dimmed,
                // Highlight the cursor's row when the pane is focused, OR when a
                // global-search preview points here (so the moving cursor is
                // visible even though focus is in the search box). Distinct from
                // the tree's cursor index only in *whether* it is drawn — which
                // row is the same question, answered once by [`cursor_row`].
                selected: (focused || state.preview_selected) && Some(i) == cursor,
            }
        })
        .collect()
}

/// The leading marker run: an enabled automation is filled, a disabled one hollow.
fn row_prefix(enabled: bool) -> String {
    let marker = if enabled { "●" } else { "○" };
    format!(" {marker} ")
}

/// The trailing summary run, separator included.
fn row_tail(summary: &str) -> String {
    format!(" — {summary} ")
}

/// Build the pane's view tree from every row plus the cursor's index.
///
/// Carries no geometry: every row is one line whose runs take their width from
/// their content, and no row is windowed away — the list's `cursor` is what lets
/// the *renderer* scroll it to that row from the height it has, so the same tree
/// paints correctly at any size. `focused` is here because it changes *content* —
/// the empty-state line names the key that adds an automation only when the pane
/// can receive it.
///
/// `cursor` is not required to be in range: an index past the last row highlights
/// nothing and windows to the end, which is [`super::file_viewer::file_tree`]'s
/// rule too.
pub fn automations_tree(rows: &[AutomationRow], cursor: Option<usize>, focused: bool) -> ViewNode {
    if rows.is_empty() {
        let text = if focused {
            "none — Ctrl+N to add"
        } else {
            "none"
        };
        return ViewNode::list(vec![ViewNode::token(text, StyleToken::Muted)]);
    }
    ViewNode::selectable_list(rows.iter().map(row_node).collect(), cursor)
}

/// One automation row: `<marker> <name> — <summary>` with global-search matches
/// emphasised.
fn row_node(row: &AutomationRow) -> ViewNode {
    let base = row_base_style(row);
    // The marker is its own run; the name follows so highlight byte offsets
    // (which index `name`) line up.
    let mut runs = vec![ViewNode::styled(row_prefix(row.enabled), base)];

    // Emphasise matched characters on every non-dimmed row (including the
    // selected one); a dimmed row did not match, so it has no positions.
    let positions: &[usize] = if row.dimmed {
        &[]
    } else {
        &row.match_positions
    };
    for (range, matched) in super::highlight::highlight_runs(&row.name, positions) {
        let style = if matched { MATCHED_STYLE } else { base };
        runs.push(ViewNode::styled(&row.name[range], style));
    }

    runs.push(ViewNode::styled(row_tail(&row.summary), base));
    ViewNode::Line(runs)
}

/// How a matched character is drawn: accent, bold and underlined, over whatever
/// the row's base style was.
///
/// A constant rather than a function of the base, for [`super::tasks_panel`]'s
/// reason: the layering only ever resolves to this, and the one base carrying an
/// attribute of its own (dimmed) never has matches to emphasise.
const MATCHED_STYLE: TextStyle = TextStyle {
    token: Some(StyleToken::Accent),
    bold: true,
    dim: false,
    underline: true,
    selected: false,
    tint: None,
};

/// A row's resting style, in the precedence thurbox's list panes share: selected
/// wins over dimmed, which wins over the enabled/disabled colour role.
///
/// The token spellings are the tree's names for what
/// [`super::highlight::row_base_style`] resolves to — `Theme::selected_item()` is
/// accent + bold, a dimmed row is muted + dim, an enabled row is the theme's
/// secondary text and a disabled one is muted — and the differential test in this
/// module is what holds the two together.
fn row_base_style(row: &AutomationRow) -> TextStyle {
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
            token: Some(if row.enabled {
                StyleToken::Secondary
            } else {
                StyleToken::Muted
            }),
            ..TextStyle::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{backend::TestBackend, buffer::Buffer, Terminal};

    fn entry(name: &str) -> AutomationPaneEntry {
        AutomationPaneEntry {
            id: 0,
            name: name.into(),
            schedule: "daily 09:00".into(),
            action: "spawn",
            enabled: true,
            due_in_secs: Some(120),
            match_positions: vec![],
            dimmed: false,
        }
    }

    fn state<'a>(
        entries: &'a [AutomationPaneEntry],
        selected: usize,
        focus: FocusLevel,
        preview: bool,
    ) -> AutomationsPaneState<'a> {
        AutomationsPaneState {
            entries,
            selected,
            focus,
            preview_selected: preview,
        }
    }

    fn hitboxes(count: usize, selected: usize) -> Vec<super::super::RowHitbox> {
        let backend = TestBackend::new(30, 5);
        let mut terminal = Terminal::new(backend).unwrap();
        let entries: Vec<AutomationPaneEntry> =
            (0..count).map(|i| entry(&format!("a{i}"))).collect();
        let mut rows = Vec::new();
        terminal
            .draw(|f| {
                rows = render_automations_pane(
                    f,
                    Rect::new(0, 0, 30, 5),
                    &state(&entries, selected, FocusLevel::Focused, false),
                );
            })
            .unwrap();
        rows
    }

    #[test]
    fn rows_render_from_top_when_everything_fits() {
        // 5 outer rows → 3 inner; two automations fit without scrolling.
        let rows = hitboxes(2, 0);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].rect, Rect::new(1, 1, 28, 1));
        assert_eq!(rows[0].index, 0);
        assert_eq!(rows[1].index, 1);
    }

    #[test]
    fn selection_below_viewport_scrolls_into_view() {
        // 10 automations, only 3 inner rows: a selection near the bottom must
        // be windowed into view rather than clipped off-screen.
        let rows = hitboxes(10, 9);
        assert_eq!(rows.len(), 3);
        let visible: Vec<usize> = rows.iter().map(|r| r.index).collect();
        assert!(
            visible.contains(&9),
            "selected row 9 should be windowed into view, got {visible:?}"
        );
        assert!(!visible.contains(&0));
    }

    #[test]
    fn an_empty_pane_has_no_hitboxes() {
        assert!(hitboxes(0, 0).is_empty());
    }

    #[test]
    fn the_summary_precedence_is_disabled_then_countdown_then_placeholder() {
        assert_eq!(
            row_summary("daily 09:00", "spawn", false, Some(30)),
            "daily 09:00 · spawn · disabled",
            "a disabled automation says so rather than counting down"
        );
        assert_eq!(
            row_summary("daily 09:00", "spawn", true, Some(7_200)),
            "daily 09:00 · spawn · in 2h"
        );
        assert_eq!(
            row_summary("once", "exec", true, None),
            "once · exec · —",
            "enabled with no next run"
        );
    }

    // ── the pre-port oracle ─────────────────────────────────────────────────
    //
    // The span renderer this pane had before the view tree became its IR, kept so
    // the port is validated cell-for-cell rather than by eye. It is the only code
    // here that names a palette colour, which is the point: the tree names
    // *tokens*, and this is what checks the two resolve to the same cells.

    fn legacy_render(frame: &mut Frame, area: Rect, state: &AutomationsPaneState<'_>) {
        let border_color = match state.focus {
            FocusLevel::Focused => Theme::border_focused(),
            _ => Theme::border_unfocused(),
        };
        let block = Block::default()
            .title(" Automations ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(border_color));
        let inner = block.inner(area);
        frame.render_widget(block, area);

        if state.entries.is_empty() {
            let text = if matches!(state.focus, FocusLevel::Focused) {
                "none — Ctrl+N to add"
            } else {
                "none"
            };
            frame.render_widget(
                Paragraph::new(Line::from(Span::styled(
                    text,
                    Style::default().fg(Theme::text_muted()),
                ))),
                inner,
            );
            return;
        }

        let width = inner.width as usize;
        let focused = matches!(state.focus, FocusLevel::Focused);
        let selected = state.selected.min(state.entries.len() - 1);
        let (start, end) = super::super::file_viewer::visible_window(
            state.entries.len(),
            selected,
            inner.height as usize,
        );
        let lines: Vec<Line> = state.entries[start..end]
            .iter()
            .enumerate()
            .map(|(row, e)| {
                let i = start + row;
                let is_selected = (focused || state.preview_selected) && i == state.selected;
                legacy_line(e, is_selected, width)
            })
            .collect();
        frame.render_widget(Paragraph::new(lines), inner);
    }

    fn legacy_line<'a>(e: &AutomationPaneEntry, selected: bool, width: usize) -> Line<'a> {
        let summary = row_summary(&e.schedule, e.action, e.enabled, e.due_in_secs);
        let prefix = row_prefix(e.enabled);
        let tail = row_tail(&summary);
        let name_budget = width
            .saturating_sub(prefix.chars().count())
            .saturating_sub(tail.chars().count());
        let name = truncate_ellipsis(&e.name, name_budget);

        let normal = Style::default().fg(if e.enabled {
            Theme::text_secondary()
        } else {
            Theme::text_muted()
        });
        let base = super::super::highlight::row_base_style(selected, e.dimmed, normal);
        let mut spans = vec![Span::styled(prefix, base)];
        let positions: &[usize] = if e.dimmed { &[] } else { &e.match_positions };
        spans.extend(super::super::highlight::highlighted_spans_owned(
            &name, positions, base,
        ));
        spans.push(Span::styled(tail, base));
        Line::from(spans)
    }

    /// Paint one pane through a closure, so the two renderers are compared on
    /// identical buffers.
    fn paint(
        width: u16,
        height: u16,
        draw: impl FnOnce(&mut Frame, Rect, &AutomationsPaneState<'_>),
        state: &AutomationsPaneState<'_>,
    ) -> Buffer {
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        terminal
            .draw(|f| draw(f, Rect::new(0, 0, width, height), state))
            .unwrap();
        terminal.backend().buffer().clone()
    }

    /// Every row appearance the pane has, and a height that forces a scroll: the
    /// tree must paint the cells the span renderer painted — whole buffer, border
    /// included, since the port changed the rows' IR and nothing about the frame.
    #[test]
    fn the_tree_paints_the_cells_the_span_renderer_painted() {
        let long = "an automation name far wider than this column".to_string();
        let entries = vec![
            AutomationPaneEntry {
                enabled: true,
                ..entry("nightly sync")
            },
            AutomationPaneEntry {
                enabled: false,
                due_in_secs: None,
                schedule: "once".into(),
                action: "send",
                ..entry("paused one")
            },
            AutomationPaneEntry {
                match_positions: vec![0, 4],
                ..entry("host tests")
            },
            AutomationPaneEntry {
                dimmed: true,
                ..entry("filtered out")
            },
            AutomationPaneEntry {
                match_positions: vec![0, 3],
                ..entry("selected and matched")
            },
            AutomationPaneEntry {
                name: long,
                action: "exec",
                ..entry("ignored")
            },
        ];
        for (label, selected, focus, preview, height) in [
            (
                "focused, cursor on a matched row",
                4,
                FocusLevel::Focused,
                false,
                10,
            ),
            ("unfocused", 4, FocusLevel::Inactive, false, 10),
            ("previewed by a search", 2, FocusLevel::Inactive, true, 10),
            ("active (editor open)", 1, FocusLevel::Active, false, 10),
            // Three inner rows for six entries: the window has to agree too.
            ("scrolled to the last row", 5, FocusLevel::Focused, false, 5),
            // A stale selection past the end is deliberately absent: it is the one
            // case the port changes, pinned by
            // `a_stale_selection_now_highlights_the_last_row_rather_than_none`.
        ] {
            let s = state(&entries, selected, focus, preview);
            let ported = paint(
                34,
                height,
                |f, a, st| {
                    render_automations_pane(f, a, st);
                },
                &s,
            );
            let legacy = paint(34, height, legacy_render, &s);
            assert_eq!(ported, legacy, "`{label}` diverges from the span renderer");
        }
    }

    /// Both empty states, likewise cell-for-cell.
    #[test]
    fn the_empty_pane_paints_identically() {
        for focus in [FocusLevel::Focused, FocusLevel::Inactive] {
            let s = state(&[], 0, focus, false);
            let ported = paint(
                34,
                5,
                |f, a, st| {
                    render_automations_pane(f, a, st);
                },
                &s,
            );
            let legacy = paint(34, 5, legacy_render, &s);
            assert_eq!(ported, legacy, "the empty pane diverges");
        }
    }

    /// **The one intended difference from the pre-port renderer**, stated rather
    /// than trimmed out of the identity test above.
    ///
    /// The pre-port pane had two rules for one index: it clamped the index it
    /// windowed on and compared the *unclamped* one to decide which row looked
    /// selected. So a focused pane whose selection was left past the end of a
    /// shortened list scrolled to the last row and highlighted **nothing**. The
    /// ported pane has a single clamped cursor — which is what the plugin host will
    /// accept, since it refuses a list whose cursor is not an index into its
    /// children — so that row is now highlighted.
    ///
    /// A behaviour change in a port is worth exactly one test and one reason: a
    /// focused list showing no cursor at all was not a state anyone chose, and one
    /// index cannot express it.
    #[test]
    fn a_stale_selection_now_highlights_the_last_row_rather_than_none() {
        let entries = vec![entry("first"), entry("second")];
        let s = state(&entries, 99, FocusLevel::Focused, false);

        // The resolved rows: the last one is the cursor's.
        let rows = resolve_rows(&s, 34);
        assert!(!rows[0].selected);
        assert!(rows[1].selected, "the clamped cursor's row is drawn");
        assert_eq!(
            cursor_row(&s),
            Some(1),
            "and it is the row the list anchors on"
        );

        // Against the oracle: the same cells except for that row's style, so the
        // difference is the highlight and nothing else.
        let ported = paint(
            34,
            5,
            |f, a, st| {
                render_automations_pane(f, a, st);
            },
            &s,
        );
        let legacy = paint(34, 5, legacy_render, &s);
        assert_ne!(ported, legacy, "this is the case that changed");
        let symbols = |b: &Buffer| -> Vec<String> {
            (0..5)
                .flat_map(|y| (0..34).map(move |x| (x, y)))
                .map(|(x, y)| b[(x, y)].symbol().to_string())
                .collect()
        };
        assert_eq!(
            symbols(&ported),
            symbols(&legacy),
            "only the styling moved: the same glyphs in the same cells"
        );
    }

    /// The hitboxes cover exactly the rows the paint drew — one window helper,
    /// called twice, so a click cannot land on a row that is not there.
    #[test]
    fn the_hitboxes_cover_exactly_the_visible_rows() {
        let rows = hitboxes(10, 5);
        let (start, end) = super::super::file_viewer::visible_window(10, 5, 3);
        let indices: Vec<usize> = rows.iter().map(|r| r.index).collect();
        assert_eq!(indices, (start..end).collect::<Vec<_>>());
    }
}
