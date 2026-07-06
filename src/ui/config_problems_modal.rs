//! Renderer for the read-only "Config problems" modal
//! (`crate::app::modals::ConfigProblemsModal`), opened from the persistent
//! `Config ⚠` footer badge. Lists each config file that failed validation and
//! its problem messages, with `[ Open Settings ]` / `[ Close ]` buttons.

use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

use super::theme::Theme;
use super::{render_modal_frame, ModalButtons};

const MODAL_WIDTH_PCT: u16 = 70;

/// Borrowed view of a `ConfigProblemsModal` for rendering.
pub struct ConfigProblemsState<'a> {
    pub files: &'a [crate::session::ConfigFile],
}

/// Build the body: a lead line, then per problem file its `✗ <label>` heading
/// (with resolved path) and one indented line per problem message.
fn body_lines(files: &[crate::session::ConfigFile]) -> Vec<Line<'static>> {
    let mut lines: Vec<Line> = vec![Line::from(Span::styled(
        "These config files failed validation and were ignored or partially applied:",
        Style::default().fg(Theme::text_secondary()),
    ))];
    for f in files {
        lines.push(Line::from(""));
        lines.push(Line::from(vec![
            Span::styled(
                format!("✗ {}", f.label),
                Style::default()
                    .fg(Theme::status_error())
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("  {}", f.path_display()),
                Style::default().fg(Theme::text_muted()),
            ),
        ]));
        for p in &f.problems {
            lines.push(Line::from(Span::styled(
                format!("    - {p}"),
                Style::default().fg(Theme::text_secondary()),
            )));
        }
    }
    lines
}

/// Render the modal and return its `[ Open Settings ]` (Enter) / `[ Close ]`
/// (Esc) buttons paired with the keys they replay through the modal handler.
pub fn render_config_problems_modal(
    frame: &mut Frame,
    state: &ConfigProblemsState<'_>,
) -> ModalButtons {
    use crossterm::event::{KeyCode, KeyModifiers};

    let body = body_lines(state.files);
    let height = body.len() as u16 + 2 + 2; // body + blank spacer + footer + border
    let area = super::centered_fixed_height_rect(MODAL_WIDTH_PCT, height, frame.area());
    let inner = render_modal_frame(frame, area, "Config problems");

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(body.len() as u16),
            Constraint::Length(1), // blank spacer
            Constraint::Length(1), // footer
        ])
        .split(inner);

    frame.render_widget(Paragraph::new(body), chunks[0]);

    let hits = super::render_button_bar(
        frame,
        chunks[2],
        &[
            super::ButtonSpec::primary("Open Settings"),
            super::ButtonSpec::secondary("Close"),
        ],
        true,
    );
    super::modal_button_keys(
        hits,
        &[
            (KeyCode::Enter, KeyModifiers::NONE),
            (KeyCode::Esc, KeyModifiers::NONE),
        ],
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn problem(label: &'static str, problems: Vec<String>) -> crate::session::ConfigFile {
        crate::session::ConfigFile {
            label,
            path: Some(PathBuf::from("/home/me/.config/thurbox").join(label)),
            exists: true,
            valid: false,
            validated: true,
            problems,
        }
    }

    fn text(line: &Line<'_>) -> String {
        line.spans.iter().map(|s| s.content.as_ref()).collect()
    }

    #[test]
    fn body_lists_each_file_and_its_problems() {
        let files = vec![
            problem("hosts.toml", vec!["unknown field `destinaton`".into()]),
            problem(
                "keybindings.json",
                vec!["ctrl+a bound to two actions".into()],
            ),
        ];
        let out: String = body_lines(&files)
            .iter()
            .map(text)
            .collect::<Vec<_>>()
            .join("\n");
        assert!(out.contains("✗ hosts.toml"), "{out}");
        assert!(out.contains("unknown field `destinaton`"), "{out}");
        assert!(out.contains("✗ keybindings.json"), "{out}");
        assert!(out.contains("ctrl+a bound to two actions"), "{out}");
    }
}
