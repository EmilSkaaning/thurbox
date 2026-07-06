use ratatui::{
    layout::{Constraint, Direction, Layout},
    text::{Line, Span},
    Frame,
};

use super::theme::Theme;
use super::{
    centered_fixed_height_rect, render_modal_frame, render_selector_footer, render_selector_rows,
    selector_line,
};

/// One selectable agent row: display name + the CLI command it launches.
#[derive(Debug, Clone)]
pub struct AgentChoice {
    pub name: String,
    pub command: String,
    /// Whether the launch `command` was found on PATH when the picker opened.
    /// Un-installed agents render dimmed + marked but stay selectable (advisory
    /// only — a shell rc could resolve the binary the probe can't see). Always
    /// `true` for remote-host sessions, where the local PATH says nothing.
    pub available: bool,
}

impl Default for AgentChoice {
    fn default() -> Self {
        Self {
            name: String::new(),
            command: String::new(),
            // A choice with no availability signal is treated as present, so
            // the picker never dims by accident.
            available: true,
        }
    }
}

/// Pick the initial selection: keep the registry default when it's available,
/// else the first available choice, else fall back to the default index (so an
/// all-un-installed list still lands somewhere sensible). Registry order is
/// never changed — only where the cursor starts.
pub fn initial_selection(choices: &[AgentChoice], default_index: usize) -> usize {
    if choices.get(default_index).is_some_and(|c| c.available) {
        return default_index;
    }
    choices
        .iter()
        .position(|c| c.available)
        .unwrap_or(default_index)
}

#[derive(Debug, Clone, Default)]
pub struct AgentPickerState {
    pub choices: Vec<AgentChoice>,
    pub selected_index: usize,
}

pub fn render_agent_picker_modal(
    frame: &mut Frame,
    state: &AgentPickerState,
) -> super::ModalRender {
    let height = (state.choices.len() as u16) + 3;
    let area = centered_fixed_height_rect(50, height, frame.area());

    let inner = render_modal_frame(frame, area, "Coding Agent");

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(inner);

    let lines: Vec<Line<'_>> = state
        .choices
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let label = if c.name == c.command {
                c.name.clone()
            } else {
                format!("{}  ({})", c.name, c.command)
            };
            let selected = i == state.selected_index;
            if c.available {
                selector_line(&label, selected)
            } else {
                unavailable_line(&label, selected)
            }
        })
        .collect();

    let buttons = render_selector_footer(frame, chunks[1]);
    let hits = render_selector_rows(frame, chunks[0], lines, state.selected_index);
    (hits, buttons)
}

/// Render an un-installed agent row: the standard selection prefix (so the
/// cursor stays visible) plus a muted label and a trailing ` (not installed)`
/// marker. Kept selectable on purpose — dimming is advisory.
fn unavailable_line<'a>(label: &str, selected: bool) -> Line<'a> {
    let prefix = if selected { "▸ " } else { "  " };
    let style = Theme::normal_item().fg(Theme::text_muted());
    Line::from(Span::styled(
        format!("{prefix}{label} (not installed)"),
        style,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn choice(name: &str, available: bool) -> AgentChoice {
        AgentChoice {
            name: name.into(),
            command: name.into(),
            available,
        }
    }

    #[test]
    fn initial_selection_keeps_available_default() {
        let choices = vec![choice("claude", true), choice("codex", true)];
        assert_eq!(initial_selection(&choices, 0), 0);
        assert_eq!(initial_selection(&choices, 1), 1);
    }

    #[test]
    fn initial_selection_moves_off_unavailable_default() {
        // Default is claude (index 0) but it's not installed; codex is.
        let choices = vec![choice("claude", false), choice("codex", true)];
        assert_eq!(initial_selection(&choices, 0), 1);
    }

    #[test]
    fn initial_selection_falls_back_when_none_available() {
        let choices = vec![choice("claude", false), choice("codex", false)];
        assert_eq!(initial_selection(&choices, 0), 0);
        assert_eq!(initial_selection(&choices, 1), 1);
    }

    #[test]
    fn initial_selection_empty_is_default_index() {
        assert_eq!(initial_selection(&[], 3), 3);
    }

    /// Concatenate a line's span contents for assertions.
    fn line_text(line: &Line<'_>) -> String {
        line.spans.iter().map(|s| s.content.as_ref()).collect()
    }

    #[test]
    fn unavailable_line_marks_and_prefixes() {
        // Selected row keeps the ▸ cursor and carries the advisory marker.
        let selected = line_text(&unavailable_line("codex  (codex)", true));
        assert!(
            selected.starts_with("▸ "),
            "selected keeps cursor: {selected}"
        );
        assert!(
            selected.contains("codex  (codex)") && selected.ends_with("(not installed)"),
            "label + marker present: {selected}"
        );
        // Unselected row uses the blank prefix, still marked.
        let unselected = line_text(&unavailable_line("vibe", false));
        assert!(unselected.starts_with("  "));
        assert!(unselected.ends_with("(not installed)"));
    }
}
