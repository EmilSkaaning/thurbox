use ratatui::{
    style::{Modifier, Style},
    text::{Line, Span},
    Frame,
};

use super::theme::Theme;

const MODAL_WIDTH_PCT: u16 = 60;

pub struct ConfirmSpawnState<'a> {
    /// The agent `command` the pre-flight couldn't find.
    pub command: &'a str,
    /// Whether the missing binary was probed on a remote host (changes the hint).
    pub remote: bool,
}

/// Render the "spawn anyway?" confirmation shown when the agent's `command`
/// isn't resolvable on PATH. The check is advisory — a `command` that's a shell
/// function/alias, or resolved via an rc file the walk can't see, is a valid
/// spawn the user knows about — so this offers a proceed-anyway escape hatch
/// rather than hard-blocking.
pub fn render_confirm_spawn_modal(
    frame: &mut Frame,
    state: &ConfirmSpawnState<'_>,
) -> super::ModalButtons {
    let hint = if state.remote {
        "It isn't on the host's PATH. If it resolves via a login rc the probe"
    } else {
        "It isn't on your PATH. If it's a shell function or resolves via an rc"
    };
    let body: Vec<Line> = vec![
        Line::from(vec![
            Span::styled(
                format!("'{}'", state.command),
                Style::default().add_modifier(Modifier::BOLD),
            ),
            Span::raw(" wasn't found — spawn anyway?"),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            hint,
            Style::default().fg(Theme::text_secondary()),
        )),
        Line::from(Span::styled(
            "can't see, it may still launch; otherwise the pane will die on start.",
            Style::default().fg(Theme::text_secondary()),
        )),
    ];

    super::render_confirm_modal(
        frame,
        MODAL_WIDTH_PCT,
        "Agent not found",
        false,
        body,
        (
            "Spawn anyway",
            crossterm::event::KeyCode::Enter,
            crossterm::event::KeyModifiers::NONE,
        ),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{backend::TestBackend, Terminal};

    fn rendered_text(command: &str, remote: bool) -> String {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| {
                render_confirm_spawn_modal(frame, &ConfirmSpawnState { command, remote });
            })
            .unwrap();
        terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|c| c.symbol())
            .collect()
    }

    #[test]
    fn shows_command_and_footer() {
        let out = rendered_text("agy", false);
        assert!(out.contains("agy"), "{out}");
        assert!(out.contains("Spawn anyway"), "{out}");
        assert!(out.contains("Cancel"), "{out}");
    }

    #[test]
    fn remote_hint_differs() {
        assert!(rendered_text("claude", true).contains("host's PATH"));
        assert!(rendered_text("claude", false).contains("your PATH"));
    }
}
