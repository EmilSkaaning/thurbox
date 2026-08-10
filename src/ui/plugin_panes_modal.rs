//! Picker over every declared plugin pane: which are on screen, and a toggle.
//!
//! Opened by the pane-visibility action when more than one pane is declared. Its
//! rows are plain `PluginPaneRow` data owned by `app`, never
//! `crate::plugin::PluginPane` — `ui` may not reference the plugin host, and the
//! renderer needs four fields.
//!
//! Modeled on [`task_action_picker_modal`](super::task_action_picker_modal),
//! with a checkbox per row: the answer to "is this pane on screen" has to be
//! visible *before* the toggle, since the picker is also how a user finds out
//! which panes exist.

use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::Style,
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

use super::{
    centered_fixed_height_rect, render_hint_action_footer, render_modal_frame,
    render_selector_rows, selector_line, theme::Theme, truncate_ellipsis,
};
use crate::app::modals::{PluginPaneRow, PluginPanesModal};

/// One row's label: its checkbox, the pane's title, and the plugin that owns it.
///
/// The plugin's name is shown on every row because two plugins may declare panes
/// with the same title, and a pane is addressed as `<plugin>.<pane>` everywhere
/// else (the generated visibility commands, the stored choice).
fn row_label(row: &PluginPaneRow, width: usize) -> String {
    let checkbox = if row.visible { "☑" } else { "☐" };
    let qualified = format!("{}.{}", row.plugin, row.id);
    let label = format!("{checkbox} {} · {qualified}", row.title);
    truncate_ellipsis(&label, width)
}

pub fn render_plugin_panes_modal(
    frame: &mut Frame,
    state: &PluginPanesModal,
) -> super::ModalRender {
    let height = (state.rows.len() as u16).max(2) + 6;
    let area = centered_fixed_height_rect(60, height, frame.area());
    let inner = render_modal_frame(frame, area, "Plugin panes");

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(1),
            Constraint::Length(1),
        ])
        .split(inner);

    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            "  Space shows or hides · Enter toggles and closes",
            Style::default().fg(Theme::text_muted()),
        ))),
        chunks[0],
    );

    let width = chunks[1].width.saturating_sub(4) as usize;
    let lines: Vec<Line<'_>> = state
        .rows
        .iter()
        .enumerate()
        .map(|(i, row)| selector_line(&row_label(row, width), i == state.index))
        .collect();

    let buttons = render_hint_action_footer(
        frame,
        chunks[2],
        Line::from(vec![
            Span::styled("j/k", Theme::keybind()),
            Span::styled(" navigate", Theme::keybind_desc()),
        ]),
        (
            "Toggle",
            crossterm::event::KeyCode::Char(' '),
            crossterm::event::KeyModifiers::NONE,
        ),
        "Close",
    );
    let hits = render_selector_rows(frame, chunks[1], lines, state.index);
    (hits, buttons)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(plugin: &str, id: &str, title: &str, visible: bool) -> PluginPaneRow {
        PluginPaneRow {
            plugin: plugin.to_string(),
            id: id.to_string(),
            title: title.to_string(),
            visible,
        }
    }

    #[test]
    fn plugin_pane_row_label_shows_visibility_and_address() {
        let shown = row_label(&row("info-panel", "info", "Info (plugin)", true), 60);
        assert_eq!(shown, "☑ Info (plugin) · info-panel.info");
        let hidden = row_label(&row("hello", "board", "Hello", false), 60);
        assert_eq!(hidden, "☐ Hello · hello.board");
    }

    #[test]
    fn plugin_pane_row_label_is_clipped_to_the_pane_width() {
        // A narrow modal elides rather than overflowing the frame.
        let label = row_label(&row("info-panel", "info", "Info (plugin)", true), 12);
        assert_eq!(label.chars().count(), 12);
        assert!(label.ends_with('…'));
    }
}
