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
        ViewNode::Text { .. } | ViewNode::Divider => 1,
        ViewNode::Spacer { lines } => *lines,
        ViewNode::Row(children) => children.iter().map(height_of).max().unwrap_or(0),
        ViewNode::Column(children) | ViewNode::List(children) => {
            children.iter().map(height_of).sum::<u16>()
        }
        ViewNode::Motion { motion, .. } => motion.frames().iter().map(height_of).max().unwrap_or(0),
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
