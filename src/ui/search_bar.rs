//! The file viewer's search bar — three rows of **kernel chrome**.
//!
//! Split out of `src/ui/file_viewer.rs` when that pane was handed over to its bundled
//! plugin (ADR-58). The pane's tree is the plugin's; this bar is not, and could not be:
//! the query, the caret and the match counter are kernel state that no capability
//! publishes, and a pane redrawing them would be a second renderer for one fact — the
//! argument ADR-53 made for the tasks pane's hint row, at three rows instead of one.
//!
//! So the kernel draws it, in the rows the native pane drew it in: the seat subtracts
//! the band before the pane's frame, and `App::pane_chrome` decides when. Nothing here
//! knows about a plugin — it takes a `SearchBar` and paints.

use ratatui::{
    layout::Rect,
    style::Style,
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use super::theme::Theme;

/// What the bar shows, as data.
///
/// A snapshot the caller resolves from `App::file_viewer`, rather than a borrow of the
/// state: seat chrome is described to the painter as a value so that what a seat may
/// draw stays enumerable (`App::PaneChrome`), and a painter that could reach the whole
/// pane model would make that enumeration meaningless.
pub(crate) struct SearchBar {
    /// The query as typed. Scrolled to its end when it outgrows the bar.
    pub query: String,
    /// Whether the sub-mode is collecting keys — the caret is drawn only then, and the
    /// bar takes the search colour rather than the muted one.
    pub is_active: bool,
    /// Caret position, in chars from the start of the query.
    pub cursor: usize,
    /// 1-based position of the current match, for the ` Search (2/5) ` title.
    pub current: usize,
    /// Total matches; zero renders ` Search (no matches) `.
    pub total: usize,
}

/// Draw the bar into `area`, which must be the three rows the seat reserved.
pub(crate) fn render_search_bar(frame: &mut Frame, area: Rect, search: &SearchBar) {
    let SearchBar {
        query,
        is_active,
        cursor,
        current,
        total,
    } = search;
    let (is_active, cursor, current, total) = (*is_active, *cursor, *current, *total);
    let query = query.as_str();

    let style = if is_active {
        Style::default().fg(Theme::search_bar())
    } else {
        Style::default().fg(Theme::text_muted())
    };

    let block = Block::default()
        .title(Line::from(Span::styled(
            search_title(query, current, total),
            style,
        )))
        .borders(Borders::ALL)
        .border_style(style);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let max_width = inner.width as usize;
    if max_width == 0 || inner.height == 0 {
        return;
    }

    let prefix = "/ ";
    let display_query = truncate_left(query, max_width.saturating_sub(prefix.len()));
    let (before, after) = split_at_cursor(display_query, cursor);

    let mut spans = vec![Span::styled(prefix, style), Span::styled(before, style)];
    append_cursor_spans(&mut spans, after, is_active, style);

    frame.render_widget(Paragraph::new(Line::from(spans)), inner);
}

fn search_title(query: &str, current: usize, total: usize) -> String {
    if query.is_empty() {
        " Search ".to_string()
    } else if total == 0 {
        " Search (no matches) ".to_string()
    } else {
        format!(" Search ({current}/{total}) ")
    }
}

/// Keep the trailing `budget` characters of `query` so the cursor end stays
/// visible. Counts and slices on `char` boundaries — never raw byte indices —
/// so a multi-byte (non-ASCII) query never panics on a mid-codepoint slice.
fn truncate_left(query: &str, budget: usize) -> &str {
    let char_count = query.chars().count();
    if char_count <= budget {
        return query;
    }
    let start = query
        .char_indices()
        .nth(char_count - budget)
        .map_or(query.len(), |(i, _)| i);
    &query[start..]
}

fn split_at_cursor(text: &str, cursor: usize) -> (&str, &str) {
    if cursor > text.chars().count() {
        return (text, "");
    }
    let byte_pos = text
        .char_indices()
        .nth(cursor)
        .map(|(i, _)| i)
        .unwrap_or(text.len());
    (&text[..byte_pos], &text[byte_pos..])
}

fn append_cursor_spans<'a>(
    spans: &mut Vec<Span<'a>>,
    after: &'a str,
    is_active: bool,
    style: Style,
) {
    if !is_active {
        spans.push(Span::styled(after, style));
        return;
    }
    let first_len = after.chars().next().map_or(0, |c| c.len_utf8());
    let cursor_char = if first_len == 0 {
        " "
    } else {
        &after[..first_len]
    };
    spans.push(Span::styled(cursor_char, Theme::cursor()));
    let rest = &after[first_len..];
    if !rest.is_empty() {
        spans.push(Span::styled(rest, style));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncate_left_keeps_trailing_chars_within_budget() {
        assert_eq!(truncate_left("hello", 10), "hello");
        assert_eq!(truncate_left("hello", 5), "hello");
        assert_eq!(truncate_left("hello", 3), "llo");
        assert_eq!(truncate_left("hello", 0), "");
    }

    #[test]
    fn truncate_left_handles_multibyte_query_without_panicking() {
        // A multi-byte (non-ASCII) query whose byte length exceeds the budget
        // used to slice on a raw byte index and panic mid-codepoint. Slicing on
        // char boundaries must keep the trailing chars and stay valid UTF-8.
        // "héllo wörld 你好" — mix of 1-, 2-, and 3-byte code points.
        let q = "héllo wörld 你好";
        assert!(q.len() > q.chars().count(), "query must be multi-byte");

        // Budget smaller than the char count would put a naive byte slice in
        // the middle of a multi-byte char. We keep the trailing `budget` chars.
        let out = truncate_left(q, 4);
        assert_eq!(out.chars().count(), 4);
        assert_eq!(out, "d 你好");

        // A budget that lands exactly on a multi-byte boundary, and one of 0.
        assert_eq!(truncate_left(q, 2), "你好");
        assert_eq!(truncate_left(q, 0), "");
        // Budget >= char count returns the whole string unchanged.
        assert_eq!(truncate_left(q, q.chars().count()), q);
        assert_eq!(truncate_left(q, 999), q);
    }
}
