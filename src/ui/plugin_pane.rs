//! Rendering a [`ViewNode`] tree into a pane.
//!
//! Named for the pane kind it was written for, but **not** limited to one: since
//! Phase 0's exit criterion this is also how thurbox's own
//! [`crate::ui::info_panel`] is painted, and it compiles in every build rather
//! than only with the plugin host. The name is kept as a deliberate
//! non-decision — renaming it carries no behaviour and would have obscured the
//! diff that ungated it.
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
use ratatui::widgets::{Paragraph, Widget, Wrap};
use unicode_width::UnicodeWidthStr;

use super::RowHitbox;
use crate::session::motion::FrameTable;
use crate::session::theme_config::ThemePalette;
use crate::session::view_tree::{DiffTint, StyleToken, TextStyle, ViewNode};

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
        StyleToken::Secondary => palette.text_secondary,
        StyleToken::Role => palette.role_name,
        StyleToken::Branch => palette.branch_name,
        StyleToken::AccentBright => palette.accent_bright,
        // `tool_allowed` rather than `diff_added`: this is the green thurbox's
        // own panes draw an insertion count in, and the two are separate
        // palette fields a custom theme may set independently.
        StyleToken::Added => palette.tool_allowed,
        StyleToken::Border => palette.border_unfocused,
        StyleToken::StatusWorking => palette.status_working,
        StyleToken::StatusBlocked => palette.status_blocked,
        StyleToken::StatusDone => palette.status_done,
        StyleToken::StatusIdle => palette.status_idle,
        StyleToken::StatusError => palette.status_error,
        StyleToken::StatusUnreachable => palette.status_unreachable,
    }
}

/// Turn a node's text style into a ratatui style.
fn text_style(style: TextStyle, palette: &ThemePalette) -> Style {
    // Selection is a *pair* the theme owns, and it replaces the token's colour
    // rather than layering over it: a run on the cursor's row is drawn in the
    // theme's selection appearance whatever role it would otherwise have had.
    // The three emphases below still apply, so a selected row can be bold.
    let mut s = if style.selected {
        Style::default()
            .bg(palette.selection_bg)
            .fg(palette.selection_fg)
    } else {
        let color = style
            .token
            .map(|t| token_color(t, palette))
            .unwrap_or(palette.text_primary);
        let s = Style::default().fg(color);
        // A tint is the other background role, and selection above already won:
        // the cursor's row is one appearance whatever it contains, which is the
        // precedence `ui::code_review::row_bg_fn` encodes for the native pane.
        // It leaves the foreground to the token, because a diff body's colours
        // are the pane's while the tint is what carries add versus remove.
        match style.tint {
            Some(DiffTint::Added) => s.bg(palette.diff_added_bg),
            Some(DiffTint::Removed) => s.bg(palette.diff_removed_bg),
            None => s,
        }
    };
    if style.bold {
        s = s.add_modifier(Modifier::BOLD);
    }
    // Attributes over the token's colour, never instead of it: a dimmed run is
    // still whatever colour the theme resolved, which is what lets a list row
    // dim without the pane naming a second palette entry.
    if style.dim {
        s = s.add_modifier(Modifier::DIM);
    }
    if style.underline {
        s = s.add_modifier(Modifier::UNDERLINED);
    }
    s
}

/// How many terminal rows a node needs at `width`.
///
/// Text is one line and does not wrap — a plugin splits lines by returning
/// separate nodes. [`ViewNode::Paragraph`] is the one kind whose height depends
/// on the width it is given, and it is why this takes a width at all: a
/// paragraph that wrapped onto three rows while being allocated one would
/// overdraw the sibling below it.
///
/// A motion takes the **tallest** of its frames rather than the height of the
/// frame currently showing: a height that changed per frame would shove every
/// sibling up and down as the animation ran, which is a worse artifact than
/// the blank row a short frame leaves.
fn height_of(node: &ViewNode, width: u16, palette: &ThemePalette, frames: &FrameTable) -> u16 {
    match node {
        // A line is one row whatever it holds: it clips rather than wraps, so
        // its height does not depend on the width it is given.
        ViewNode::Text { .. } | ViewNode::Divider | ViewNode::Fill { .. } | ViewNode::Line(_) => 1,
        // Header rows plus the bar's one row. The header is normally one row,
        // but a label and suffix that together overflow the width wrap — and the
        // bar has to move down with them rather than being drawn over the
        // overflow.
        ViewNode::Gauge {
            label,
            percent,
            suffix,
        } => gauge_header_rows(label, percent.get(), suffix.as_deref(), width, palette) + 1,
        ViewNode::Spacer { lines } => *lines,
        ViewNode::Paragraph(runs) => {
            if width == 0 {
                return 0;
            }
            let mut spans = Vec::new();
            inline_spans(runs, width as usize, palette, frames, &mut spans);
            // Measured by the same widget that will draw it, so the height and
            // the paint can never disagree about where the wrap falls.
            wrapped_paragraph(spans)
                .line_count(width)
                .min(u16::MAX as usize) as u16
        }
        ViewNode::Row(children) => row_cells(children, width)
            .iter()
            .zip(children)
            .map(|(cell, child)| height_of(child, cell.width, palette, frames))
            .max()
            .unwrap_or(0),
        ViewNode::Column(children) | ViewNode::List { children, .. } => children
            .iter()
            .map(|c| height_of(c, width, palette, frames))
            .sum::<u16>(),
        ViewNode::Motion { motion, .. } => motion
            .frames()
            .iter()
            .map(|f| height_of(f, width, palette, frames))
            .max()
            .unwrap_or(0),
    }
}

/// The equal-share cells a [`ViewNode::Row`] divides `width` into.
///
/// Shared by the height walk and the paint so the two split identically; a
/// hand-rolled `width / n` would drift from `Layout`'s remainder distribution.
fn row_cells(children: &[ViewNode], width: u16) -> std::rc::Rc<[Rect]> {
    let constraints: Vec<Constraint> = children
        .iter()
        .map(|_| Constraint::Ratio(1, children.len().max(1) as u32))
        .collect();
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints(constraints)
        .split(Rect {
            x: 0,
            y: 0,
            width,
            height: 1,
        })
}

/// The widget a [`ViewNode::Paragraph`] is both measured and drawn with.
///
/// `trim: false` keeps leading whitespace on a wrapped row, which is what makes
/// an indented continuation line up under the value it continues.
fn wrapped_paragraph(spans: Vec<Span<'_>>) -> Paragraph<'_> {
    Paragraph::new(Line::from(spans)).wrap(Wrap { trim: false })
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
        // A fill has no intrinsic width — it *is* whatever the line has left,
        // so it reserves nothing and every other run measures first. That is
        // what makes the residue well defined.
        ViewNode::Fill { .. } => 0,
        // Refused at conversion, so unreachable from a plugin; zero is the only
        // honest answer for a node whose width comes from its area.
        ViewNode::Row(_)
        | ViewNode::Paragraph(_)
        | ViewNode::Column(_)
        | ViewNode::List { .. }
        | ViewNode::Divider
        | ViewNode::Gauge { .. }
        | ViewNode::Spacer { .. } => 0,
    }
}

/// The spans a one-row node draws as, resolved against `width`.
///
/// Exposed for a **native** pane whose rows are view-tree nodes but whose *list*
/// is not — thurbox's session list keeps its ratatui list widget, so its rows
/// have to become `Line`s rather than be painted into rects. Sharing this walk is
/// what makes a `Fill`'s residue come out at the same column in the native pane
/// and in a plugin reproducing it; two implementations of that arithmetic would
/// be two panes that disagree about where a selection bar ends.
///
/// A node that is not a [`ViewNode::Line`] is treated as a line of one run, so a
/// caller may pass either.
pub fn line_spans<'a>(
    node: &'a ViewNode,
    width: u16,
    palette: &ThemePalette,
    frames: &FrameTable,
) -> Vec<Span<'a>> {
    let mut out = Vec::new();
    let runs = match node {
        ViewNode::Line(runs) => runs.as_slice(),
        other => std::slice::from_ref(other),
    };
    inline_spans(runs, width as usize, palette, frames, &mut out);
    out
}

/// Flatten a line's runs into spans, in order, padding each to the width it
/// reserved.
///
/// Padding only ever applies to a motion (a text run occupies exactly its own
/// width), and it goes on the right so the *start* of an animation stays fixed
/// — that is the column the eye tracks.
fn inline_spans<'a>(
    runs: &'a [ViewNode],
    width: usize,
    palette: &ThemePalette,
    frames: &FrameTable,
    out: &mut Vec<Span<'a>>,
) {
    // Fills are emitted empty and filled in afterwards, because how wide one is
    // depends on every *other* run on the line — measuring forwards would need
    // the answer before it exists. The placeholder keeps each fill's position,
    // so a fill in the middle of a line still pushes what follows it right.
    let mut placeholders: Vec<(usize, char)> = Vec::new();
    collect_inline_spans(runs, palette, frames, out, &mut placeholders);
    if placeholders.is_empty() {
        return;
    }
    let drawn: usize = out.iter().map(|s| s.content.width()).sum();
    let residue = width.saturating_sub(drawn);
    // Split evenly, remainder to the first: an even split is the only rule that
    // does not privilege a position, and a leftover column has to go somewhere.
    let share = residue / placeholders.len();
    let mut extra = residue % placeholders.len();
    for (index, glyph) in placeholders {
        let mut take = share;
        if extra > 0 {
            take += 1;
            extra -= 1;
        }
        if take == 0 {
            continue;
        }
        let style = out[index].style;
        out[index] = Span::styled(glyph.to_string().repeat(take), style);
    }
}

/// The walk [`inline_spans`] wraps: push one span per run, recording where each
/// fill's placeholder landed.
fn collect_inline_spans<'a>(
    runs: &'a [ViewNode],
    palette: &ThemePalette,
    frames: &FrameTable,
    out: &mut Vec<Span<'a>>,
    placeholders: &mut Vec<(usize, char)>,
) {
    for run in runs {
        match run {
            ViewNode::Text { content, style } => {
                out.push(Span::styled(content.clone(), text_style(*style, palette)));
            }
            ViewNode::Line(nested) => {
                collect_inline_spans(nested, palette, frames, out, placeholders)
            }
            ViewNode::Fill { glyph, style } => {
                placeholders.push((out.len(), *glyph));
                out.push(Span::styled(String::new(), text_style(*style, palette)));
            }
            ViewNode::Motion { key, motion, .. } => {
                let reserved = inline_width(run);
                let index = frames
                    .frame(key)
                    .min(motion.frames().len().saturating_sub(1));
                let before = out.len();
                if let Some(frame) = motion.frames().get(index) {
                    collect_inline_spans(
                        std::slice::from_ref(frame),
                        palette,
                        frames,
                        out,
                        placeholders,
                    );
                }
                let drawn: usize = out[before..].iter().map(|s| s.content.width()).sum();
                if let Some(pad) = reserved.checked_sub(drawn).filter(|p| *p > 0) {
                    out.push(Span::raw(" ".repeat(pad)));
                }
            }
            // Refused at conversion; drawing nothing is the safe residual.
            ViewNode::Row(_)
            | ViewNode::Paragraph(_)
            | ViewNode::Column(_)
            | ViewNode::List { .. }
            | ViewNode::Divider
            | ViewNode::Gauge { .. }
            | ViewNode::Spacer { .. } => {}
        }
    }
}

/// A gauge's header row: the label at the left, the suffix (or the rounded
/// percentage) flush right.
///
/// The flush-right placement is the kernel's, which is the whole reason the node
/// exists — nothing that draws a gauge needs to know how wide its pane resolved
/// to. When the label and the right-hand text together exceed the width the
/// padding is zero and the line **overflows**, which is why the header is drawn
/// wrapped rather than as one row.
///
/// The percentage is clamped **here**, at the moment of drawing, rather than
/// when the node was built — so a metric that momentarily overshoots draws a
/// full bar instead of failing the pane.
fn gauge_header(
    label: &str,
    clamped: f32,
    suffix: Option<&str>,
    width: usize,
    palette: &ThemePalette,
) -> Line<'static> {
    let right_text = suffix
        .map(str::to_string)
        .unwrap_or_else(|| format!("{clamped:.0}%"));
    let padding = width.saturating_sub(label.chars().count() + right_text.chars().count());
    Line::from(vec![
        Span::styled(label.to_string(), Style::default().fg(palette.text_muted)),
        Span::raw(" ".repeat(padding)),
        Span::styled(right_text, Style::default().fg(palette.text_primary)),
    ])
}

/// Rows a gauge's header takes at `width` — more than one only when the label
/// plus the right-hand text overflow it.
fn gauge_header_rows(
    label: &str,
    percent: f32,
    suffix: Option<&str>,
    width: u16,
    palette: &ThemePalette,
) -> u16 {
    if width == 0 {
        return 1;
    }
    let header = gauge_header(
        label,
        percent.clamp(0.0, 100.0),
        suffix,
        width as usize,
        palette,
    );
    wrapped_paragraph(header.spans)
        .line_count(width)
        .max(1)
        .min(u16::MAX as usize) as u16
}

/// Draw a [`ViewNode::Gauge`]: its header, then the bar on the row below
/// whatever the header took.
fn render_gauge(
    label: &str,
    percent: f32,
    suffix: Option<&str>,
    area: Rect,
    palette: &ThemePalette,
    buf: &mut ratatui::buffer::Buffer,
) {
    let width = area.width as usize;
    let clamped = percent.clamp(0.0, 100.0);
    let header_rows = gauge_header_rows(label, percent, suffix, area.width, palette);
    let header = gauge_header(label, clamped, suffix, width, palette);
    wrapped_paragraph(header.spans).render(
        Rect {
            height: header_rows.min(area.height),
            ..area
        },
        buf,
    );

    // The bar owns the row after the header. A gauge clipped shorter than that
    // simply has none.
    if area.height <= header_rows {
        return;
    }
    let area = Rect {
        y: area.y.saturating_add(header_rows),
        height: area.height - header_rows,
        ..area
    };
    let bar_width = width.saturating_sub(2);
    let filled = ((clamped / 100.0) * bar_width as f32).round() as usize;
    let empty = bar_width.saturating_sub(filled);
    let muted = Style::default().fg(palette.text_muted);
    let bar_line = Line::from(vec![
        Span::styled("[", muted),
        Span::styled("█".repeat(filled), Style::default().fg(palette.accent)),
        Span::styled("░".repeat(empty), muted),
        Span::styled("]", muted),
    ]);
    Paragraph::new(bar_line).render(Rect { height: 1, ..area }, buf);
}

/// Draw `children` top to bottom, each at the height it asks for, stopping at
/// the bottom of `area`.
///
/// Shared by [`ViewNode::Column`] and [`ViewNode::List`] so the two stack
/// identically; a list's only difference is *which* children it hands over.
fn render_stacked(
    children: &[ViewNode],
    area: Rect,
    palette: &ThemePalette,
    frames: &FrameTable,
    buf: &mut ratatui::buffer::Buffer,
    rows: &mut Option<RowSink>,
) -> Vec<Rect> {
    let mut drawn = Vec::new();
    let mut y = area.y;
    let bottom = area.y.saturating_add(area.height);
    for child in children {
        if y >= bottom {
            break;
        }
        let want = height_of(child, area.width, palette, frames);
        let height = want.min(bottom - y);
        if height == 0 {
            // A child with no height is not drawn, so it has no rect — and the
            // returned list is positional, so a placeholder would misalign every
            // row after it against its own index.
            drawn.push(Rect {
                x: area.x,
                y,
                width: area.width,
                height: 0,
            });
            continue;
        }
        let rect = Rect {
            x: area.x,
            y,
            width: area.width,
            height,
        };
        paint(child, rect, palette, frames, buf, rows);
        drawn.push(rect);
        y = y.saturating_add(height);
    }
    drawn
}

/// Collects the rects of the **outermost** list's rows while painting.
///
/// Outermost, and claimed once: a nested list's rows would give one click two
/// answers, and the pane's rows are what a user points at.
#[derive(Default)]
struct RowSink {
    rows: Vec<RowHitbox>,
    claimed: bool,
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
    paint(node, area, palette, frames, buf, &mut None);
}

/// Draw a view tree and report its clickable rows: one hitbox per row of the
/// **outermost** list, carrying that row's **1-based index in the whole list**.
///
/// Derived from what was painted rather than recomputed afterwards: the kernel
/// windows a list that names its cursor (ADR-30), so a second walk would have to
/// reproduce that arithmetic and could resolve a click against a layout other than
/// the one on screen. The index is list-space for the same reason — a plugin never
/// learns the window, so a screen position would be a number it cannot interpret.
///
/// A tree with no list has no rows. A column of lines is not a list of rows, and
/// guessing otherwise would let a click select something the plugin does not model.
pub fn render_tree_rows(
    node: &ViewNode,
    area: Rect,
    palette: &ThemePalette,
    frames: &FrameTable,
    buf: &mut ratatui::buffer::Buffer,
) -> Vec<RowHitbox> {
    let mut sink = Some(RowSink::default());
    paint(node, area, palette, frames, buf, &mut sink);
    sink.map(|s| s.rows).unwrap_or_default()
}

/// The painter both entry points share; `rows` is `Some` only when the caller
/// wants the outermost list's hitboxes, so an ordinary paint allocates nothing
/// extra.
fn paint(
    node: &ViewNode,
    area: Rect,
    palette: &ThemePalette,
    frames: &FrameTable,
    buf: &mut ratatui::buffer::Buffer,
    rows: &mut Option<RowSink>,
) {
    if area.width == 0 || area.height == 0 {
        return;
    }

    match node {
        ViewNode::Text { content, style } => {
            let line = Line::from(Span::styled(content.clone(), text_style(*style, palette)));
            Paragraph::new(line).render(area, buf);
        }
        // A fill outside a line has the whole row to itself, which is the one
        // sensible reading of "the width that is left" when nothing else is on
        // it — and it keeps the node from being a special case only a line can
        // hold.
        ViewNode::Fill { glyph, style } => {
            let run = glyph.to_string().repeat(area.width as usize);
            Paragraph::new(Line::from(Span::styled(run, text_style(*style, palette))))
                .render(area, buf);
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
            inline_spans(runs, area.width as usize, palette, frames, &mut spans);
            Paragraph::new(Line::from(spans)).render(area, buf);
        }
        ViewNode::Paragraph(runs) => {
            let mut spans = Vec::new();
            inline_spans(runs, area.width as usize, palette, frames, &mut spans);
            // Overflow wraps onto further rows and is clipped by `area`'s
            // bottom, so a paragraph given fewer rows than it wants loses its
            // tail rather than painting over a sibling.
            wrapped_paragraph(spans).render(area, buf);
        }
        ViewNode::Gauge {
            label,
            percent,
            suffix,
        } => render_gauge(label, percent.get(), suffix.as_deref(), area, palette, buf),
        ViewNode::Row(children) => {
            if children.is_empty() {
                return;
            }
            // Equal shares: a plugin cannot specify widths, so the kernel does
            // not have to arbitrate between competing requests.
            for (child, cell) in children.iter().zip(row_cells(children, area.width).iter()) {
                paint(
                    child,
                    Rect {
                        x: area.x.saturating_add(cell.x),
                        y: area.y,
                        width: cell.width,
                        height: area.height,
                    },
                    palette,
                    frames,
                    buf,
                    rows,
                );
            }
        }
        ViewNode::Column(children) => {
            render_stacked(children, area, palette, frames, buf, rows);
        }
        ViewNode::List { children, selected } => {
            // The one place the kernel resolves a *height* on a plugin's behalf.
            // A plugin is never told how many rows it got, so a list that
            // declares which child the cursor is on hands the scroll decision
            // here — through the same `visible_window` thurbox's own panes use,
            // which is what lets a native pane and a plugin reproducing it paint
            // the same frame rather than merely build the same tree.
            let window = selected.map(|selected| {
                super::file_viewer::visible_window(children.len(), selected, area.height as usize)
            });
            let (start, end) = window.unwrap_or((0, children.len()));
            let drawn = render_stacked(&children[start..end], area, palette, frames, buf, rows);
            if let Some(sink) = rows.as_mut() {
                if !sink.claimed {
                    // Claimed even when the list is empty, so a *nested* list can
                    // never become the pane's rows by being the first one with
                    // children in it.
                    sink.claimed = true;
                    sink.rows = drawn
                        .iter()
                        .enumerate()
                        .filter(|(_, rect)| rect.height > 0)
                        .map(|(i, rect)| RowHitbox {
                            rect: *rect,
                            index: start + i + 1,
                        })
                        .collect();
                }
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
                paint(frame, area, palette, frames, buf, rows);
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
        let tree = ViewNode::list(vec![
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
        let tree = ViewNode::list(vec![
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
        let tree = ViewNode::list(vec![
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

    /// Emphasis is per run, over the token's colour: a dimmed or underlined run
    /// keeps the colour the theme resolved and its neighbour keeps neither
    /// attribute. Without both a list row's three appearances collapse into one.
    #[test]
    fn emphasis_is_applied_over_the_token_and_only_to_its_own_run() {
        let p = palette();
        let rect = area(10, 1);
        let mut buf = Buffer::empty(rect);
        render_tree(
            &ViewNode::Line(vec![
                ViewNode::styled(
                    "d",
                    TextStyle {
                        token: Some(StyleToken::Muted),
                        dim: true,
                        ..TextStyle::default()
                    },
                ),
                ViewNode::styled(
                    "u",
                    TextStyle {
                        token: Some(StyleToken::Accent),
                        bold: true,
                        underline: true,
                        ..TextStyle::default()
                    },
                ),
                ViewNode::text("p"),
            ]),
            rect,
            &p,
            &FrameTable::default(),
            &mut buf,
        );

        let dim = &buf[(0, 0)];
        assert_eq!(dim.fg, p.text_muted, "dim does not replace the colour");
        assert!(dim.modifier.contains(Modifier::DIM));
        assert!(!dim.modifier.contains(Modifier::UNDERLINED));

        let under = &buf[(1, 0)];
        assert_eq!(under.fg, p.accent);
        assert!(under.modifier.contains(Modifier::UNDERLINED));
        assert!(under.modifier.contains(Modifier::BOLD));
        assert!(!under.modifier.contains(Modifier::DIM));

        let plain = &buf[(2, 0)];
        assert_eq!(plain.fg, p.text_primary);
        assert!(
            !plain
                .modifier
                .intersects(Modifier::DIM | Modifier::UNDERLINED | Modifier::BOLD),
            "emphasis must not bleed onto the next run"
        );
    }

    /// The selection role: both palette colours come from the theme, and the run
    /// keeps whatever emphasis it declared.
    #[test]
    fn a_selected_run_takes_the_themes_selection_pair() {
        let p = palette();
        let rect = area(6, 1);
        let mut buf = Buffer::empty(rect);
        render_tree(
            &ViewNode::Line(vec![
                ViewNode::styled(
                    "s",
                    TextStyle {
                        // A token that would otherwise have coloured it: the
                        // selection has to win, or a selected muted row would
                        // read as unselected.
                        token: Some(StyleToken::Muted),
                        bold: true,
                        selected: true,
                        ..TextStyle::default()
                    },
                ),
                ViewNode::token("n", StyleToken::Muted),
            ]),
            rect,
            &p,
            &FrameTable::default(),
            &mut buf,
        );

        let sel = &buf[(0, 0)];
        assert_eq!(sel.fg, p.selection_fg);
        assert_eq!(sel.bg, p.selection_bg);
        assert!(
            sel.modifier.contains(Modifier::BOLD),
            "emphasis still applies"
        );

        let next = &buf[(1, 0)];
        assert_eq!(next.fg, p.text_muted, "the neighbour keeps its own token");
        assert_ne!(next.bg, p.selection_bg, "the selection does not bleed");
    }

    // ── a list that scrolls to its cursor ──

    fn rows(labels: &[&str], selected: Option<usize>) -> ViewNode {
        ViewNode::selectable_list(
            labels.iter().map(|l| ViewNode::text(*l)).collect(),
            selected,
        )
    }

    #[test]
    fn a_list_that_fits_draws_every_row_whether_or_not_it_has_a_cursor() {
        let labels = ["a", "b", "c"];
        assert_eq!(draw(&rows(&labels, Some(2)), 4, 3), vec!["a", "b", "c"]);
        assert_eq!(draw(&rows(&labels, None), 4, 3), vec!["a", "b", "c"]);
    }

    /// The whole point of the field: without it the cursor's row is below the
    /// fold and the pane shows a list with no visible selection.
    #[test]
    fn a_list_longer_than_its_area_scrolls_to_its_cursor() {
        let labels = ["r0", "r1", "r2", "r3", "r4", "r5", "r6", "r7"];
        let drawn = draw(&rows(&labels, Some(7)), 4, 3);
        assert!(drawn.contains(&"r7".to_string()), "{drawn:?}");
        assert!(!drawn.contains(&"r0".to_string()), "{drawn:?}");
        // And the same list without a cursor still starts at the top, so the
        // field changes nothing a plugin did not ask for.
        assert_eq!(draw(&rows(&labels, None), 4, 3), vec!["r0", "r1", "r2"]);
    }

    /// The window is the *shared* one, not a second rule that happens to agree:
    /// a native pane derives its click hitboxes from `visible_window` and the
    /// renderer paints from it, and the two must not be able to drift.
    #[test]
    fn the_window_is_the_one_the_native_panes_use() {
        let labels: Vec<String> = (0..20).map(|i| format!("r{i}")).collect();
        let refs: Vec<&str> = labels.iter().map(String::as_str).collect();
        for selected in [0usize, 7, 19] {
            let (start, end) = super::super::file_viewer::visible_window(refs.len(), selected, 5);
            let expected: Vec<String> = refs[start..end].iter().map(|s| s.to_string()).collect();
            assert_eq!(draw(&rows(&refs, Some(selected)), 4, 5), expected);
        }
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
                    ..TextStyle::default()
                },
            ),
            ViewNode::styled(
                "V",
                TextStyle {
                    token: Some(StyleToken::Accent),
                    ..TextStyle::default()
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
        let tree = ViewNode::list(vec![
            ViewNode::Line(vec![ViewNode::text("aaaa"), ViewNode::text("bbbb")]),
            ViewNode::text("next"),
        ]);
        // The overflowing run must not push `next` off its row.
        assert_eq!(draw(&tree, 6, 2), vec!["aaaabb", "next"]);
    }

    // ── paragraph: the node that wraps ──

    #[test]
    fn a_paragraph_wraps_instead_of_clipping() {
        // The same content a line would have clipped (see the test above).
        let tree = ViewNode::Paragraph(vec![ViewNode::text("aaaa"), ViewNode::text("bbbb")]);
        // No break opportunity inside the run, so the wrap falls at the width.
        assert_eq!(draw(&tree, 6, 2), vec!["aaaabb", "bb"]);
    }

    #[test]
    fn a_wrapping_paragraph_does_not_overdraw_its_sibling() {
        // The reason `height_of` takes a width at all: allocate one row here and
        // `next` would be painted over by the wrap.
        let tree = ViewNode::list(vec![
            ViewNode::Paragraph(vec![ViewNode::text("one two three four")]),
            ViewNode::text("next"),
        ]);
        assert_eq!(
            draw(&tree, 6, 5),
            vec!["one", "two", "three", "four", "next"]
        );
    }

    #[test]
    fn a_paragraph_that_fits_is_one_row() {
        let tree = ViewNode::list(vec![
            ViewNode::Paragraph(vec![ViewNode::text("short")]),
            ViewNode::text("next"),
        ]);
        assert_eq!(draw(&tree, 20, 2), vec!["short", "next"]);
    }

    #[test]
    fn a_paragraph_is_truncated_by_the_bottom_of_its_area() {
        let tree = ViewNode::Paragraph(vec![ViewNode::text("one two three")]);
        // Wants three rows, given two: the tail is dropped, not overflowed.
        assert_eq!(draw(&tree, 5, 2), vec!["one", "two"]);
    }

    #[test]
    fn runs_in_a_paragraph_keep_their_styles_across_the_wrap() {
        let p = palette();
        let tree = ViewNode::Paragraph(vec![
            ViewNode::styled(
                "L: ",
                TextStyle {
                    token: Some(StyleToken::Muted),
                    ..TextStyle::default()
                },
            ),
            ViewNode::text("wrapping value"),
        ]);
        let rect = area(8, 3);
        let mut buf = Buffer::empty(rect);
        render_tree(&tree, rect, &p, &FrameTable::default(), &mut buf);
        assert_eq!(buf[(0, 0)].fg, p.text_muted, "the label keeps its token");
        // The wrapped remainder belongs to the value run, not the label.
        assert_eq!(buf[(0, 1)].fg, p.text_primary);
    }

    // ── gauge: the node the kernel resolves geometry for ──

    #[test]
    fn a_gauge_puts_its_suffix_flush_right_and_fills_its_bar() {
        let tree = ViewNode::gauge("RAM", 50.0, Some("8/16 GB".to_string()));
        let rows = draw(&tree, 20, 2);
        assert_eq!(rows[0], "RAM          8/16 GB", "flush right");
        // 20 wide: brackets plus an 18-cell bar, half filled.
        assert_eq!(rows[1].chars().count(), 20);
        assert_eq!(rows[1].matches('█').count(), 9);
        assert_eq!(rows[1].matches('░').count(), 9);
    }

    #[test]
    fn a_gauge_without_a_suffix_shows_its_percentage() {
        // Flush right in a 12-wide area: "CPU", six spaces, then "37%".
        assert_eq!(
            draw(&ViewNode::gauge("CPU", 37.0, None), 12, 1),
            vec!["CPU      37%"]
        );
    }

    #[test]
    fn a_gauge_clamps_out_of_range_percentages() {
        let over = draw(&ViewNode::gauge("CPU", 150.0, None), 12, 2);
        assert!(over[0].contains("100%"));
        assert_eq!(over[1].matches('░').count(), 0, "a full bar");
        let under = draw(&ViewNode::gauge("CPU", -10.0, None), 12, 2);
        assert!(under[0].contains("0%"));
        assert_eq!(under[1].matches('█').count(), 0, "an empty bar");
    }

    #[test]
    fn a_gauge_reserves_two_rows_so_a_sibling_follows_its_bar() {
        let tree = ViewNode::list(vec![
            ViewNode::gauge("CPU", 50.0, None),
            ViewNode::text("after"),
        ]);
        assert_eq!(draw(&tree, 12, 3)[2], "after");
    }

    #[test]
    fn a_gauge_whose_header_overflows_pushes_its_bar_down() {
        // The case the info panel's differential sweep found: label plus suffix
        // exceed the width, so the header wraps and the bar must follow it rather
        // than being painted over the overflow.
        let tree = ViewNode::list(vec![
            ViewNode::gauge("Context", 7.0, None),
            ViewNode::text("after"),
        ]);
        let rows = draw(&tree, 6, 4);
        assert_eq!(rows[0], "Contex");
        assert_eq!(rows[1], "t7%");
        assert!(rows[2].starts_with('['), "the bar is on the third row");
        assert_eq!(rows[3], "after", "and the sibling after that");
    }

    #[test]
    fn neither_new_node_is_inlineable() {
        // Both take their width from their area, and a paragraph its height from
        // the wrap, so a one-row line cannot hold either.
        for node in [ViewNode::Paragraph(vec![]), ViewNode::gauge("x", 0.0, None)] {
            assert!(!node.is_inlineable(), "{}", node.kind_name());
            assert_eq!(inline_width(&node), 0);
        }
    }

    #[test]
    fn two_identical_gauges_compare_equal() {
        // Load-bearing: an identical re-push must keep a motion's epoch, which
        // needs the whole tree to stay comparable despite carrying a float.
        let a = ViewNode::gauge("CPU", 42.5, Some("s".into()));
        let b = ViewNode::gauge("CPU", 42.5, Some("s".into()));
        assert_eq!(a, b);
        assert_ne!(a, ViewNode::gauge("CPU", 42.6, Some("s".into())));
    }

    #[test]
    fn an_empty_line_draws_nothing() {
        assert_eq!(draw(&ViewNode::Line(vec![]), 5, 1), vec![""]);
    }

    #[test]
    fn a_line_is_one_row_high() {
        let tree = ViewNode::Line(vec![ViewNode::text("a"), ViewNode::text("b")]);
        assert_eq!(height_of(&tree, 10, &palette(), &FrameTable::default()), 1);
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
        assert_eq!(height_of(&tree, 10, &palette(), &FrameTable::default()), 5);
    }

    // ── clickable rows (ADR-36) ──────────────────────────────────────────────

    /// Paint into a `w × h` area and return the rows a click could hit.
    fn row_hits(node: &ViewNode, w: u16, h: u16) -> Vec<RowHitbox> {
        let rect = area(w, h);
        let mut buf = Buffer::empty(rect);
        render_tree_rows(node, rect, &palette(), &FrameTable::default(), &mut buf)
    }

    #[test]
    fn a_list_reports_one_hitbox_per_row_one_based() {
        let list = ViewNode::list(vec![
            ViewNode::text("first"),
            ViewNode::text("second"),
            ViewNode::text("third"),
        ]);
        let hits = row_hits(&list, 20, 5);
        assert_eq!(hits.len(), 3);
        // One-based, matching the index a list uses to name its cursor.
        assert_eq!(hits[0].index, 1);
        assert_eq!(hits[0].rect.y, 0);
        assert_eq!(hits[1].index, 2);
        assert_eq!(hits[1].rect.y, 1);
        assert_eq!(hits[2].index, 3);
    }

    /// A row taller than one line owns every line it drew, so a click anywhere in
    /// it hits the same row.
    #[test]
    fn a_multi_line_row_owns_its_whole_rect() {
        let list = ViewNode::list(vec![
            ViewNode::Column(vec![ViewNode::text("a"), ViewNode::text("b")]),
            ViewNode::text("next"),
        ]);
        let hits = row_hits(&list, 20, 5);
        assert_eq!(hits.len(), 2);
        assert_eq!(hits[0].rect.height, 2);
        assert_eq!(hits[1].rect.y, 2);
    }

    /// The kernel windows a list that names its cursor, and the reported index is
    /// the row's place in the *whole* list — a plugin never learns the window, so a
    /// screen position would be a number it could not interpret.
    #[test]
    fn a_scrolled_list_reports_list_space_indices() {
        let children: Vec<ViewNode> = (0..10)
            .map(|i| ViewNode::text(format!("row {i}")))
            .collect();
        // Cursor on the last row, three lines of pane: the window ends at row 10.
        let list = ViewNode::selectable_list(children, Some(9));
        let hits = row_hits(&list, 20, 3);
        assert_eq!(hits.len(), 3);
        assert_eq!(
            hits.iter().map(|h| h.index).collect::<Vec<_>>(),
            vec![8, 9, 10],
            "the top visible row is row 8, not row 1"
        );
        assert_eq!(hits[0].rect.y, 0, "and it is drawn at the pane's top");
    }

    #[test]
    fn a_list_inside_a_column_is_still_the_panes_rows() {
        let tree = ViewNode::Column(vec![
            ViewNode::text("header"),
            ViewNode::list(vec![ViewNode::text("one"), ViewNode::text("two")]),
        ]);
        let hits = row_hits(&tree, 20, 5);
        assert_eq!(hits.len(), 2);
        // Offset by the header, because the rects come from what was painted.
        assert_eq!(hits[0].rect.y, 1);
        assert_eq!(hits[0].index, 1);
    }

    /// One click must not have two answers, so only the outermost list's rows are
    /// clickable.
    #[test]
    fn a_nested_list_contributes_no_rows() {
        let tree = ViewNode::list(vec![
            ViewNode::text("outer one"),
            ViewNode::list(vec![ViewNode::text("inner a"), ViewNode::text("inner b")]),
        ]);
        let hits = row_hits(&tree, 20, 6);
        assert_eq!(
            hits.iter().map(|h| h.index).collect::<Vec<_>>(),
            vec![1, 2],
            "the outer list has two rows; the inner one adds none"
        );
    }

    /// The rows belong to the first list that is **drawn**, not the first that
    /// appears in the tree: an empty list has no height, so it is never painted and
    /// claims nothing. Pinned because it is the one case where "outermost" and
    /// "first" differ, and the honest answer is the drawn one — a pane's rows are
    /// what a user can point at.
    #[test]
    fn the_rows_belong_to_the_first_drawn_list() {
        let tree = ViewNode::Column(vec![
            ViewNode::list(Vec::new()),
            ViewNode::list(vec![ViewNode::text("the only rows on screen")]),
        ]);
        let hits = row_hits(&tree, 20, 4);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].index, 1);
    }

    /// A list clipped entirely away likewise claims nothing, for the same reason.
    #[test]
    fn a_list_with_no_room_reports_nothing() {
        let list = ViewNode::list(vec![ViewNode::text("no room")]);
        assert!(row_hits(&list, 20, 0).is_empty());
    }

    #[test]
    fn a_tree_with_no_list_has_no_rows() {
        let tree = ViewNode::Column(vec![ViewNode::text("a"), ViewNode::text("b")]);
        assert!(
            row_hits(&tree, 20, 4).is_empty(),
            "a column of lines is not a list of rows"
        );
    }

    /// A row clipped away by the pane's height is not clickable, because it was
    /// never drawn.
    #[test]
    fn a_row_below_the_pane_is_not_clickable() {
        let list = ViewNode::list(vec![ViewNode::text("visible"), ViewNode::text("clipped")]);
        let hits = row_hits(&list, 20, 1);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].index, 1);
    }

    /// The plain painter must draw exactly what the reporting one draws — the rows
    /// are a by-product of the paint, not a second layout.
    #[test]
    fn reporting_rows_does_not_change_the_paint() {
        let tree = ViewNode::list(vec![
            ViewNode::Line(vec![ViewNode::text("a"), ViewNode::text("b")]),
            ViewNode::text("second"),
        ]);
        let rect = area(20, 4);
        let mut plain = Buffer::empty(rect);
        render_tree(&tree, rect, &palette(), &FrameTable::default(), &mut plain);
        let mut reporting = Buffer::empty(rect);
        render_tree_rows(
            &tree,
            rect,
            &palette(),
            &FrameTable::default(),
            &mut reporting,
        );
        assert_eq!(plain, reporting);
    }
}
