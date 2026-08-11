use ratatui::{
    style::Style,
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

use super::render_list_modal_frame;
use super::theme::Theme;

/// View-only entry for the automations list modal.
pub struct AutomationsListEntry {
    pub name: String,
    pub summary: String,
    pub enabled: bool,
}

/// Compose an automation's summary tail from its parts:
/// `<schedule> · <action> · <when>`.
///
/// The three-way `when` precedence is the whole content: a disabled automation says so
/// rather than counting down to a run that will not happen, an enabled one with a next
/// run counts down, and an enabled one with none shows a placeholder.
///
/// **Why it lives here.** It was `ui::automations_panel`'s, shared with this modal so
/// the two native surfaces could not disagree; the handover deleted that pane
/// (ADR-56), and this modal is the surface that still composes it. Not `ui/mod.rs`,
/// which is the layer's *shared* vocabulary (where [`super::format_countdown`] belongs,
/// because three surfaces format a countdown) — this is one surface's row format. Not
/// `app::automation`, beside the `format_automation_summary` that calls it: this is
/// display-text composition, and putting it in the coordinator would split one rule
/// across two layers with its own helper left behind in `ui`.
///
/// The bundled automations plugin composes the same string from the same published
/// parts, and `tests/bundled_automations_panel.rs` compares the two against **this**
/// function rather than against a third copy written in a test. That edge outlives the
/// handover precisely because this function does.
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

pub struct AutomationsListState<'a> {
    pub entries: &'a [AutomationsListEntry],
    pub selected_index: usize,
}

pub fn render_automations_list_modal(
    frame: &mut Frame,
    state: &AutomationsListState<'_>,
) -> super::ModalRender {
    let empty_footer = Line::from(vec![
        Span::styled("n", Theme::keybind()),
        Span::styled(" new  ", Theme::keybind_desc()),
        Span::styled("Esc", Theme::keybind()),
        Span::styled(" close", Theme::keybind_desc()),
    ]);

    let Some([list_area, footer_area]) = render_list_modal_frame(
        frame,
        70,
        "Automations",
        state.entries.len(),
        Some("No automations — press n to create one"),
        Some(empty_footer),
    ) else {
        return ((Vec::new(), None), Vec::new());
    };

    let inner_width = list_area.width as usize;

    let lines: Vec<Line<'_>> = state
        .entries
        .iter()
        .enumerate()
        .map(|(i, entry)| {
            let selected = i == state.selected_index;
            let marker = if entry.enabled { "●" } else { "○" };
            let text = truncate(
                &format!(" {marker} {} — {} ", entry.name, entry.summary),
                inner_width,
            );
            if selected {
                Line::from(Span::styled(text, Theme::selected_item()))
            } else {
                let color = if entry.enabled {
                    Theme::text_secondary()
                } else {
                    Theme::text_muted()
                };
                Line::from(Span::styled(text, Style::default().fg(color)))
            }
        })
        .collect();

    let hits = super::render_selector_rows(frame, list_area, lines, state.selected_index);

    let help = Line::from(vec![
        Span::styled("n", Theme::keybind()),
        Span::styled(" new  ", Theme::keybind_desc()),
        Span::styled("Spc", Theme::keybind()),
        Span::styled(" toggle  ", Theme::keybind_desc()),
        Span::styled("r", Theme::keybind()),
        Span::styled(" run  ", Theme::keybind_desc()),
        Span::styled("d", Theme::keybind()),
        Span::styled(" delete", Theme::keybind_desc()),
    ]);
    frame.render_widget(Paragraph::new(help), footer_area);
    let buttons = super::render_action_footer(
        frame,
        footer_area,
        (
            "Edit",
            crossterm::event::KeyCode::Enter,
            crossterm::event::KeyModifiers::NONE,
        ),
        "Close",
    );
    (hits, buttons)
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() > max {
        let t: String = s.chars().take(max.saturating_sub(3)).collect();
        format!("{t}...")
    } else {
        s.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncate_leaves_short_strings_untouched() {
        assert_eq!(truncate("", 10), "");
        assert_eq!(truncate("hello", 10), "hello");
    }

    #[test]
    fn truncate_keeps_string_at_exact_width() {
        assert_eq!(truncate("hello", 5), "hello");
    }

    #[test]
    fn truncate_adds_ellipsis_when_over_width() {
        let out = truncate("abcdefg", 6);
        assert_eq!(out, "abc...");
        assert_eq!(out.chars().count(), 6);
    }

    #[test]
    fn truncate_counts_chars_not_bytes() {
        // Multi-byte chars must not be split mid-byte.
        let out = truncate("héllo wörld", 6);
        assert_eq!(out, "hél...");
        assert_eq!(out.chars().count(), 6);
    }

    /// The summary's three-way `when` precedence, which is the whole content of the
    /// composition — moved here with the function (ADR-56).
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
}
