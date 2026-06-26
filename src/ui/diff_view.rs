//! Central-pane unified-diff renderer for the file-viewer review pane.
//!
//! Pure rendering: receives a [`FileDiff`] (already parsed + classified by the
//! `git` layer into [`DiffLineKind`]s) and colors each line. It never parses a
//! diff or touches `git` — the `ui ✗ git` architecture rule holds.

use ratatui::{
    layout::{Alignment, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

use super::scrollbar;
use super::{focus_block, theme::Theme, FocusLevel};
use crate::session::{DiffLine, DiffLineKind, FileDiff};

/// Render `diff` into `area` as a scrollable, color-coded unified diff.
/// `scroll` is the top line offset (clamped here so an over-scroll past the end
/// snaps to the last page).
pub fn render_diff(frame: &mut Frame, area: Rect, diff: &FileDiff, scroll: u16, focus: FocusLevel) {
    let title = diff_title(diff);
    let block = focus_block(&title, focus);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if inner.height == 0 || inner.width == 0 {
        return;
    }

    if diff.binary {
        let p = Paragraph::new(Line::from(Span::styled(
            "binary file — no textual diff",
            Style::default().fg(Theme::text_muted()),
        )))
        .alignment(Alignment::Center);
        frame.render_widget(p, inner);
        return;
    }

    if diff.lines.is_empty() {
        let p = Paragraph::new(Line::from(Span::styled(
            "no changes",
            Style::default().fg(Theme::text_muted()),
        )))
        .alignment(Alignment::Center);
        frame.render_widget(p, inner);
        return;
    }

    let total = diff.lines.len();
    let height = inner.height as usize;
    let max_scroll = total.saturating_sub(height);
    let start = (scroll as usize).min(max_scroll);
    let end = (start + height).min(total);

    let (rows_area, track) = scrollbar::reserve_track(inner, total, height);
    let lines: Vec<Line> = diff.lines[start..end].iter().map(style_line).collect();
    frame.render_widget(Paragraph::new(lines), rows_area);

    if let Some(track) = track {
        scrollbar::render_into(frame, track, total, height, start);
    }
}

/// Block title: `<path> · Δ <base>`.
fn diff_title(diff: &FileDiff) -> String {
    let name = diff
        .path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| diff.path.to_string_lossy().into_owned());
    let base = diff
        .base_ref
        .strip_prefix("refs/remotes/")
        .or_else(|| diff.base_ref.strip_prefix("refs/heads/"))
        .unwrap_or(&diff.base_ref);
    format!(" {name} · Δ {base} ")
}

/// Color one diff line by its kind: added=green, removed=red, hunk=accent
/// (bold), context=normal, metadata=muted.
fn style_line(line: &DiffLine) -> Line<'static> {
    let (color, bold) = match line.kind {
        DiffLineKind::Added => (Theme::status_idle(), false),
        DiffLineKind::Removed => (Theme::status_blocked(), false),
        DiffLineKind::Hunk => (Theme::accent(), true),
        DiffLineKind::Context => (Theme::text_primary(), false),
        DiffLineKind::Meta => (Theme::text_muted(), false),
    };
    let mut style = Style::default().fg(color);
    if bold {
        style = style.add_modifier(Modifier::BOLD);
    }
    Line::from(Span::styled(line.text.clone(), style))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(kind: DiffLineKind, text: &str) -> DiffLine {
        DiffLine {
            kind,
            text: text.into(),
        }
    }

    #[test]
    fn title_shortens_base_and_uses_basename() {
        let fd = FileDiff {
            path: "src/app/mod.rs".into(),
            base_ref: "refs/remotes/origin/main".into(),
            lines: vec![],
            binary: false,
        };
        assert_eq!(diff_title(&fd), " mod.rs · Δ origin/main ");
    }

    #[test]
    fn style_line_colors_by_kind() {
        // Added is green (status_idle), removed is red (status_blocked).
        let added = style_line(&line(DiffLineKind::Added, "+x"));
        let removed = style_line(&line(DiffLineKind::Removed, "-y"));
        assert_eq!(added.spans[0].style.fg, Some(Theme::status_idle()));
        assert_eq!(removed.spans[0].style.fg, Some(Theme::status_blocked()));
        // Hunk header is bold accent.
        let hunk = style_line(&line(DiffLineKind::Hunk, "@@ -1 +1 @@"));
        assert_eq!(hunk.spans[0].style.fg, Some(Theme::accent()));
        assert!(hunk.spans[0].style.add_modifier.contains(Modifier::BOLD));
    }
}
