//! Rendering a plugin's [`ViewNode`] tree into a pane.
//!
//! `ui` renders the view tree without ever seeing `crate::plugin` — the tree
//! is pure data in `session`, so the renderer has no path back to a VM and
//! cannot call plugin code even by accident. That is the whole point of where
//! the types live.
//!
//! Style tokens resolve against the *active* palette here, at paint time, so a
//! plugin follows a theme switch without knowing one happened.

use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget};
use unicode_width::UnicodeWidthStr;

use crate::session::motion::FrameTable;
use crate::session::theme_config::ThemePalette;
use crate::session::view_tree::{StyleToken, TextStyle, ViewNode};

/// Resolve a style token against the active palette.
///
/// Tokens are roles, not colours: the same `accent` reads correctly on all 36
/// palettes including the eight light ones, which is exactly what a plugin
/// naming an RGB value could not do.
pub fn token_color(token: StyleToken, palette: &ThemePalette) -> Color {
    match token {
        StyleToken::Accent => palette.accent,
        StyleToken::Muted => palette.text_muted,
        StyleToken::Danger => palette.danger,
        StyleToken::Success => palette.status_idle,
        StyleToken::Warning => palette.status_working,
    }
}

/// Turn a node's text style into a ratatui style.
fn text_style(style: TextStyle, palette: &ThemePalette) -> Style {
    let color = style
        .token
        .map(|t| token_color(t, palette))
        .unwrap_or(palette.text_primary);
    let mut s = Style::default().fg(color);
    if style.bold {
        s = s.add_modifier(Modifier::BOLD);
    }
    s
}

/// How many terminal rows a node needs.
///
/// Text is one line and does not wrap — a plugin splits lines by returning
/// separate nodes — so height is independent of width. When wrapping arrives
/// this grows a width parameter; until then taking one would be a lie.
///
/// A motion takes the **tallest** of its frames rather than the height of the
/// frame currently showing: a height that changed per frame would shove every
/// sibling up and down as the animation ran, which is a worse artifact than
/// the blank row a short frame leaves.
fn height_of(node: &ViewNode) -> u16 {
    match node {
        // A line is one row whatever it holds: it clips rather than wraps, so
        // its height does not depend on the width it is given.
        ViewNode::Text { .. } | ViewNode::Divider | ViewNode::Line(_) => 1,
        ViewNode::Spacer { lines } => *lines,
        ViewNode::Row(children) => children.iter().map(height_of).max().unwrap_or(0),
        ViewNode::Column(children) | ViewNode::List(children) => {
            children.iter().map(height_of).sum::<u16>()
        }
        ViewNode::Motion { motion, .. } => motion.frames().iter().map(height_of).max().unwrap_or(0),
    }
}

/// Terminal columns a node occupies inside a [`ViewNode::Line`].
///
/// Measured in **display** width, not characters: a CJK glyph or a wide emoji
/// takes two cells, and counting characters would leave every run after it one
/// column adrift.
///
/// A motion reports its **widest** frame rather than the frame showing. A
/// per-frame width would move every run to its right whenever the frame
/// changed, which reads as the pane redrawing wrongly rather than as an
/// animation — the same reason [`height_of`] takes the tallest frame.
fn inline_width(node: &ViewNode) -> usize {
    match node {
        ViewNode::Text { content, .. } => UnicodeWidthStr::width(content.as_str()),
        ViewNode::Line(runs) => runs.iter().map(inline_width).sum(),
        ViewNode::Motion { motion, .. } => {
            motion.frames().iter().map(inline_width).max().unwrap_or(0)
        }
        // Refused at conversion, so unreachable from a plugin; zero is the only
        // honest answer for a node whose width comes from its area.
        ViewNode::Row(_)
        | ViewNode::Column(_)
        | ViewNode::List(_)
        | ViewNode::Divider
        | ViewNode::Spacer { .. } => 0,
    }
}

/// Flatten a line's runs into spans, in order, padding each to the width it
/// reserved.
///
/// Padding only ever applies to a motion (a text run occupies exactly its own
/// width), and it goes on the right so the *start* of an animation stays fixed
/// — that is the column the eye tracks.
fn inline_spans<'a>(
    runs: &'a [ViewNode],
    palette: &ThemePalette,
    frames: &FrameTable,
    out: &mut Vec<Span<'a>>,
) {
    for run in runs {
        match run {
            ViewNode::Text { content, style } => {
                out.push(Span::styled(content.clone(), text_style(*style, palette)));
            }
            ViewNode::Line(nested) => inline_spans(nested, palette, frames, out),
            ViewNode::Motion { key, motion, .. } => {
                let reserved = inline_width(run);
                let index = frames
                    .frame(key)
                    .min(motion.frames().len().saturating_sub(1));
                let before = out.len();
                if let Some(frame) = motion.frames().get(index) {
                    inline_spans(std::slice::from_ref(frame), palette, frames, out);
                }
                let drawn: usize = out[before..].iter().map(|s| s.content.width()).sum();
                if let Some(pad) = reserved.checked_sub(drawn).filter(|p| *p > 0) {
                    out.push(Span::raw(" ".repeat(pad)));
                }
            }
            // Refused at conversion; drawing nothing is the safe residual.
            ViewNode::Row(_)
            | ViewNode::Column(_)
            | ViewNode::List(_)
            | ViewNode::Divider
            | ViewNode::Spacer { .. } => {}
        }
    }
}

/// Draw a view tree into `area`.
///
/// `frames` says which frame each animated node is showing. It is plain data
/// the caller resolved from the kernel clock — the renderer has no clock, no
/// state, and no way to reach a plugin, so painting stays a pure function of
/// (tree, frame table, palette).
pub fn render_tree(
    node: &ViewNode,
    area: Rect,
    palette: &ThemePalette,
    frames: &FrameTable,
    buf: &mut ratatui::buffer::Buffer,
) {
    if area.width == 0 || area.height == 0 {
        return;
    }

    match node {
        ViewNode::Text { content, style } => {
            let line = Line::from(Span::styled(content.clone(), text_style(*style, palette)));
            Paragraph::new(line).render(area, buf);
        }
        ViewNode::Divider => {
            let rule = "─".repeat(area.width as usize);
            Paragraph::new(Line::from(Span::styled(
                rule,
                Style::default().fg(palette.border_unfocused),
            )))
            .render(area, buf);
        }
        // Nothing to draw; the space it occupies is the point.
        ViewNode::Spacer { .. } => {}
        ViewNode::Line(runs) => {
            // One `Paragraph` of consecutive spans, so overflow is clipped by
            // the terminal at the pane edge exactly as an over-long text node
            // is — there is no wrap, so nothing below it moves.
            let mut spans = Vec::new();
            inline_spans(runs, palette, frames, &mut spans);
            Paragraph::new(Line::from(spans)).render(area, buf);
        }
        ViewNode::Row(children) => {
            if children.is_empty() {
                return;
            }
            // Equal shares: a plugin cannot specify widths, so the kernel does
            // not have to arbitrate between competing requests.
            let constraints: Vec<Constraint> = children
                .iter()
                .map(|_| Constraint::Ratio(1, children.len() as u32))
                .collect();
            let cells = Layout::default()
                .direction(Direction::Horizontal)
                .constraints(constraints)
                .split(area);
            for (child, cell) in children.iter().zip(cells.iter()) {
                render_tree(child, *cell, palette, frames, buf);
            }
        }
        ViewNode::Column(children) | ViewNode::List(children) => {
            let mut y = area.y;
            let bottom = area.y.saturating_add(area.height);
            for child in children {
                if y >= bottom {
                    break;
                }
                let want = height_of(child);
                let height = want.min(bottom - y);
                if height == 0 {
                    continue;
                }
                render_tree(
                    child,
                    Rect {
                        x: area.x,
                        y,
                        width: area.width,
                        height,
                    },
                    palette,
                    frames,
                    buf,
                );
                y = y.saturating_add(height);
            }
        }
        ViewNode::Motion { key, motion, .. } => {
            // Frame 0 is the answer for a motion the kernel is not animating
            // (reduced motion, a frozen lease, a paint that raced a push), so
            // it has to be a correct rendering on its own.
            let index = frames
                .frame(key)
                .min(motion.frames().len().saturating_sub(1));
            if let Some(frame) = motion.frames().get(index) {
                render_tree(frame, area, palette, frames, buf);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::theme_config::ThemePreset;
    use ratatui::buffer::Buffer;

    fn palette() -> ThemePalette {
        ThemePreset::Default.palette()
    }

    fn area(w: u16, h: u16) -> Rect {
        Rect {
            x: 0,
            y: 0,
            width: w,
            height: h,
        }
    }

    /// Render a tree and return the buffer's lines as plain strings.
    fn draw(node: &ViewNode, w: u16, h: u16) -> Vec<String> {
        let rect = area(w, h);
        let mut buf = Buffer::empty(rect);
        render_tree(node, rect, &palette(), &FrameTable::default(), &mut buf);
        (0..h)
            .map(|y| {
                (0..w)
                    .map(|x| buf[(x, y)].symbol().to_string())
                    .collect::<String>()
                    .trim_end()
                    .to_string()
            })
            .collect()
    }

    #[test]
    fn text_renders_its_content() {
        assert_eq!(draw(&ViewNode::text("hello"), 10, 1), vec!["hello"]);
    }

    #[test]
    fn a_list_stacks_its_children() {
        let tree = ViewNode::List(vec![
            ViewNode::text("one"),
            ViewNode::text("two"),
            ViewNode::text("three"),
        ]);
        assert_eq!(draw(&tree, 10, 3), vec!["one", "two", "three"]);
    }

    #[test]
    fn a_divider_fills_the_width() {
        assert_eq!(draw(&ViewNode::Divider, 5, 1), vec!["─────"]);
    }

    #[test]
    fn a_spacer_leaves_blank_lines() {
        let tree = ViewNode::List(vec![
            ViewNode::text("above"),
            ViewNode::Spacer { lines: 2 },
            ViewNode::text("below"),
        ]);
        assert_eq!(draw(&tree, 10, 4), vec!["above", "", "", "below"]);
    }

    #[test]
    fn a_row_splits_the_width() {
        let tree = ViewNode::Row(vec![ViewNode::text("ab"), ViewNode::text("cd")]);
        // Two equal halves of a 4-wide area.
        assert_eq!(draw(&tree, 4, 1), vec!["abcd"]);
    }

    #[test]
    fn content_past_the_bottom_is_dropped_not_overdrawn() {
        let tree = ViewNode::List(vec![
            ViewNode::text("one"),
            ViewNode::text("two"),
            ViewNode::text("three"),
        ]);
        // Only two rows available: the third must not wrap onto another.
        assert_eq!(draw(&tree, 10, 2), vec!["one", "two"]);
    }

    #[test]
    fn a_zero_sized_area_draws_nothing() {
        let rect = area(0, 0);
        let mut buf = Buffer::empty(rect);
        // Must not panic or index out of bounds.
        render_tree(
            &ViewNode::text("x"),
            rect,
            &palette(),
            &FrameTable::default(),
            &mut buf,
        );
    }

    #[test]
    fn an_empty_container_draws_nothing() {
        assert_eq!(draw(&ViewNode::Column(vec![]), 5, 2), vec!["", ""]);
        assert_eq!(draw(&ViewNode::Row(vec![]), 5, 1), vec![""]);
    }

    #[test]
    fn every_token_resolves_on_a_dark_palette() {
        let p = ThemePreset::Default.palette();
        for token in StyleToken::all() {
            // A token that resolved to the background would be invisible.
            assert_ne!(token_color(*token, &p), p.app_bg, "{token:?}");
        }
    }

    #[test]
    fn every_token_resolves_on_a_light_palette() {
        // The reason tokens exist rather than colours: the same plugin must
        // stay legible when the user switches to a light theme.
        let p = ThemePreset::CatppuccinLatte.palette();
        for token in StyleToken::all() {
            assert_ne!(token_color(*token, &p), p.app_bg, "{token:?}");
        }
    }

    #[test]
    fn a_token_changes_colour_with_the_theme() {
        let dark = ThemePreset::Default.palette();
        let light = ThemePreset::CatppuccinLatte.palette();
        assert_ne!(
            token_color(StyleToken::Accent, &dark),
            token_color(StyleToken::Accent, &light),
            "a plugin must follow a theme switch without knowing"
        );
    }

    /// Render a tree with a frame table and return the styled cells of one row,
    /// as `(symbol, fg)` pairs — the shape needed to prove two runs kept
    /// different styles on one line.
    fn draw_styled(node: &ViewNode, w: u16, frames: &FrameTable) -> Vec<(String, Color)> {
        let rect = area(w, 1);
        let mut buf = Buffer::empty(rect);
        render_tree(node, rect, &palette(), frames, &mut buf);
        (0..w)
            .map(|x| {
                let cell = &buf[(x, 0)];
                (cell.symbol().to_string(), cell.fg)
            })
            .collect()
    }

    #[test]
    fn a_line_packs_runs_at_their_own_width() {
        // The whole point: `row` would have given each of these half the area,
        // truncating the second to two columns.
        let tree = ViewNode::Line(vec![ViewNode::text("ab"), ViewNode::text("cdefgh")]);
        assert_eq!(draw(&tree, 12, 1), vec!["abcdefgh"]);
    }

    #[test]
    fn a_one_column_run_does_not_get_an_equal_share() {
        let tree = ViewNode::Line(vec![
            ViewNode::text("*"),
            ViewNode::text("twenty-characters-x"),
        ]);
        let cells = draw_styled(&tree, 40, &FrameTable::default());
        assert_eq!(cells[0].0, "*");
        assert_eq!(cells[1].0, "t", "the second run starts at column 1");
    }

    #[test]
    fn runs_in_a_line_keep_their_own_styles() {
        let p = palette();
        let tree = ViewNode::Line(vec![
            ViewNode::styled(
                "L:",
                TextStyle {
                    token: Some(StyleToken::Muted),
                    bold: false,
                },
            ),
            ViewNode::styled(
                "V",
                TextStyle {
                    token: Some(StyleToken::Accent),
                    bold: false,
                },
            ),
        ]);
        let cells = draw_styled(&tree, 10, &FrameTable::default());
        assert_eq!(cells[0].1, p.text_muted);
        assert_eq!(cells[2].1, p.accent);
        assert_ne!(
            cells[0].1, cells[2].1,
            "one pre-composed text node could not have done this"
        );
    }

    #[test]
    fn a_line_longer_than_the_pane_is_clipped_not_wrapped() {
        let tree = ViewNode::List(vec![
            ViewNode::Line(vec![ViewNode::text("aaaa"), ViewNode::text("bbbb")]),
            ViewNode::text("next"),
        ]);
        // The overflowing run must not push `next` off its row.
        assert_eq!(draw(&tree, 6, 2), vec!["aaaabb", "next"]);
    }

    #[test]
    fn an_empty_line_draws_nothing() {
        assert_eq!(draw(&ViewNode::Line(vec![]), 5, 1), vec![""]);
    }

    #[test]
    fn a_line_is_one_row_high() {
        let tree = ViewNode::Line(vec![ViewNode::text("a"), ViewNode::text("b")]);
        assert_eq!(height_of(&tree), 1);
    }

    #[test]
    fn a_run_after_a_motion_stays_put_across_frames() {
        // A motion reserves its widest frame, so the narrow frame is padded and
        // the following run's column does not depend on which frame shows.
        let motion = ViewNode::Motion {
            key: "dot".to_string(),
            keyed_by_id: true,
            motion: crate::session::motion::Motion::cycle(
                vec![ViewNode::text("."), ViewNode::text("...")],
                8,
                true,
            ),
        };
        let tree = ViewNode::Line(vec![motion, ViewNode::text("|end")]);

        let mut first = FrameTable::default();
        first.set("dot", 0);
        let mut second = FrameTable::default();
        second.set("dot", 1);

        let a = draw_styled(&tree, 12, &first);
        let b = draw_styled(&tree, 12, &second);
        let bar = |cells: &[(String, Color)]| {
            cells
                .iter()
                .position(|(s, _)| s == "|")
                .expect("the following run is drawn")
        };
        assert_eq!(bar(&a), 3, "the narrow frame is padded to the widest");
        assert_eq!(bar(&b), bar(&a), "advancing a frame must not move siblings");
    }

    #[test]
    fn inline_width_counts_display_columns_not_characters() {
        // A CJK glyph is two cells; counting characters would leave every run
        // after it one column adrift.
        assert_eq!(inline_width(&ViewNode::text("字")), 2);
        assert_eq!(inline_width(&ViewNode::text("ab")), 2);
        assert_eq!(
            inline_width(&ViewNode::Line(vec![
                ViewNode::text("字"),
                ViewNode::text("x")
            ])),
            3
        );
    }

    #[test]
    fn height_accounts_for_nesting() {
        let tree = ViewNode::Column(vec![
            ViewNode::text("a"),
            ViewNode::Spacer { lines: 3 },
            ViewNode::Row(vec![ViewNode::text("b"), ViewNode::text("c")]),
        ]);
        // 1 + 3 + max(1, 1)
        assert_eq!(height_of(&tree), 5);
    }
}
