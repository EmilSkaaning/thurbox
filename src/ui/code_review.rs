//! Native renderer for the code-review view (the diff + interleaved comments +
//! summary + an in-view compose box), modeled on the file viewer. Pure
//! rendering: it returns click/scroll hitboxes for the app layer to record.

use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Clear, Paragraph};
use ratatui::Frame;

use crate::app::code_review::{CodeReviewState, ComposeState, ReviewButton, ReviewRow};
use crate::session::motion::FrameTable;
use crate::session::overlay::{Align, CrossExtent, Overlay};
use crate::session::pane_context as pc;
use crate::session::review::{
    pair_hunk, Classification, CommentAnchor, DiffFile, DiffHunk, DiffLine, DiffLineKind, SidePair,
};
use crate::session::view_tree::{DiffTint, StyleToken as Token, TextStyle, ViewNode};
use crate::ui::overlay::OverlayLayer;
use crate::ui::scrollbar::{self, ScrollbarGeom};
use crate::ui::theme::Theme;
use crate::ui::{focus_block, render_button_bar, ButtonSpec, FocusLevel, RowHitbox};

/// What the renderer hands back for the app layer to record as click/scroll
/// targets this frame.
pub(crate) struct CodeReviewHits {
    /// One hitbox per visible diff/comment row (index = row in `state.rows`).
    pub rows: Vec<RowHitbox>,
    /// One hitbox per visible target-picker entry (index = entry in the
    /// picker); empty unless the target picker is open.
    pub targets: Vec<RowHitbox>,
    /// Footer buttons paired with the action each triggers.
    pub buttons: Vec<(crate::ui::ButtonHit, ReviewButton)>,
    pub scrollbar: Option<ScrollbarGeom>,
    /// This pane's overlay rects, **topmost first** — recorded ahead of `rows`
    /// so a click on a floating box (the compose box) does not fall through to
    /// the diff row underneath it. Empty unless something is anchored.
    pub overlay: Vec<Rect>,
}

/// Theme color for a classification badge.
fn class_color(c: Classification) -> Color {
    match c {
        Classification::Issue => Theme::danger(),
        Classification::Suggestion => Theme::accent(),
        Classification::Note => Theme::text_secondary(),
        Classification::Praise => Theme::status_done(),
    }
}

/// Theme color for a file's status glyph (M/A/D/R): added green, deleted red,
/// modified yellow, renamed accent — so the status reads at a glance.
fn status_color(s: crate::session::review::FileStatus) -> Color {
    crate::ui::plugin_pane::token_color(status_token(s), &crate::ui::theme::current())
}

/// The colour **role** a file's status glyph is drawn in.
///
/// The token rather than the colour, in one place, because both of this pane's
/// rows carry the same glyph — the diff's file header and the changed-files
/// list's row — and a second mapping is how the two come to disagree about what
/// a rename looks like.
fn status_token(s: crate::session::review::FileStatus) -> Token {
    use crate::session::review::FileStatus::*;
    match s {
        Added => Token::DiffAdded,
        Deleted => Token::DiffRemoved,
        Modified => Token::StatusWorking,
        Renamed => Token::Accent,
    }
}

pub(crate) fn render(
    frame: &mut Frame,
    area: Rect,
    state: &mut CodeReviewState,
    level: FocusLevel,
    // Columns reserved on the left of the top border for the central-pane tab
    // strip; the right-aligned title is truncated to the remaining width. See
    // `terminal_view::render_terminal`.
    reserved_left: u16,
) -> CodeReviewHits {
    let (add, del) = state.totals();
    let target = state.target.label(&state.repos, &state.commits);
    let title = format!(" Code review · {target}  +{add} -{del} ");
    // Right-aligned so the app-layer central-pane tab strip (Agent/Shell/Review)
    // overlaid on the left of this top border has room; truncated to the space
    // the tabs leave so it can't overlap the pills.
    let title = crate::ui::fit_right_title(&title, area.width, reserved_left);
    let block = focus_block("", level)
        .title_top(Line::from(Span::styled(title, crate::ui::title_style(level))).right_aligned());
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.height == 0 || inner.width == 0 {
        return CodeReviewHits {
            rows: Vec::new(),
            targets: Vec::new(),
            buttons: Vec::new(),
            scrollbar: None,
            overlay: Vec::new(),
        };
    }

    // Footer (buttons) is always the last row; the diff uses the rest. A search
    // bar, when open, takes the top row of what's left.
    let footer = Rect::new(inner.x, inner.y + inner.height - 1, inner.width, 1);
    let mut diff_area = Rect::new(inner.x, inner.y, inner.width, inner.height - 1);
    if state.search.is_some() && diff_area.height > 1 {
        let search_row = Rect::new(diff_area.x, diff_area.y, diff_area.width, 1);
        diff_area = Rect::new(
            diff_area.x,
            diff_area.y + 1,
            diff_area.width,
            diff_area.height - 1,
        );
        render_search_bar(frame, search_row, state);
    }

    let composing = state.compose.is_some();
    // The target picker, when open, replaces the diff body. Its entries are
    // clickable (`targets`), on top of the keyboard-driven ↑/↓/Enter path.
    let mut targets = Vec::new();
    let hits = if state.loading {
        // A background build is in flight (ADR-P8): the pane opened instantly
        // and fills in when the worker delivers. No rows = nothing clickable.
        let msg = Line::from(Span::styled(
            "Building diff…",
            Style::default().fg(Theme::text_muted()),
        ))
        .centered();
        let y = diff_area.y + diff_area.height / 2;
        frame.render_widget(
            Paragraph::new(msg),
            Rect::new(diff_area.x, y, diff_area.width, 1),
        );
        (Vec::new(), None)
    } else if state.target_picker.is_some() {
        targets = render_target_picker(frame, diff_area, state);
        (Vec::new(), None)
    } else {
        render_rows(frame, diff_area, state)
    };
    // The compose box floats at the selected line rather than being pinned to
    // the bottom — GitHub-style line comments. It is an anchored overlay, so the
    // prefer-below / flip-above / dock decision is the shared resolver's, not
    // this renderer's.
    let mut overlay = OverlayLayer::default();
    if composing {
        if let Some(comp) = state.compose.as_ref() {
            // `None` when the anchored line has scrolled out of the viewport:
            // there is no rect to sit against, and the resolver docks.
            let target = hits
                .0
                .iter()
                .find(|h| h.index == state.selected)
                .map(|h| h.rect);
            let rect = overlay.place(&compose_anchor(diff_area), target, diff_area);
            frame.render_widget(Clear, rect);
            render_compose(frame, rect, comp);
        }
    }
    let buttons = render_footer(frame, footer, composing, state.side_by_side, state.wrap);

    CodeReviewHits {
        rows: hits.0,
        targets,
        buttons,
        scrollbar: hits.1,
        overlay: overlay.into_hit_order(),
    }
}

/// Render the review-target picker (branch / working / per-commit) into `area`.
///
/// Returns one [`RowHitbox`] per rendered entry (`index` = entry position in
/// `picker.entries`) so a click selects that target, matching the keyboard
/// ↑/↓/Enter path.
fn render_target_picker(frame: &mut Frame, area: Rect, state: &CodeReviewState) -> Vec<RowHitbox> {
    let Some(picker) = state.target_picker.as_ref() else {
        return Vec::new();
    };
    let mut lines: Vec<Line> = vec![Line::from(Span::styled(
        " Review target  (↑/↓ select · Enter · Esc · or click)",
        Style::default().fg(Theme::text_muted()),
    ))];
    let mut hits = Vec::new();
    let height = area.height as usize;
    for (i, target) in picker
        .entries
        .iter()
        .enumerate()
        .take(height.saturating_sub(1))
    {
        let selected = i == picker.selected;
        let marker = if selected { "▸ " } else { "  " };
        let label = target.label(&state.repos, &state.commits);
        let style = if selected {
            Style::default()
                .fg(Theme::selection_fg())
                .bg(Theme::selection_bg())
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Theme::text_primary())
        };
        // Row 0 is the header line; entry `i` renders on the next row down.
        hits.push(RowHitbox {
            rect: Rect::new(area.x, area.y + 1 + i as u16, area.width, 1),
            index: i,
        });
        lines.push(Line::from(Span::styled(
            truncate(&format!("{marker}{label}"), area.width as usize),
            style,
        )));
    }
    frame.render_widget(Paragraph::new(lines), area);
    hits
}

/// Render the windowed diff/comment rows + scrollbar. Returns row hitboxes
/// (index in `state.rows`) and the scrollbar geometry.
///
/// Selection, hitboxes, and the scrollbar are all defined over **logical** rows
/// (`state.rows`): the scrollbar thumb tracks `state.selected` over
/// `total = state.rows.len()`, and its drag mapping (`position_for_y` →
/// `cr_select_row`) feeds a logical index. When `wrap` is on, a logical row
/// expands into several **visual** rows; only the vertical windowing here
/// becomes visual — every visual sub-row carries its parent's logical index, so
/// clicks and the scrollbar stay correct.
fn render_rows(
    frame: &mut Frame,
    area: Rect,
    state: &mut CodeReviewState,
) -> (Vec<RowHitbox>, Option<ScrollbarGeom>) {
    let total = state.rows.len();
    let height = area.height as usize;
    if height == 0 {
        return (Vec::new(), None);
    }

    let (content, track) = scrollbar::reserve_track(area, total, height);
    let width = content.width as usize;

    // Line-number column width from the largest number on screen.
    let num_w = line_number_width(state);

    // Wrap applies to both layouts: a unified line soft-wraps its body, a paired
    // side-by-side row soft-wraps each half independently (the taller half drives
    // the row count). Horizontal scroll stays unified-only — side-by-side always
    // pins `h_scroll` to 0 (below), so the scroll math treats an unwrapped paired
    // row exactly like an unwrapped unified one.
    let wrap = state.wrap;

    // Final horizontal clamp: the app layer bounds `h_scroll` by the longest
    // line, but the exact body width is only known now — never scroll past what
    // the widest line can reveal. Wrapped/side-by-side layouts pin it to 0.
    // Gated on an active scroll: `max_line_width` is an O(total chars) scan,
    // so the default `h_scroll == 0` frame must not pay it on every repaint.
    if state.wrap || state.side_by_side {
        state.h_scroll = 0;
    } else if state.h_scroll > 0 {
        let avail = width.saturating_sub(gutter_width(num_w)).max(1);
        let max_h = state.max_line_width().saturating_sub(avail);
        state.h_scroll = state.h_scroll.min(max_h);
    }

    // Clamp scroll so the selection stays visible (the nav layer set a lower
    // bound; here we enforce the upper edge given the known height). With wrap
    // off, one logical row = one visual row, so the original math holds.
    if !wrap {
        if state.selected >= state.scroll + height {
            state.scroll = state.selected + 1 - height;
        }
        if state.scroll + height > total {
            state.scroll = total.saturating_sub(height);
        }
    }
    if state.selected < state.scroll {
        state.scroll = state.selected;
    }

    // The active search query drives the in-row match highlight — trimmed +
    // lowercased exactly like `search_matches`, so a row the matcher counted
    // never renders without its highlight (e.g. a query with a trailing space).
    let query = state
        .search
        .as_ref()
        .map(|s| s.query.trim().to_lowercase())
        .filter(|q| !q.is_empty());

    if wrap {
        converge_wrap_scroll(state, width, num_w, height);
    }

    // Build the windowed visual lines. When wrapping, the selected logical row's
    // first visual line must stay on screen: expand from `scroll` and, if the
    // selection would fall past `height`, advance `scroll` and retry (bounded by
    // `total`). Each visual line remembers its logical row for the hitboxes.
    let (visual, logical_of): (Vec<Line>, Vec<usize>) = loop {
        let mut lines: Vec<Line> = Vec::with_capacity(height);
        let mut logical: Vec<usize> = Vec::with_capacity(height);
        let mut selected_first: Option<usize> = None;
        for i in state.scroll..total {
            if lines.len() >= height {
                break;
            }
            if i == state.selected {
                selected_first = Some(lines.len());
            }
            for line in row_visual_lines(
                state,
                i,
                width,
                num_w,
                query.as_deref(),
                state.h_scroll,
                wrap,
            ) {
                lines.push(line);
                logical.push(i);
            }
        }
        // Selection off the bottom (only possible when wrapping inflates rows):
        // scroll down one logical row and rebuild.
        let overflows =
            wrap && selected_first.is_none_or(|f| f >= height) && state.scroll < state.selected;
        if overflows {
            state.scroll += 1;
            continue;
        }
        lines.truncate(height);
        logical.truncate(height);
        break (lines, logical);
    };

    frame.render_widget(Paragraph::new(visual), content);

    // The view is selection-primary: the thumb tracks `selected` (which reaches
    // the last row, `total - 1`), not `scroll` (which caps at `total - height`,
    // so a thumb driven by it could never reach the bottom of the track). This
    // also matches the drag mapping — `position_for_y` returns a `0..total`
    // index that `apply_scrollbar_position` feeds straight to `cr_select_row`.
    let geom = track.and_then(|t| scrollbar::render_into(frame, t, total, height, state.selected));

    let hitboxes = logical_of
        .into_iter()
        .enumerate()
        .map(|(row, index)| RowHitbox {
            rect: Rect::new(content.x, content.y + row as u16, content.width, 1),
            index,
        })
        .collect();
    (hitboxes, geom)
}

/// Width for the diff line-number gutter (two numbers, old+new).
fn line_number_width(state: &CodeReviewState) -> usize {
    gutter_number_width(&state.files)
}

/// Width of one of the gutter's two number columns, over a whole review.
///
/// Split out from `line_number_width` because it is also what the published
/// review section carries: it is computed over **every** hunk of **every** file,
/// so a bounded window of lines cannot derive it, and a pane that tried would
/// draw a narrower gutter than this one.
pub fn gutter_number_width(files: &[DiffFile]) -> usize {
    let max = files
        .iter()
        .flat_map(|f| f.hunks.iter())
        .map(|h| h.new_start as usize + h.lines.len())
        .max()
        .unwrap_or(0);
    max.to_string().len().max(2)
}

/// Build the visual sub-rows for logical row `i`. `query` (lowercased,
/// non-empty) is the active find-in-diff search. `h_scroll` slides the body
/// horizontally (unified, non-wrap). With `wrap` on, a long diff line expands
/// into several `Line`s — a unified line wraps its body (continuation rows carry
/// a blank gutter), a paired side-by-side row wraps each half independently
/// (the taller half drives the row count); every other row kind yields one line.
fn row_visual_lines<'a>(
    state: &CodeReviewState,
    i: usize,
    width: usize,
    num_w: usize,
    query: Option<&str>,
    h_scroll: usize,
    wrap: bool,
) -> Vec<Line<'a>> {
    let selected = i == state.selected;
    let sel_style = |base: Style| {
        if selected {
            base.bg(Theme::selection_bg()).fg(Theme::selection_fg())
        } else {
            base
        }
    };

    match &state.rows[i] {
        ReviewRow::FileHeader(fi) => {
            vec![file_header_line(state, *fi, width, query, &sel_style)]
        }
        ReviewRow::HunkHeader(fi, hi) => {
            vec![hunk_header_line(state, *fi, *hi, width, query, &sel_style)]
        }
        ReviewRow::Line(fi, hi, li) => {
            let f = &state.files[*fi];
            let hunk = &f.hunks[*hi];
            if state.side_by_side {
                paired_diff_line(hunk, *li, width, num_w, wrap, &sel_style)
            } else if wrap {
                let l = &hunk.lines[*li];
                unified_diff_line_wrapped(f, l, width, num_w, selected, query, &sel_style)
            } else {
                let l = &hunk.lines[*li];
                vec![unified_diff_line(
                    f, l, width, num_w, selected, h_scroll, query, &sel_style,
                )]
            }
        }
        ReviewRow::Comment(id) | ReviewRow::Summary(id) => {
            vec![comment_line(state, *id, width, query, sel_style)]
        }
        ReviewRow::SummaryHeader => vec![Line::from(Span::styled(
            truncate(crate::app::code_review::SUMMARY_HEADER_LABEL, width),
            sel_style(
                Style::default()
                    .fg(Theme::accent())
                    .add_modifier(Modifier::BOLD),
            ),
        ))],
        ReviewRow::Info(text) => vec![Line::from(Span::styled(
            truncate(text, width),
            Style::default().fg(Theme::text_muted()),
        ))],
    }
}

/// A file header: a full-width rule with the path embedded (tuicr-style file
/// separator), a fold chevron, and the status glyph + add/remove counts tinted
/// with the diff colors so the status reads at a glance.
fn file_header_line<'a>(
    state: &CodeReviewState,
    fi: usize,
    width: usize,
    query: Option<&str>,
    sel_style: &impl Fn(Style) -> Style,
) -> Line<'a> {
    let f = &state.files[fi];
    let chevron = if state.is_file_folded(&f.path) {
        "▸"
    } else {
        "▾"
    };
    let header = Style::default()
        .fg(Theme::accent_bright())
        .add_modifier(Modifier::BOLD);
    let lead = format!("{chevron} ");
    let glyph = f.status.glyph().to_string();
    let mid = format!(" {}  ", f.path);
    let adds = format!("+{}", f.added_count());
    let dels = format!(" -{}", f.deleted_count());
    let mark = if state.reviewed_files.contains(&f.path) {
        " ✓"
    } else {
        ""
    };
    let used = lead.chars().count()
        + glyph.chars().count()
        + mid.chars().count()
        + adds.chars().count()
        + dels.chars().count()
        + mark.chars().count()
        + 1; // trailing space before the rule
    let pad = if used < width {
        "─".repeat(width - used)
    } else {
        String::new()
    };
    let mut spans = vec![
        Span::styled(lead, sel_style(header)),
        Span::styled(
            glyph,
            sel_style(
                Style::default()
                    .fg(status_color(f.status))
                    .add_modifier(Modifier::BOLD),
            ),
        ),
    ];
    spans.extend(highlight_text(mid, sel_style(header), query));
    spans.extend([
        Span::styled(adds, sel_style(Style::default().fg(Theme::diff_added()))),
        Span::styled(dels, sel_style(Style::default().fg(Theme::diff_removed()))),
    ]);
    if !mark.is_empty() {
        spans.push(Span::styled(
            mark.to_string(),
            sel_style(Style::default().fg(Theme::status_done())),
        ));
    }
    spans.push(Span::styled(format!(" {pad}"), sel_style(header)));
    Line::from(spans)
}

/// A hunk header: the `@@ -a,b +c,d @@` ranges (recomputed from the lines) + the
/// section heading, with a `✓` when the hunk is marked reviewed.
fn hunk_header_line<'a>(
    state: &CodeReviewState,
    fi: usize,
    hi: usize,
    width: usize,
    query: Option<&str>,
    sel_style: &impl Fn(Style) -> Style,
) -> Line<'a> {
    let f = &state.files[fi];
    let h = &f.hunks[hi];
    let mark = if state.reviewed_hunks.contains(&(f.path.clone(), hi)) {
        " ✓"
    } else {
        ""
    };
    // Old-side span = lines present on the old side (context + deletions);
    // new-side span = lines present on the new side (context + additions).
    let old_span = h.lines.iter().filter(|l| l.old_no.is_some()).count();
    let new_span = h.lines.iter().filter(|l| l.new_no.is_some()).count();
    let text = format!(
        "  @@ -{},{} +{},{} @@ {}{}",
        h.old_start, old_span, h.new_start, new_span, h.header, mark
    );
    let base = sel_style(
        Style::default()
            .fg(Theme::accent())
            .add_modifier(Modifier::DIM),
    );
    Line::from(highlight_text(truncate(&text, width), base, query))
}

/// A unified-diff line: a `old new ±` gutter plus the syntax-highlighted body,
/// with the add/remove row tint (the gutter sign + tint carry the +/-, leaving
/// the text free for syntax colour). The gutter stays pinned; the body is
/// windowed to `[h_scroll, h_scroll + avail)` (horizontal scroll) and padded to
/// `width`.
#[allow(clippy::too_many_arguments)]
fn unified_diff_line<'a>(
    f: &DiffFile,
    l: &DiffLine,
    width: usize,
    num_w: usize,
    selected: bool,
    h_scroll: usize,
    query: Option<&str>,
    sel_style: &impl Fn(Style) -> Style,
) -> Line<'a> {
    let (sign, row_bg) = diff_row_bg(l.kind);
    let bg = row_bg_fn(row_bg, selected);
    let gutter = diff_gutter(l, num_w, sign);
    let avail = width.saturating_sub(gutter_width(num_w));

    let mut spans = vec![Span::styled(
        gutter,
        sel_style(bg(Style::default().fg(Theme::text_muted()))),
    )];
    spans.extend(diff_body_spans(
        f, &l.text, h_scroll, avail, query, sel_style, &bg,
    ));
    Line::from(spans)
}

/// A unified-diff line soft-wrapped onto as many rows as its body needs. The
/// first row carries the real gutter; continuation rows carry a blank
/// gutter-width prefix so the body stays left-aligned. The row tint + selection
/// highlight cover every wrapped row (so a selected wrapped line reads as one
/// block). Empty bodies still emit one row (matching the non-wrap path).
fn unified_diff_line_wrapped<'a>(
    f: &DiffFile,
    l: &DiffLine,
    width: usize,
    num_w: usize,
    selected: bool,
    query: Option<&str>,
    sel_style: &impl Fn(Style) -> Style,
) -> Vec<Line<'a>> {
    let (sign, row_bg) = diff_row_bg(l.kind);
    let bg = row_bg_fn(row_bg, selected);
    let gutter = diff_gutter(l, num_w, sign);
    let gutter_w = gutter_width(num_w);
    let avail = width.saturating_sub(gutter_w).max(1);

    let body_len = l.text.chars().count();
    let chunks = body_len.div_ceil(avail).max(1);
    let mut lines = Vec::with_capacity(chunks);
    for c in 0..chunks {
        let prefix = if c == 0 {
            Span::styled(
                gutter.clone(),
                sel_style(bg(Style::default().fg(Theme::text_muted()))),
            )
        } else {
            // Blank gutter so continuation text lines up under the first row.
            Span::styled(" ".repeat(gutter_w), sel_style(bg(Style::default())))
        };
        let mut spans = vec![prefix];
        spans.extend(diff_body_spans(
            f,
            &l.text,
            c * avail,
            avail,
            query,
            sel_style,
            &bg,
        ));
        lines.push(Line::from(spans));
    }
    lines
}

/// The `('sign', row-tint role)` for a diff line kind.
///
/// The single encoding of "which side of the change is this row on", shared by
/// the native span renderer and [`diff_row_tree`] so the two cannot disagree
/// about a sign or a tint.
fn diff_row_sign_tint(kind: DiffLineKind) -> (char, Option<DiffTint>) {
    match kind {
        DiffLineKind::Add => ('+', Some(DiffTint::Added)),
        DiffLineKind::Del => ('-', Some(DiffTint::Removed)),
        DiffLineKind::Context => (' ', None),
    }
}

/// The `('sign', row-tint)` for a diff line kind, resolved against the theme.
fn diff_row_bg(kind: DiffLineKind) -> (char, Option<Color>) {
    let (sign, tint) = diff_row_sign_tint(kind);
    let color = tint.map(|t| match t {
        DiffTint::Added => Theme::diff_added_bg(),
        DiffTint::Removed => Theme::diff_removed_bg(),
    });
    (sign, color)
}

/// One unified diff line as a **geometry-free** view tree row: the line-number
/// gutter, one run per syntax token, and a fill that carries the row tint to the
/// pane's right edge.
///
/// This is the form a plugin pane can produce, and the reason it exists is that
/// the pane's own renderer cannot be it. `unified_diff_line` windows the body
/// to `[h_scroll, h_scroll + avail)` and slices that window by **character
/// count** against a resolved width; a view tree carries no width, so a tree can
/// hold only the whole body and let the renderer clip it. The two are pinned
/// together by painting both and comparing the frames (see this module's tests),
/// which is what makes the bundled `code-review` plugin's equality mean something
/// rather than two functions agreeing about a format neither is obliged to match.
///
/// The row declares its tint **and** its selection when the cursor is on it: the
/// renderer resolves selection first, which is the precedence this module's own
/// `row_bg_fn` applies.
pub fn diff_row_tree(line: &pc::ReviewLineSnapshot, num_w: usize, selected: bool) -> ViewNode {
    let (sign, tint) = diff_row_sign_tint(DiffLineKind::parse(line.kind));
    let style = row_style(selected, tint);
    let mut runs = vec![ViewNode::styled(
        gutter_text(line.old_no, line.new_no, num_w, sign),
        style(Some(Token::Muted)),
    )];
    let lang = crate::ui::syntax::lang_for(&line.path);
    for (run, token) in crate::ui::syntax::highlight_tokens(&line.text, &lang) {
        runs.push(ViewNode::styled(run, style(token)));
    }
    // The tint's whole point is that it reaches the edge: a background that
    // stopped where the code stopped would not be the row this pane draws.
    runs.push(ViewNode::fill(' ', style(None)));
    ViewNode::Line(runs)
}

/// A run style for one row: a token, plus the row's selection and tint.
///
/// Both roles go on **every** run of a row because the renderer resolves
/// selection first — the cursor's row draws in one appearance rather than a
/// selection background beside a tint, which is the precedence [`row_bg_fn`]
/// applies natively.
fn row_style(selected: bool, tint: Option<DiffTint>) -> impl Fn(Option<Token>) -> TextStyle {
    move |token| TextStyle {
        token,
        bold: false,
        dim: false,
        underline: false,
        selected,
        tint,
        // A diff body is never fitted: the review scrolls horizontally or wraps
        // instead, so no run of it yields its width.
        ellipsize: false,
    }
}

/// A file header as a view tree: the chevron, the status glyph, the path, the
/// two counts, the reviewed mark, and the rule that carries the row to the edge.
///
/// The rule is a [`ViewNode::Fill`], which is the whole reason this row is
/// expressible: the native renderer repeats `─` for `width - used` columns, and a
/// fill is the kernel resolving that residue for a pane that was never told its
/// width. The trailing space is a run of its own so both paths spend the same
/// column on it — otherwise the fill would be one character long.
pub fn file_header_tree(f: &pc::ReviewFileSnapshot, selected: bool) -> ViewNode {
    let style = row_style(selected, None);
    let header = |token: Option<Token>| TextStyle {
        bold: true,
        ..style(token)
    };
    let chevron = if f.folded { "▸" } else { "▾" };
    let status = match crate::session::review::FileStatus::parse(f.status) {
        Some(crate::session::review::FileStatus::Added) => Token::DiffAdded,
        Some(crate::session::review::FileStatus::Deleted) => Token::DiffRemoved,
        Some(crate::session::review::FileStatus::Renamed) => Token::Accent,
        // Modified, and the residual for a name this build does not know: the
        // native pane's own default for its status colour.
        _ => Token::StatusWorking,
    };
    let glyph = crate::session::review::FileStatus::parse(f.status)
        .map(|s| s.glyph())
        .unwrap_or("?");
    let mut runs = vec![
        ViewNode::styled(format!("{chevron} "), header(Some(Token::AccentBright))),
        ViewNode::styled(glyph.to_string(), header(Some(status))),
        ViewNode::styled(format!(" {}  ", f.path), header(Some(Token::AccentBright))),
        ViewNode::styled(format!("+{}", f.added), style(Some(Token::DiffAdded))),
        ViewNode::styled(format!(" -{}", f.removed), style(Some(Token::DiffRemoved))),
    ];
    if f.reviewed {
        runs.push(ViewNode::styled(" ✓", style(Some(Token::StatusDone))));
    }
    runs.push(ViewNode::styled(" ", header(Some(Token::AccentBright))));
    runs.push(ViewNode::fill('─', header(Some(Token::AccentBright))));
    ViewNode::Line(runs)
}

/// A hunk header as a view tree: the `@@` ranges, the heading, and the mark.
pub fn hunk_header_tree(h: &pc::ReviewHunkSnapshot, selected: bool) -> ViewNode {
    let mark = if h.reviewed { " ✓" } else { "" };
    let text = format!(
        "  @@ -{},{} +{},{} @@ {}{}",
        h.old_start, h.old_span, h.new_start, h.new_span, h.heading, mark
    );
    ViewNode::styled(
        text,
        TextStyle {
            dim: true,
            ..row_style(selected, None)(Some(Token::Accent))
        },
    )
}

/// A comment as a view tree: its classification badge, then its first line.
///
/// Unlike a diff row this ends without a fill, because the native row does not
/// pad either: the columns past the body keep the terminal's default style.
pub fn comment_tree(c: &pc::ReviewCommentSnapshot, selected: bool) -> ViewNode {
    let style = row_style(selected, None);
    let class = crate::session::review::Classification::parse(c.classification);
    let token = match class {
        Some(Classification::Issue) => Token::Danger,
        Some(Classification::Suggestion) => Token::Accent,
        Some(Classification::Praise) => Token::StatusDone,
        // Note, and the residual for an unknown name.
        _ => Token::Secondary,
    };
    let label = class.map(|c| c.label()).unwrap_or("Note");
    let more = if c.more { " …" } else { "" };
    ViewNode::Line(vec![
        ViewNode::styled(
            format!("  ▸ [{label}] "),
            TextStyle {
                bold: true,
                ..style(Some(token))
            },
        ),
        ViewNode::styled(format!("{}{more}", c.text), style(Some(Token::Secondary))),
    ])
}

/// The review-summary heading as a view tree.
///
/// The label is the kernel's rather than composed here, because it names a
/// keystroke — see
/// [`ReviewRowSnapshot::SummaryHeader`](crate::session::pane_context::ReviewRowSnapshot::SummaryHeader).
pub fn summary_header_tree(label: &str, selected: bool) -> ViewNode {
    ViewNode::styled(
        label.to_string(),
        TextStyle {
            bold: true,
            ..row_style(selected, None)(Some(Token::Accent))
        },
    )
}

/// An informational row as a view tree.
///
/// Never drawn selected: the native row applies no selection style because such a
/// row is not selectable, so a cursor cannot rest on one.
pub fn info_tree(text: &str) -> ViewNode {
    ViewNode::token(text, Token::Muted)
}

/// The review's row stream as a view tree: one row per published row, with the
/// row the cursor is on named so the **kernel** windows the list.
///
/// Reproduces the pane's whole *document* — every row kind this module's own
/// `row_visual_lines` draws — and none of its behaviour. What is left out, and
/// why each entry needs a host power or a resolved width, is
/// `docs/PHASE4-PANE-READINESS.md` §11.
///
/// One divergence from the native rows, enumerated rather than hidden: a hunk
/// header, a comment, an informational row and the summary header are truncated
/// natively to the pane's width with a trailing `…`, while a tree carries the
/// whole text and the renderer clips it. That is the same absent resolved width
/// that blocks wrap, horizontal scroll and the side-by-side layout.
pub fn review_stream_tree(
    rows: &[pc::ReviewRowSnapshot],
    cursor: Option<usize>,
    num_w: usize,
) -> ViewNode {
    if rows.is_empty() {
        // A review that has not been opened publishes nothing, and the native
        // pane's own empty state is an `Info` row with this text — so the pane a
        // plugin draws before any publication is the one the review draws for a
        // target with no changes.
        return ViewNode::list(vec![info_tree("No changes to show for this target.")]);
    }
    let children = rows
        .iter()
        .enumerate()
        .map(|(i, row)| {
            let selected = Some(i) == cursor;
            match row {
                pc::ReviewRowSnapshot::File(f) => file_header_tree(f, selected),
                pc::ReviewRowSnapshot::Hunk(h) => hunk_header_tree(h, selected),
                pc::ReviewRowSnapshot::Line(l) => diff_row_tree(l, num_w, selected),
                pc::ReviewRowSnapshot::Comment(c) => comment_tree(c, selected),
                pc::ReviewRowSnapshot::SummaryHeader { label } => {
                    summary_header_tree(label, selected)
                }
                pc::ReviewRowSnapshot::Info { text } => info_tree(text),
            }
        })
        .collect();
    ViewNode::selectable_list(children, cursor)
}

/// A closure that applies the row tint under a style — unless the row is
/// selected, where the selection background wins.
fn row_bg_fn(row_bg: Option<Color>, selected: bool) -> impl Fn(Style) -> Style {
    move |s: Style| match row_bg {
        Some(c) if !selected => s.bg(c),
        _ => s,
    }
}

/// The `{old} {new} {sign} ` line-number gutter for a unified diff line.
fn diff_gutter(l: &DiffLine, num_w: usize, sign: char) -> String {
    gutter_text(l.old_no, l.new_no, num_w, sign)
}

/// The gutter for a pair of line numbers, either of which may be absent.
///
/// Split from [`diff_gutter`] so the native renderer (which holds a [`DiffLine`])
/// and the tree builder (which holds a published row) format one gutter rather
/// than two that must agree. An absent number is an empty column, not a zero: a
/// deletion has no new-side number.
fn gutter_text(old_no: Option<u32>, new_no: Option<u32>, num_w: usize, sign: char) -> String {
    let old = old_no.map(|n| n.to_string()).unwrap_or_default();
    let new = new_no.map(|n| n.to_string()).unwrap_or_default();
    format!("{old:>num_w$} {new:>num_w$} {sign} ")
}

/// Cell width of [`diff_gutter`]'s output (two right-aligned numbers + three
/// single-space/sign separators). The single encoding of the gutter layout —
/// every consumer (h-scroll clamp, wrap chunking, continuation padding) derives
/// from it, so a format change can't desync them from what is painted.
fn gutter_width(num_w: usize) -> usize {
    num_w * 2 + 4
}

/// With wrap on, a far jump (`G`, a scrollbar drag, a search step) can leave
/// `scroll` many rows above the selection. Converge directly with a backward
/// walk over visual-row counts — O(height) count-only passes — instead of
/// letting `render_rows`' build loop advance one row per full-window rebuild
/// (quadratic in the jump distance, a visible stall on a large diff). Lands
/// on the smallest scroll ≥ the current one that keeps the selection's first
/// visual row within `height`.
fn converge_wrap_scroll(state: &mut CodeReviewState, width: usize, num_w: usize, height: usize) {
    if !state.wrap || state.selected <= state.scroll {
        return;
    }
    // Rows above the selection may fill at most height-1 visual rows, leaving
    // one for the selection's first line (saturating: height 0 → selection).
    let budget = height.saturating_sub(1);
    let mut acc = 0usize;
    let mut first = state.selected;
    while first > state.scroll {
        let c = visual_line_count(state, first - 1, width, num_w);
        if acc + c > budget {
            break;
        }
        acc += c;
        first -= 1;
    }
    state.scroll = first;
}

/// How many visual rows logical row `i` occupies — the count-only mirror of
/// [`row_visual_lines`] (wrap inflates only diff lines; every other row kind is
/// exactly one). Must stay in lockstep with [`unified_diff_line_wrapped`]'s and
/// [`paired_diff_line`]'s chunking so the wrap scroll walk in `render_rows`
/// lands exactly where the build does.
fn visual_line_count(state: &CodeReviewState, i: usize, width: usize, num_w: usize) -> usize {
    if !state.wrap {
        return 1;
    }
    match &state.rows[i] {
        ReviewRow::Line(fi, hi, li) => {
            let hunk = &state.files[*fi].hunks[*hi];
            if state.side_by_side {
                paired_visual_count(hunk, *li, width, num_w)
            } else {
                let l = &hunk.lines[*li];
                let avail = width.saturating_sub(gutter_width(num_w)).max(1);
                l.text.chars().count().div_ceil(avail).max(1)
            }
        }
        _ => 1,
    }
}

/// Visual-row count of a wrapped paired side-by-side row — the count-only mirror
/// of [`paired_diff_line`]'s chunking (each half wraps independently; the taller
/// half drives the row count). Kept next to it so the two can't drift.
fn paired_visual_count(hunk: &DiffHunk, li: usize, width: usize, num_w: usize) -> usize {
    let pair = paired_row(hunk, li);
    let body_w = paired_body_width(width, num_w);
    let lc = pair.old.map_or(0, |i| {
        hunk.lines[i].text.chars().count().div_ceil(body_w).max(1)
    });
    let rc = pair.new.map_or(0, |i| {
        hunk.lines[i].text.chars().count().div_ceil(body_w).max(1)
    });
    lc.max(rc).max(1)
}

/// Styled spans for a diff body windowed to `[start, start + avail)` chars,
/// padded to `avail`. When the active search query hits the visible window the
/// literal matches are highlighted over plain text (search clarity wins);
/// otherwise the syntax-highlighted token stream is sliced to the window (the
/// full line is tokenized for correctness, then windowed). Shared by the
/// horizontal-scroll and wrap paths.
fn diff_body_spans<'a>(
    f: &DiffFile,
    text: &str,
    start: usize,
    avail: usize,
    query: Option<&str>,
    sel_style: &impl Fn(Style) -> Style,
    bg: &impl Fn(Style) -> Style,
) -> Vec<Span<'a>> {
    let windowed: String = text.chars().skip(start).take(avail).collect();
    let positions = query
        .map(|q| match_positions(&windowed, q))
        .unwrap_or_default();

    let mut spans = Vec::new();
    let mut used;
    if !positions.is_empty() {
        used = windowed.chars().count();
        let base = sel_style(bg(Style::default().fg(Theme::text_primary())));
        spans.extend(crate::ui::highlight::highlighted_spans_owned(
            &windowed, &positions, base,
        ));
    } else {
        let lang = crate::ui::syntax::lang_for(&f.path);
        // Walk the full token stream, tracking the running char position so each
        // token can be sliced to the visible window `[start, start + avail)`.
        used = 0;
        let mut pos = 0usize;
        for (tok, tcolor) in crate::ui::syntax::highlight(text, &lang) {
            let tok_len = tok.chars().count();
            let tok_start = pos;
            pos += tok_len;
            if used >= avail {
                break;
            }
            // Intersect this token's char span with the visible window.
            if pos <= start {
                continue; // entirely left of the window
            }
            let skip = start.saturating_sub(tok_start);
            let piece: String = tok.chars().skip(skip).take(avail - used).collect();
            if piece.is_empty() {
                continue;
            }
            used += piece.chars().count();
            spans.push(Span::styled(
                piece,
                sel_style(bg(Style::default().fg(tcolor))),
            ));
        }
    }
    // Pad so the row tint fills the available width.
    if used < avail {
        spans.push(Span::styled(
            " ".repeat(avail - used),
            sel_style(bg(Style::default())),
        ));
    }
    spans
}

fn comment_line<'a>(
    state: &CodeReviewState,
    id: i64,
    width: usize,
    query: Option<&str>,
    sel_style: impl Fn(Style) -> Style,
) -> Line<'a> {
    let Some(c) = state.comment(id) else {
        return Line::from("");
    };
    let badge = format!("  ▸ [{}] ", c.classification.label());
    let first = c.body.lines().next().unwrap_or("");
    let more = if c.body.lines().count() > 1 {
        " …"
    } else {
        ""
    };
    let body = truncate(
        &format!("{first}{more}"),
        width.saturating_sub(badge.chars().count()),
    );
    let mut spans = vec![Span::styled(
        badge,
        sel_style(
            Style::default()
                .fg(class_color(c.classification))
                .add_modifier(Modifier::BOLD),
        ),
    )];
    spans.extend(highlight_text(
        body,
        sel_style(Style::default().fg(Theme::text_secondary())),
        query,
    ));
    Line::from(spans)
}

/// Re-derive the [`SidePair`] a paired row stands for from the same pure pairing
/// the row builder used, so renderer and builder never disagree on which lines
/// share the row. `li` is the row's representative line index.
fn paired_row(hunk: &DiffHunk, li: usize) -> SidePair {
    pair_hunk(hunk)
        .into_iter()
        .find(|p| p.old == Some(li) || p.new == Some(li))
        .unwrap_or(SidePair {
            old: Some(li),
            new: None,
        })
}

/// Body (text) width of one side-by-side half cell: the half width minus the
/// right-aligned line-number column and its trailing space (mirrors
/// [`half_cell_chunk`]'s framing).
fn paired_body_width(width: usize, num_w: usize) -> usize {
    let half = width.saturating_sub(1) / 2;
    half.saturating_sub(num_w + 1).max(1)
}

/// Render one paired side-by-side row: the old-side cell `│` the new-side cell.
/// True GitHub-style pairing — a deletion (left) and its aligned addition
/// (right) sit on the *same* screen row (see
/// [`crate::session::review::pair_hunk`]). `li` is the row's representative line
/// index; the [`SidePair`] it belongs to supplies both sides (a blank half-cell
/// where a side is absent). The selection + comment anchor stay 1 row = 1
/// selectable unit; which side a comment attaches to is resolved at compose
/// time. Plain add/remove tinting (no syntax highlighting), matching the
/// unified body's gutter-sign convention.
///
/// With `wrap` on, each half soft-wraps independently onto as many chunks as its
/// text needs; the taller half drives the visual-row count (mirrored by
/// [`paired_visual_count`]), and the shorter half pads with blank cells past its
/// last chunk. Off, each half truncates to one row (the historical behavior).
fn paired_diff_line<'a>(
    hunk: &DiffHunk,
    li: usize,
    width: usize,
    num_w: usize,
    wrap: bool,
    sel_style: &impl Fn(Style) -> Style,
) -> Vec<Line<'a>> {
    let pair = paired_row(hunk, li);
    let half = width.saturating_sub(1) / 2;
    let body_w = paired_body_width(width, num_w);
    let prim = || Style::default().fg(Theme::text_primary());
    let removed = || {
        Style::default()
            .fg(Theme::diff_removed())
            .bg(Theme::diff_removed_bg())
    };
    let added = || {
        Style::default()
            .fg(Theme::diff_added())
            .bg(Theme::diff_added_bg())
    };

    let left = pair.old.map(|i| &hunk.lines[i]);
    let right = pair.new.map(|i| &hunk.lines[i]);
    // Each cell tints only when it carries a change (a context line pairs with
    // itself and stays plain on both sides); sel_style overrides bg on select.
    let lstyle = match left {
        Some(l) if l.kind == DiffLineKind::Del => removed(),
        _ => prim(),
    };
    let rstyle = match right {
        Some(l) if l.kind == DiffLineKind::Add => added(),
        _ => prim(),
    };

    // How many chunks each present half needs (an absent half contributes none);
    // the taller drives the row count. Must match `paired_visual_count`.
    let chunks = |line: Option<&DiffLine>| -> usize {
        match line {
            Some(l) if wrap => l.text.chars().count().div_ceil(body_w).max(1),
            Some(_) => 1,
            None => 0,
        }
    };
    let lchunks = chunks(left);
    let rchunks = chunks(right);
    let rows = lchunks.max(rchunks).max(1);

    (0..rows)
        .map(|c| {
            // A half renders its `c`-th chunk while it still has one; past that
            // (the shorter side, or an absent side) it pads blank + plain.
            let (left_cell, ls) = if c < lchunks {
                let l = left.expect("chunk count > 0 implies present");
                (half_cell_chunk(l.old_no, &l.text, c, num_w, half), lstyle)
            } else {
                (half_cell_chunk(None, "", 0, num_w, half), prim())
            };
            let (right_cell, rs) = if c < rchunks {
                let r = right.expect("chunk count > 0 implies present");
                (half_cell_chunk(r.new_no, &r.text, c, num_w, half), rstyle)
            } else {
                (half_cell_chunk(None, "", 0, num_w, half), prim())
            };
            Line::from(vec![
                Span::styled(left_cell, sel_style(ls)),
                Span::styled("│", sel_style(Style::default().fg(Theme::text_muted()))),
                Span::styled(right_cell, sel_style(rs)),
            ])
        })
        .collect()
}

/// A fixed-width side-by-side half cell for wrap chunk `c`: the right-aligned
/// line number (only on `c == 0`; blank on continuation rows) then the `c`-th
/// `body_w`-wide slice of `text`, padded or truncated to exactly `cell_w`
/// columns so the center separator stays aligned. With `c == 0` and a
/// single-chunk text this reproduces the unwrapped cell exactly.
fn half_cell_chunk(num: Option<u32>, text: &str, c: usize, num_w: usize, cell_w: usize) -> String {
    let body_w = cell_w.saturating_sub(num_w + 1).max(1);
    let n = if c == 0 {
        num.map(|n| n.to_string()).unwrap_or_default()
    } else {
        String::new()
    };
    let slice: String = text.chars().skip(c * body_w).take(body_w).collect();
    let raw = format!("{n:>num_w$} {slice}");
    let len = raw.chars().count();
    if len > cell_w {
        raw.chars().take(cell_w).collect()
    } else {
        let mut s = raw;
        s.push_str(&" ".repeat(cell_w - len));
        s
    }
}

/// Render the changed-files list shown in the file-viewer column during a review
/// (the navigation aid; clicking a row jumps the diff to that file). Returns one
/// [`RowHitbox`] per visible file (index = diff-file index).
pub(crate) fn render_files_list(
    frame: &mut Frame,
    area: Rect,
    state: &CodeReviewState,
    level: FocusLevel,
) -> Vec<RowHitbox> {
    let block = focus_block(" Changed files ", level);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if inner.height == 0 || inner.width == 0 || state.files.is_empty() {
        return Vec::new();
    }
    // Reserve the last row for a compact nav-key legend (the keys that aren't
    // footer buttons), so all shortcuts stay discoverable.
    let hint_row = Rect::new(inner.x, inner.y + inner.height - 1, inner.width, 1);
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            truncate(" ↑↓ move · ↵ open · r seen · / find", inner.width as usize),
            Style::default().fg(Theme::text_muted()),
        ))),
        hint_row,
    );
    let list = Rect::new(inner.x, inner.y, inner.width, inner.height - 1);
    if list.height == 0 {
        return Vec::new();
    }

    // Unbounded, because this pane holds the whole review: the publication's cap
    // is a property of the wire, not of the list.
    let (rows, cursor) = state.file_row_snapshots(usize::MAX);
    let tree = files_list_tree(&rows, cursor);
    let palette = crate::ui::theme::current();
    // Painted through the renderer a plugin pane's tree goes through, so the
    // window, the row rects and every cell inside a row are resolved once —
    // here and in whatever else draws this list (ADR-63's move, applied to the
    // review's second pane).
    let painted = crate::ui::plugin_pane::render_tree_rows(
        &tree,
        list,
        &palette,
        &FrameTable::default(),
        frame.buffer_mut(),
    );
    painted
        .rows
        .into_iter()
        .filter_map(|hit| {
            // The painter numbers rows from one, in list space. A folder header
            // is a row of the list and not a file, so it keeps its place in that
            // numbering and yields no hitbox — clicking a directory jumps
            // nowhere, exactly as it did when the rows were spans.
            match rows.get(hit.index - 1)? {
                pc::ReviewFileRowSnapshot::File { path, .. } => {
                    // Back to the index the review knows the file by: the tree is
                    // sorted, so a row's position in it is not the file's.
                    let index = state.files.iter().position(|f| &f.path == path)?;
                    Some(RowHitbox {
                        rect: hit.rect,
                        index,
                    })
                }
                pc::ReviewFileRowSnapshot::Folder { .. } => None,
            }
        })
        .collect()
}

/// The changed-files list as a view tree: one child per tree row, with the row
/// the diff's cursor is in named so the **kernel** windows the list.
///
/// The cursor is a *file*, not a row of this list: `current_file` is the file the
/// diff's own cursor sits in, and is `None` on the summary section — where this
/// list highlights nothing and the window opens at the top, which is what naming
/// the first row as the anchor does.
pub fn files_list_tree(rows: &[pc::ReviewFileRowSnapshot], cursor: Option<usize>) -> ViewNode {
    let children = rows
        .iter()
        .enumerate()
        .map(|(i, row)| match row {
            pc::ReviewFileRowSnapshot::Folder { depth, name } => folder_row_tree(*depth, name),
            pc::ReviewFileRowSnapshot::File {
                depth,
                path,
                status,
                added,
                removed,
                reviewed,
            } => file_row_tree(
                FileRow {
                    depth: *depth,
                    path,
                    status,
                    added: *added,
                    removed: *removed,
                    reviewed: *reviewed,
                },
                Some(i) == cursor,
            ),
        })
        .collect();
    ViewNode::List {
        children,
        // The window opens at the top when the diff's cursor is on a row belonging
        // to no file, which is what the native pane did with no file to anchor on.
        // An empty list has no cursor at all — naming a row of a list with none
        // would be an index into nothing.
        selected: (!rows.is_empty()).then(|| cursor.unwrap_or(0)),
        scrollbar: false,
    }
}

/// One published changed file, as the row builder needs it.
///
/// A struct rather than six arguments, which is the shape `clippy::too_many_arguments`
/// asks for and the shape the snapshot already has.
struct FileRow<'a> {
    depth: usize,
    path: &'a str,
    status: &'static str,
    added: usize,
    removed: usize,
    reviewed: bool,
}

/// A directory header row in the changed-files tree.
///
/// The indent travels inside the yielding run because the native row truncated
/// the indented string as one — so a deeply nested directory loses its name's
/// tail rather than its indentation.
fn folder_row_tree(depth: usize, name: &str) -> ViewNode {
    ViewNode::styled(
        format!("{}{name}/", "  ".repeat(depth)),
        TextStyle {
            token: Some(Token::Muted),
            bold: true,
            ellipsize: true,
            ..TextStyle::default()
        },
    )
}

/// A file row in the changed-files tree: indented name + colored status glyph
/// and `+`/`-` counts (the selection appearance owns the whole row on the file
/// the diff's cursor is in).
fn file_row_tree(f: FileRow<'_>, current: bool) -> ViewNode {
    let mark = if f.reviewed { "✓ " } else { "  " };
    let name = f.path.rsplit('/').next().unwrap_or(f.path);
    let base = TextStyle {
        bold: current,
        selected: current,
        ..TextStyle::default()
    };
    // Tint the glyph + counts with diff colours, except on the selected row
    // where the selection appearance owns the line.
    let tint = |token: Token| TextStyle {
        token: (!current).then_some(token),
        ..base
    };
    let status = crate::session::review::FileStatus::parse(f.status);
    let glyph = status.map(|s| s.glyph()).unwrap_or("?");
    let token = status.map(status_token).unwrap_or(Token::StatusWorking);
    ViewNode::Line(vec![
        ViewNode::styled(format!("{}{mark}", "  ".repeat(f.depth)), base),
        ViewNode::styled(glyph, tint(token)),
        ViewNode::styled(format!(" {name}  "), base),
        ViewNode::styled(format!("+{}", f.added), tint(Token::DiffAdded)),
        ViewNode::styled(format!(" -{}", f.removed), tint(Token::DiffRemoved)),
    ])
}

/// The compose box's anchor declaration: a full-width band attached to the
/// selected line.
///
/// Where it actually lands — below the line, flipped above it, or docked at the
/// diff area's bottom edge because neither fits or the line is off-screen — is
/// [`Overlay::place`]'s answer, shared with every other floating surface.
fn compose_anchor(diff_area: Rect) -> Overlay {
    Overlay {
        // The comment belongs to the line above it, the way a GitHub review
        // comment does, and flips over the line when the pane's bottom is too
        // close to fit it underneath.
        side: crate::session::overlay::Side::Below,
        flip: true,
        // A border, the header line, and at least one body row; capped so a
        // comment box never swallows the diff it is about.
        main: diff_area.height.clamp(3, 6),
        // Indented one column so the box reads as attached to the line above it.
        cross: CrossExtent::Stretch { inset: 1 },
        // Unused by a stretch, which spans the pane rather than aligning to the
        // row.
        align: Align::Start,
    }
}

/// Render the in-view comment compose box.
fn render_compose(frame: &mut Frame, area: Rect, comp: &ComposeState) {
    if area.height == 0 {
        return;
    }
    let target = match &comp.anchor {
        CommentAnchor::Line { side, line, .. } => format!("line {}:{}", side.as_str(), line),
        CommentAnchor::File { file } => format!("file {file}"),
        CommentAnchor::Review => "review summary".to_string(),
    };
    let mut lines: Vec<Line> = Vec::new();
    lines.push(Line::from(vec![
        Span::styled(
            format!(" Comment on {target}  "),
            Style::default().fg(Theme::text_secondary()),
        ),
        Span::styled(
            format!("[{}]", comp.classification.label()),
            Style::default()
                .fg(class_color(comp.classification))
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            "  Tab: cycle type · Ctrl+S: save · Esc: cancel",
            Style::default().fg(Theme::text_muted()),
        ),
    ]));
    // Body with a visible cursor.
    let (cur_line, cur_col) = comp.body.cursor_line_col();
    let body = comp.body.value();
    let body_lines: Vec<&str> = if body.is_empty() {
        vec![""]
    } else {
        body.split('\n').collect()
    };
    for (li, text) in body_lines.iter().enumerate() {
        if li == cur_line {
            lines.push(cursor_line(text, cur_col));
        } else {
            lines.push(Line::from(Span::styled(
                text.to_string(),
                Style::default().fg(Theme::text_primary()),
            )));
        }
    }
    let block = crate::ui::focus_block(" Compose ", FocusLevel::Focused);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    frame.render_widget(Paragraph::new(lines), inner);
}

/// A compose body line with a reversed cell at the cursor column.
fn cursor_line<'a>(text: &str, col: usize) -> Line<'a> {
    let chars: Vec<char> = text.chars().collect();
    let before: String = chars.iter().take(col).collect();
    let at = chars.get(col).copied().unwrap_or(' ');
    let after: String = chars.iter().skip(col + 1).collect();
    Line::from(vec![
        Span::styled(before, Style::default().fg(Theme::text_primary())),
        Span::styled(
            at.to_string(),
            Style::default().add_modifier(Modifier::REVERSED),
        ),
        Span::styled(after, Style::default().fg(Theme::text_primary())),
    ])
}

/// Render the context-dependent footer button bar and pair each button with its
/// [`ReviewButton`] action.
fn render_footer(
    frame: &mut Frame,
    area: Rect,
    composing: bool,
    side_by_side: bool,
    wrap: bool,
) -> Vec<(crate::ui::ButtonHit, ReviewButton)> {
    // Each button carries its keyboard shortcut as a dimmed hint (`label·key`)
    // so it stays discoverable. The non-composing bar leads with the essential
    // actions (Comment, Send→Agent, Close) so they survive a narrow footer —
    // `render_button_bar` drops overflow from the right and marks it with `…`.
    let view_label = if side_by_side { "Unified" } else { "Side" };
    let wrap_label = if wrap { "NoWrap" } else { "Wrap" };
    let (specs, actions): (Vec<ButtonSpec>, Vec<ReviewButton>) = if composing {
        (
            vec![
                ButtonSpec::primary("Save").with_hint("·^S"),
                ButtonSpec::secondary("Cancel").with_hint("·esc"),
                ButtonSpec::secondary("Class").with_hint("·tab"),
            ],
            vec![
                ReviewButton::Save,
                ReviewButton::Cancel,
                ReviewButton::CycleClass,
            ],
        )
    } else {
        (
            vec![
                ButtonSpec::primary("Comment").with_hint("·c"),
                ButtonSpec::primary("Send→Agent").with_hint("·e"),
                ButtonSpec::secondary("Close").with_hint("·esc"),
                ButtonSpec::secondary("Find").with_hint("·/"),
                ButtonSpec::secondary("File").with_hint("·f"),
                ButtonSpec::secondary("Summary").with_hint("·s"),
                ButtonSpec::secondary("Reviewed").with_hint("·r"),
                ButtonSpec::secondary("Target").with_hint("·t"),
                ButtonSpec::secondary(view_label).with_hint("·v"),
                ButtonSpec::secondary(wrap_label).with_hint("·w"),
                ButtonSpec::secondary("Copy").with_hint("·y"),
            ],
            vec![
                ReviewButton::Comment,
                ReviewButton::SendToAgent,
                ReviewButton::Close,
                ReviewButton::Find,
                ReviewButton::FileComment,
                ReviewButton::Summary,
                ReviewButton::MarkReviewed,
                ReviewButton::Target,
                ReviewButton::ToggleView,
                ReviewButton::ToggleWrap,
                ReviewButton::Copy,
            ],
        )
    };
    let hits = render_button_bar(frame, area, &specs, false);
    hits.into_iter().map(|h| (h, actions[h.index])).collect()
}

/// Byte offsets of every source char inside a case-insensitive occurrence of
/// `query` (already trimmed + lowercased) within `text`. Non-overlapping,
/// left-to-right. The text is lowered char-by-char *including multi-char
/// expansions* (`İ` → `i̇`), matching `search_matches`' `str::to_lowercase`
/// row filter — a per-char equality would leave such matcher hits rendering
/// with no highlight. Feeds
/// [`crate::ui::highlight::highlighted_spans_owned`] so search hits get the
/// same accent+bold+underline emphasis as the fuzzy lists elsewhere in the app.
fn match_positions(text: &str, query: &str) -> Vec<usize> {
    if query.is_empty() {
        return Vec::new();
    }
    // (source byte offset, lowered char) — an expansion repeats its offset, so
    // a window landing anywhere inside it still highlights the source char.
    let lowered: Vec<(usize, char)> = text
        .char_indices()
        .flat_map(|(o, c)| c.to_lowercase().map(move |lc| (o, lc)))
        .collect();
    let q: Vec<char> = query.chars().collect();
    let (n, m) = (lowered.len(), q.len());
    let mut positions = Vec::new();
    let mut i = 0;
    while i + m <= n {
        if (0..m).all(|k| lowered[i + k].1 == q[k]) {
            positions.extend((0..m).map(|k| lowered[i + k].0));
            i += m;
        } else {
            i += 1;
        }
    }
    // Offsets are non-decreasing; expansions produce adjacent duplicates.
    positions.dedup();
    positions
}

/// Spans for `text` styled with `base`, with literal `query` hits highlighted
/// when `query` (lowercased, non-empty) is set and matches; otherwise a single
/// plain span. Owned so the spans can outlive a per-row `String`.
fn highlight_text(text: String, base: Style, query: Option<&str>) -> Vec<Span<'static>> {
    if let Some(q) = query {
        let positions = match_positions(&text, q);
        if !positions.is_empty() {
            return crate::ui::highlight::highlighted_spans_owned(&text, &positions, base);
        }
    }
    vec![Span::styled(text, base)]
}

/// Render the find-in-diff search bar (the top row of the diff area while a
/// search is open): the `/`-prefixed query (with a caret while typing) plus the
/// match position/count and key hints, so the shortcut stays discoverable.
fn render_search_bar(frame: &mut Frame, area: Rect, state: &CodeReviewState) {
    let Some(s) = state.search.as_ref() else {
        return;
    };
    let style = Style::default().fg(Theme::search_bar());
    let total = s.matches.len();
    // The "current" position is derived from the selection (like the file
    // viewer's `current_match_index`), so it stays correct after `n`/`N` or a
    // plain `j`/`k` move; 0 when the cursor isn't on a match.
    let current = s
        .matches
        .iter()
        .position(|&i| i == state.selected)
        .map(|p| p + 1)
        .unwrap_or(0);
    let count = if s.query.trim().is_empty() {
        String::new()
    } else if total == 0 {
        "  no matches".to_string()
    } else {
        format!("  {current}/{total}")
    };
    let caret = if s.editing { "█" } else { "" };
    let hint = if s.editing {
        "   ↵/↓ next · ↑ prev · tab done · esc cancel"
    } else {
        "   n next · N prev · esc clear"
    };
    let text = format!("/{}{caret}{count}{hint}", s.query);
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            truncate(&text, area.width as usize),
            style,
        ))),
        area,
    );
}

/// Truncate `s` to `width` display columns (char-based; good enough for the
/// diff text we render).
fn truncate(s: &str, width: usize) -> String {
    if s.chars().count() <= width {
        s.to_string()
    } else {
        s.chars().take(width.saturating_sub(1)).collect::<String>() + "…"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::code_review::CodeReviewState;
    use crate::session::review::{DiffFile, DiffHunk, DiffLine, DiffLineKind, FileStatus};
    use crate::session::SessionId;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;
    use std::collections::HashSet;

    fn demo_state() -> CodeReviewState {
        let file = DiffFile {
            path: "src/a/very/deep/foo.rs".into(),
            old_path: None,
            status: FileStatus::Modified,
            hunks: vec![DiffHunk {
                old_start: 1,
                new_start: 1,
                header: "fn x".into(),
                lines: vec![
                    DiffLine {
                        kind: DiffLineKind::Context,
                        old_no: Some(1),
                        new_no: Some(1),
                        text: "ctx".into(),
                    },
                    DiffLine {
                        kind: DiffLineKind::Del,
                        old_no: Some(2),
                        new_no: None,
                        text: "old".into(),
                    },
                    DiffLine {
                        kind: DiffLineKind::Add,
                        old_no: None,
                        new_no: Some(2),
                        text: "new".into(),
                    },
                ],
            }],
        };
        let mut s = CodeReviewState {
            session_id: SessionId::default(),
            loading: false,
            repos: vec![crate::app::code_review::ReviewRepo {
                label: String::new(),
                dir: std::path::PathBuf::from("/tmp"),
                base: Some("main".into()),
            }],
            multi: false,
            files: vec![file],
            comments: Vec::new(),
            reviewed_files: HashSet::new(),
            reviewed_hunks: HashSet::new(),
            fold_override: HashSet::new(),
            rows: Vec::new(),
            selected: 0,
            scroll: 0,
            compose: None,
            side_by_side: false,
            click_side: None,
            h_scroll: 0,
            wrap: false,
            target: crate::app::code_review::ReviewTarget::Branch,
            commits: Vec::new(),
            host: None,
            target_picker: None,
            search: None,
        };
        s.rebuild_rows();
        s
    }

    /// Like `demo_state` but the single hunk's first line is very long, so
    /// horizontal-scroll and wrap have something to act on.
    fn demo_state_long() -> CodeReviewState {
        let mut s = demo_state();
        s.files[0].hunks[0].lines[0].text = "z".repeat(300);
        s.rebuild_rows();
        s
    }

    #[test]
    fn renders_unified_and_side_by_side_without_panic() {
        for sxs in [false, true] {
            for wrap in [false, true] {
                let mut state = demo_state();
                state.side_by_side = sxs;
                state.wrap = wrap;
                let mut term = Terminal::new(TestBackend::new(100, 20)).unwrap();
                term.draw(|f| {
                    let area = Rect::new(0, 0, 100, 20);
                    let hits = render(f, area, &mut state, FocusLevel::Focused, 0);
                    assert!(!hits.rows.is_empty(), "diff rows are clickable");
                    assert!(!hits.buttons.is_empty(), "footer buttons render");
                })
                .unwrap();
            }
        }
    }

    /// True paired side-by-side draws a deletion and its aligned addition on the
    /// SAME screen row: one line shows the old body left, the new body right,
    /// split by the `│` separator — the density win over v1's split-column.
    #[test]
    fn paired_side_by_side_shows_both_sides_on_one_row() {
        let mut state = demo_state();
        state.side_by_side = true;
        state.rebuild_rows();
        let mut term = Terminal::new(TestBackend::new(60, 20)).unwrap();
        term.draw(|f| {
            let _ = render(
                f,
                Rect::new(0, 0, 60, 20),
                &mut state,
                FocusLevel::Focused,
                0,
            );
        })
        .unwrap();
        let buf = term.backend().buffer();
        let mut paired_row = false;
        for y in 0..20 {
            let row: String = (0..60).map(|x| buf[(x, y)].symbol()).collect();
            if row.contains("old") && row.contains("new") && row.contains('│') {
                paired_row = true;
            }
        }
        assert!(
            paired_row,
            "the deletion (old) and addition (new) render on one paired row"
        );
    }

    /// A far jump under wrap (`G`-style: selection moved, scroll untouched)
    /// converges in one pass: `converge_wrap_scroll` walks visual-row counts
    /// backward from the selection instead of the build loop's
    /// one-row-per-rebuild retry, and lands on the same invariant — the
    /// selection's first visual row fits within the viewport height.
    #[test]
    fn converge_wrap_scroll_jumps_directly_to_the_selection() {
        let mut state = demo_state_long();
        state.wrap = true;
        // Rows: FileHeader, HunkHeader, the 300-char line (row 2), then two
        // short lines. At width 40 / num_w 2 the long line wraps to 10 visual
        // rows, so with height 5 a selection on the last row (4) must scroll
        // past it: rows 3+4 fill 2 of the 4 non-selection slots, row 2's 10
        // don't fit → scroll = 3.
        state.selected = state.rows.len() - 1;
        state.scroll = 0;
        converge_wrap_scroll(&mut state, 40, 2, 5);
        assert_eq!(state.scroll, 3);

        // A selection already in reach leaves scroll alone.
        let mut near = demo_state_long();
        near.wrap = true;
        near.selected = 1;
        converge_wrap_scroll(&mut near, 40, 2, 5);
        assert_eq!(near.scroll, 0);
    }

    /// Wrap expands a long line onto extra visual rows: several hitboxes share
    /// the same logical `index` (so a click on any wrapped row selects the whole
    /// line), and the tail of the line appears on a lower row.
    #[test]
    fn wrap_expands_long_line_into_multiple_visual_rows() {
        let mut state = demo_state_long();
        state.wrap = true;
        // Select the long line (row 2: FileHeader, HunkHeader, then first line).
        state.selected = 2;
        let mut term = Terminal::new(TestBackend::new(40, 20)).unwrap();
        let mut hits: Vec<RowHitbox> = Vec::new();
        term.draw(|f| {
            hits = render(
                f,
                Rect::new(0, 0, 40, 20),
                &mut state,
                FocusLevel::Focused,
                0,
            )
            .rows;
        })
        .unwrap();
        let dup = hits.iter().filter(|h| h.index == 2).count();
        assert!(
            dup >= 2,
            "the long line wraps onto ≥2 visual rows sharing its logical index (got {dup})"
        );
        // The wrapped continuation (the second visual row of logical line 2)
        // still shows body text — with a blank gutter under the first row.
        let cont_y = hits.iter().filter(|h| h.index == 2).nth(1).unwrap().rect.y;
        let buf = term.backend().buffer();
        let cont: String = (0..40).map(|x| buf[(x, cont_y)].symbol()).collect();
        assert!(
            cont.contains('z'),
            "wrapped continuation shows body text: {cont:?}"
        );
    }

    /// Wrap in side-by-side: a long paired row expands onto several visual rows
    /// sharing its logical index (so selection/anchor stay 1 row = 1 unit), each
    /// keeps the `│` separator, and the wrapped tail shows body text on a lower
    /// row — the split-mode mirror of [`wrap_expands_long_line_into_multiple_visual_rows`].
    #[test]
    fn wrap_expands_paired_row_in_side_by_side() {
        let mut state = demo_state_long();
        state.side_by_side = true;
        state.wrap = true;
        state.rebuild_rows();
        // Side-by-side rows: FileHeader, HunkHeader, then one paired row per
        // SidePair. The 300-char line is the context pair (logical row 2).
        state.selected = 2;
        let mut term = Terminal::new(TestBackend::new(40, 20)).unwrap();
        let mut hits: Vec<RowHitbox> = Vec::new();
        term.draw(|f| {
            hits = render(
                f,
                Rect::new(0, 0, 40, 20),
                &mut state,
                FocusLevel::Focused,
                0,
            )
            .rows;
        })
        .unwrap();
        let dup = hits.iter().filter(|h| h.index == 2).count();
        assert!(
            dup >= 2,
            "the long paired row wraps onto ≥2 visual rows sharing its logical index (got {dup})"
        );
        let cont_y = hits.iter().filter(|h| h.index == 2).nth(1).unwrap().rect.y;
        let buf = term.backend().buffer();
        let cont: String = (0..40).map(|x| buf[(x, cont_y)].symbol()).collect();
        assert!(
            cont.contains('z'),
            "wrapped continuation shows body text: {cont:?}"
        );
        assert!(
            cont.contains('│'),
            "the paired separator persists on continuation rows: {cont:?}"
        );
    }

    /// Asymmetric paired wrap: a long deletion paired with a short addition. The
    /// taller (left) half drives the visual-row count — and the builder emits
    /// exactly as many rows as `paired_visual_count` predicts (the counter/render
    /// mirror can't drift) — while the shorter half renders only on the first row
    /// and pads blank on the continuations.
    #[test]
    fn wrap_paired_row_taller_half_drives_rows_shorter_pads_blank() {
        let mut state = demo_state();
        // demo_state's hunk is [ctx, Del "old", Add "new"]; make the deletion
        // long so its half wraps while the addition stays a single row.
        state.files[0].hunks[0].lines[1].text = "x".repeat(300);
        state.side_by_side = true;
        state.wrap = true;
        state.rebuild_rows();
        // Side-by-side rows: FileHeader, HunkHeader, context pair (2), the
        // del/add pair (3, keyed by the old line index 1).
        state.selected = 3;
        // `render` wraps the diff in a bordered block, so the row builder sees a
        // content width of (area 40 − 1-col border each side) = 38 — feed the
        // counter the same so `expected` matches what the builder emits.
        let num_w = line_number_width(&state);
        let expected = paired_visual_count(&state.files[0].hunks[0], 1, 38, num_w);
        assert!(expected >= 2, "the long deletion should wrap to ≥2 rows");

        // Tall enough that the whole wrapped pair (plus the header/context rows
        // and the footer) fits the viewport, so the windowed render isn't cut
        // short and can be compared against the full `paired_visual_count`.
        let mut term = Terminal::new(TestBackend::new(40, 40)).unwrap();
        let mut hits: Vec<RowHitbox> = Vec::new();
        term.draw(|f| {
            hits = render(
                f,
                Rect::new(0, 0, 40, 40),
                &mut state,
                FocusLevel::Focused,
                0,
            )
            .rows;
        })
        .unwrap();

        let pair_rows: Vec<u16> = hits
            .iter()
            .filter(|h| h.index == 3)
            .map(|h| h.rect.y)
            .collect();
        assert_eq!(
            pair_rows.len(),
            expected,
            "the builder emits exactly paired_visual_count rows"
        );

        let buf = term.backend().buffer();
        let row_text = |y: u16| -> String { (0..40).map(|x| buf[(x, y)].symbol()).collect() };
        // The short addition ("new") shows on exactly one visual row of the pair.
        let with_new = pair_rows
            .iter()
            .filter(|&&y| row_text(y).contains("new"))
            .count();
        assert_eq!(
            with_new, 1,
            "shorter half renders once, blank on continuations"
        );
        // Every row of the pair carries the long deletion's wrapping tail.
        assert!(
            pair_rows.iter().all(|&y| row_text(y).contains('x')),
            "the taller half's wrapped body spans all pair rows"
        );
    }

    /// With wrap off, a long line stays one visual row per logical row (the
    /// selection/anchor invariant), and horizontal scroll reveals its tail with
    /// the line-number gutter still pinned at column 0.
    #[test]
    fn h_scroll_reveals_tail_with_pinned_gutter() {
        let mut state = demo_state_long();
        state.wrap = false;
        state.h_scroll = 100;
        let mut term = Terminal::new(TestBackend::new(40, 20)).unwrap();
        let mut hits: Vec<RowHitbox> = Vec::new();
        term.draw(|f| {
            hits = render(
                f,
                Rect::new(0, 0, 40, 20),
                &mut state,
                FocusLevel::Focused,
                0,
            )
            .rows;
        })
        .unwrap();
        // One hitbox per logical row — no visual expansion when wrap is off.
        assert_eq!(
            hits.iter().filter(|h| h.index == 2).count(),
            1,
            "wrap off → one visual row per logical line"
        );
        // Locate the diff-line row (logical index 2) on screen via its hitbox;
        // read from the hitbox's x (past the block border) so we see the gutter.
        let hb = hits.iter().find(|h| h.index == 2).unwrap();
        let (x0, y) = (hb.rect.x, hb.rect.y);
        let buf = term.backend().buffer();
        let row: String = (x0..x0 + hb.rect.width)
            .map(|x| buf[(x, y)].symbol())
            .collect();
        // The gutter (old/new line numbers "1 1") is still pinned at the left.
        assert!(
            row.trim_start().starts_with('1'),
            "line-number gutter stays pinned at the body's left edge: {row:?}"
        );
        assert!(
            row.contains('z'),
            "the scrolled-in body is visible: {row:?}"
        );
    }

    #[test]
    fn footer_leads_with_essentials_and_marks_overflow_when_narrow() {
        // 40 cols fits exactly Comment·c (11) + Send→Agent·e (14) + Close·esc
        // (11) with separators (11+1+14+1+11 = 38); the rest overflow.
        let mut term = Terminal::new(TestBackend::new(40, 1)).unwrap();
        let mut actions = Vec::new();
        term.draw(|f| {
            actions = render_footer(f, Rect::new(0, 0, 40, 1), false, false, false)
                .into_iter()
                .map(|(_, a)| a)
                .collect();
        })
        .unwrap();
        assert_eq!(
            actions,
            vec![
                ReviewButton::Comment,
                ReviewButton::SendToAgent,
                ReviewButton::Close,
            ],
            "the essential actions lead and survive a narrow footer"
        );
        let buf = term.backend().buffer();
        let row: String = (0..40).map(|x| buf[(x, 0)].symbol()).collect();
        assert!(
            row.contains('…'),
            "dropped buttons are marked with an ellipsis"
        );
    }

    /// A review whose files span several directories, for the changed-files list.
    fn files_state(paths: &[&str]) -> CodeReviewState {
        let mut s = demo_state();
        s.files = paths
            .iter()
            .map(|p| DiffFile {
                path: (*p).into(),
                old_path: None,
                status: FileStatus::Modified,
                hunks: vec![DiffHunk {
                    old_start: 1,
                    new_start: 1,
                    header: String::new(),
                    lines: vec![DiffLine {
                        kind: DiffLineKind::Add,
                        old_no: None,
                        new_no: Some(1),
                        text: "x".into(),
                    }],
                }],
            })
            .collect();
        s.rebuild_rows();
        s
    }

    /// Paint the changed-files list and read the cells back, one string per row.
    fn files_frame(state: &CodeReviewState, w: u16, h: u16) -> Vec<String> {
        let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
        term.draw(|f| {
            let _ = render_files_list(f, Rect::new(0, 0, w, h), state, FocusLevel::Active);
        })
        .unwrap();
        let buf = term.backend().buffer().clone();
        (0..h)
            .map(|y| {
                (0..w)
                    .map(|x| buf[(x, y)].symbol())
                    .collect::<String>()
                    .trim_end()
                    .to_string()
            })
            .collect()
    }

    /// The pre-port span builders for the changed-files list, retained as the
    /// oracle that pins its tree to what the pane drew before it: these are the
    /// code this module shipped until the rows became view-tree nodes, and
    /// `the_changed_files_tree_paints_what_the_span_builder_painted` asserts the
    /// two agree cell for cell.
    fn legacy_folder_row_line<'a>(depth: usize, name: &str, width: usize) -> Line<'a> {
        let text = format!("{}{name}/", "  ".repeat(depth));
        Line::from(Span::styled(
            truncate(&text, width),
            Style::default()
                .fg(Theme::text_muted())
                .add_modifier(Modifier::BOLD),
        ))
    }

    fn legacy_file_row_line<'a>(
        state: &CodeReviewState,
        index: usize,
        depth: usize,
        current: bool,
    ) -> Line<'a> {
        let f = &state.files[index];
        let mark = if state.reviewed_files.contains(&f.path) {
            "✓ "
        } else {
            "  "
        };
        let name = f.path.rsplit('/').next().unwrap_or(&f.path).to_string();
        let base = if current {
            Style::default()
                .fg(Theme::selection_fg())
                .bg(Theme::selection_bg())
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Theme::text_primary())
        };
        let tint = |c: Color| if current { base } else { base.fg(c) };
        Line::from(vec![
            Span::styled(format!("{}{mark}", "  ".repeat(depth)), base),
            Span::styled(f.status.glyph().to_string(), tint(status_color(f.status))),
            Span::styled(format!(" {name}  "), base),
            Span::styled(format!("+{}", f.added_count()), tint(Theme::diff_added())),
            Span::styled(
                format!(" -{}", f.deleted_count()),
                tint(Theme::diff_removed()),
            ),
        ])
    }

    /// Paint the same rows twice — once through the view tree the pane now
    /// builds, once through the spans it used to — and compare the buffers.
    fn render_files_both(
        state: &CodeReviewState,
        width: u16,
    ) -> (ratatui::buffer::Buffer, ratatui::buffer::Buffer) {
        let (rows, cursor) = state.file_row_snapshots(usize::MAX);
        let height = rows.len() as u16;
        let area = Rect::new(0, 0, width, height);

        let mut tree_term = Terminal::new(TestBackend::new(width, height)).unwrap();
        tree_term
            .draw(|f| {
                let tree = files_list_tree(&rows, cursor);
                crate::ui::plugin_pane::render_tree_rows(
                    &tree,
                    area,
                    &crate::ui::theme::current(),
                    &FrameTable::default(),
                    f.buffer_mut(),
                );
            })
            .unwrap();

        let current = state.current_file();
        let legacy_rows = crate::session::review::file_tree_rows(&state.files);
        let mut legacy_term = Terminal::new(TestBackend::new(width, height)).unwrap();
        legacy_term
            .draw(|f| {
                let lines: Vec<Line> = legacy_rows
                    .iter()
                    .map(|row| match row {
                        crate::session::review::FileTreeRow::Folder { depth, name } => {
                            legacy_folder_row_line(*depth, name, width as usize)
                        }
                        crate::session::review::FileTreeRow::File { depth, index } => {
                            legacy_file_row_line(state, *index, *depth, current == Some(*index))
                        }
                    })
                    .collect();
                f.render_widget(Paragraph::new(lines), area);
            })
            .unwrap();

        (
            tree_term.backend().buffer().clone(),
            legacy_term.backend().buffer().clone(),
        )
    }

    /// The changed-files list is drawn by the shared painter now; what it draws
    /// is what its spans drew — every cell, every colour, at a width that fits
    /// and one that cuts a folder name short.
    #[test]
    fn the_changed_files_tree_paints_what_the_span_builder_painted() {
        let mut state = files_state(&["src/b.rs", "top.rs", "src/ui/a.rs", "src/ui/z.rs"]);
        state.reviewed_files.insert("top.rs".into());
        for width in [30u16, 12] {
            let (tree, legacy) = render_files_both(&state, width);
            assert_eq!(
                tree, legacy,
                "the changed-files list must paint what its spans painted (width {width})"
            );
        }
    }

    /// The window is the kernel's shared rule now, which is a **behaviour
    /// change**: the pane used to keep the current file on the last visible row
    /// once the list overflowed, and now opens a margin above it like every other
    /// list thurbox draws.
    #[test]
    fn the_changed_files_window_is_the_shared_rule() {
        let paths: Vec<String> = (0..12).map(|i| format!("src/f{i:02}.rs")).collect();
        let refs: Vec<&str> = paths.iter().map(String::as_str).collect();
        let mut state = files_state(&refs);
        state.selected = state
            .rows
            .iter()
            .position(|r| matches!(r, ReviewRow::FileHeader(9)))
            .expect("the tenth file has a header row");

        let frame = files_frame(&state, 30, 8);
        // One `src/` header + twelve files, five content rows: the shared rule
        // opens `min(height / 4, 3)` rows above the cursor and clamps at the
        // tail, so f09 sits third of five rather than last.
        let visible: Vec<&str> = frame[1..6].iter().map(String::as_str).collect();
        assert!(
            visible[2].contains("f09.rs"),
            "the current file should sit a margin below the window's top: {visible:?}"
        );
        assert!(
            visible[4].contains("f11.rs"),
            "the window should clamp at the list's tail: {visible:?}"
        );
    }

    #[test]
    fn renders_changed_files_list_without_panic() {
        let state = demo_state();
        let mut term = Terminal::new(TestBackend::new(30, 10)).unwrap();
        term.draw(|f| {
            let area = Rect::new(0, 0, 30, 10);
            let rows = render_files_list(f, area, &state, FocusLevel::Active);
            assert_eq!(rows.len(), 1, "one changed file → one clickable row");
        })
        .unwrap();
    }

    #[test]
    fn match_positions_finds_case_insensitive_runs() {
        // "Foo bar foo" with query "foo" → two non-overlapping matches, each
        // contributing one byte offset per char (3 chars → 3 offsets).
        let pos = match_positions("Foo bar foo", "foo");
        assert_eq!(pos, vec![0, 1, 2, 8, 9, 10]);
        // No match → empty; empty query → empty.
        assert!(match_positions("abc", "z").is_empty());
        assert!(match_positions("abc", "").is_empty());
    }

    #[test]
    fn match_positions_handles_multichar_lowercase_expansion() {
        // `İ` (U+0130) lowers to two chars (`i` + U+0307). `search_matches`
        // lowers rows with `str::to_lowercase`, so it counts this row as a hit
        // for the lowered query — the highlighter must agree and mark the
        // source char (one deduped offset, since both lowered chars share it).
        let lowered = "\u{130}".to_lowercase(); // "i\u{307}"
        let pos = match_positions("a\u{130}b", &lowered);
        assert_eq!(pos, vec![1], "the single source char is highlighted once");
    }

    #[test]
    fn renders_search_bar_and_highlights_match() {
        let mut state = demo_state();
        state.search = Some(crate::app::code_review::ReviewSearch {
            query: "ctx".to_string(),
            editing: true,
            matches: state.search_matches("ctx"),
        });
        let mut term = Terminal::new(TestBackend::new(80, 20)).unwrap();
        term.draw(|f| {
            let area = Rect::new(0, 0, 80, 20);
            let _ = render(f, area, &mut state, FocusLevel::Focused, 0);
        })
        .unwrap();
        // The search bar prints the `/`-prefixed query somewhere on screen.
        let buf = term.backend().buffer();
        let mut screen = String::new();
        for y in 0..20 {
            for x in 0..80 {
                screen.push_str(buf[(x, y)].symbol());
            }
            screen.push('\n');
        }
        assert!(
            screen.contains("/ctx"),
            "search bar shows the query: {screen}"
        );
    }

    #[test]
    fn renders_inline_compose_without_panic() {
        use crate::session::review::{Classification, CommentAnchor, Side};
        let mut state = demo_state();
        state.selected = 2; // a Line row
        state.compose = Some(crate::app::code_review::ComposeState {
            anchor: CommentAnchor::Line {
                file: "src/foo.rs".into(),
                side: Side::New,
                line: 2,
            },
            classification: Classification::Issue,
            body: crate::app::modals::TextArea::new(),
            editing_id: None,
        });
        let mut term = Terminal::new(TestBackend::new(100, 20)).unwrap();
        term.draw(|f| {
            let area = Rect::new(0, 0, 100, 20);
            let hits = render(f, area, &mut state, FocusLevel::Focused, 0);
            // Footer still renders its buttons while composing.
            assert!(!hits.buttons.is_empty());
            // The compose box is the pane's one overlay, so a click on it is
            // hit-tested before the diff row it covers.
            assert_eq!(hits.overlay.len(), 1);
            let box_rect = hits.overlay[0];
            let anchored_row = hits
                .rows
                .iter()
                .find(|h| h.index == state.selected)
                .expect("the anchored row is on screen");
            assert_eq!(
                box_rect.y,
                anchored_row.rect.y + 1,
                "the box sits on the row after the line it comments on"
            );
        })
        .unwrap();
    }

    #[test]
    fn renders_no_overlay_when_nothing_is_composed() {
        let mut state = demo_state();
        let mut term = Terminal::new(TestBackend::new(100, 20)).unwrap();
        term.draw(|f| {
            let hits = render(
                f,
                Rect::new(0, 0, 100, 20),
                &mut state,
                FocusLevel::Focused,
                0,
            );
            assert!(
                hits.overlay.is_empty(),
                "a pane with nothing anchored declares no overlay"
            );
        })
        .unwrap();
    }

    /// The pre-port `render_compose_inline` placement, kept as the oracle the
    /// anchored version must reproduce: below the line when it fits, above it
    /// when that fits, else pinned to the bottom, inset one column.
    fn legacy_compose_rect(area: Rect, anchor_y: Option<u16>) -> Rect {
        let h = area.height.clamp(3, 6);
        let bottom = area.y + area.height;
        let top = match anchor_y {
            Some(ay) if ay + 1 + h <= bottom => ay + 1,
            Some(ay) if ay >= area.y + h => ay - h,
            _ => bottom.saturating_sub(h),
        };
        Rect::new(area.x + 1, top, area.width.saturating_sub(2).max(1), h)
    }

    #[test]
    fn compose_anchor_reproduces_the_legacy_inline_placement() {
        // Widths from 2 and heights from 3 up: below that the legacy formula let
        // the box escape its pane, which the anchor deliberately clamps instead
        // (`compose_anchor_clamps_a_pane_too_short_for_the_box`).
        for width in [2_u16, 3, 8, 60] {
            for height in 3..24_u16 {
                let area = Rect::new(4, 2, width, height);
                let anchor = compose_anchor(area);
                for y in area.y..area.y + area.height {
                    let row = Rect::new(area.x, y, area.width, 1);
                    assert_eq!(
                        anchor.place(Some(row), area),
                        legacy_compose_rect(area, Some(y)),
                        "anchored at row {y} in {area:?}"
                    );
                }
                assert_eq!(
                    anchor.place(None, area),
                    legacy_compose_rect(area, None),
                    "anchored line off screen in {area:?}"
                );
            }
        }
    }

    #[test]
    fn compose_anchor_clamps_a_pane_too_short_for_the_box() {
        // The one deliberate divergence: the legacy formula built a three-row box
        // in a two-row pane and docked it at `bottom - 3`, painting a row above
        // the pane. The anchor shrinks it to the pane instead.
        let area = Rect::new(0, 4, 40, 2);
        let legacy = legacy_compose_rect(area, Some(4));
        assert!(legacy.y < area.y, "the legacy box escaped upward");
        let placed = compose_anchor(area).place(Some(Rect::new(0, 4, 40, 1)), area);
        assert_eq!((placed.y, placed.height), (4, 2));
    }

    /// Paint a view tree the way any pane is painted.
    fn paint_tree(node: &ViewNode, width: u16) -> ratatui::buffer::Buffer {
        let area = Rect::new(0, 0, width, 1);
        let mut buf = ratatui::buffer::Buffer::empty(area);
        crate::ui::plugin_pane::render_tree(
            node,
            area,
            &crate::ui::theme::current(),
            &crate::session::motion::FrameTable::default(),
            &mut buf,
        );
        buf
    }

    /// Paint the **untouched** native row the same way.
    fn paint_native(
        f: &DiffFile,
        l: &DiffLine,
        width: u16,
        num_w: usize,
        selected: bool,
    ) -> ratatui::buffer::Buffer {
        let sel_style = |base: Style| {
            if selected {
                base.bg(Theme::selection_bg()).fg(Theme::selection_fg())
            } else {
                base
            }
        };
        let line = unified_diff_line(f, l, width as usize, num_w, selected, 0, None, &sel_style);
        let area = Rect::new(0, 0, width, 1);
        let mut buf = ratatui::buffer::Buffer::empty(area);
        ratatui::widgets::Widget::render(Paragraph::new(line), area, &mut buf);
        buf
    }

    /// Compare two painted rows cell by cell.
    ///
    /// Symbol, background and modifiers must be identical everywhere. The
    /// foreground is compared only where the symbol is not a space, and that
    /// latitude is the one difference between the two paths rather than a
    /// convenience: the native renderer pads its row with `Style::default()`,
    /// which leaves a blank cell's foreground unset, while a fill run resolves
    /// the theme's primary foreground. A space carries no ink, so the two paint
    /// the same row — and every cell that *does* carry ink is required to match
    /// exactly, which is where a real divergence would show.
    fn assert_same_row(
        plugin: &ratatui::buffer::Buffer,
        native: &ratatui::buffer::Buffer,
        case: &str,
    ) {
        assert_eq!(plugin.area, native.area, "{case}: different areas");
        for x in 0..plugin.area.width {
            let a = &plugin[(x, 0)];
            let b = &native[(x, 0)];
            assert_eq!(a.symbol(), b.symbol(), "{case}: symbol at column {x}");
            assert_eq!(a.bg, b.bg, "{case}: background at column {x}");
            assert_eq!(a.modifier, b.modifier, "{case}: modifiers at column {x}");
            if a.symbol() != " " {
                assert_eq!(a.fg, b.fg, "{case}: foreground at column {x}");
            }
        }
    }

    fn diff_line(
        kind: DiffLineKind,
        old_no: Option<u32>,
        new_no: Option<u32>,
        text: &str,
    ) -> DiffLine {
        DiffLine {
            kind,
            old_no,
            new_no,
            text: text.to_string(),
        }
    }

    /// The published form of a diff line, which is what a pane receives and what
    /// the tree builder reads.
    fn snap_line(path: &str, l: &DiffLine) -> pc::ReviewLineSnapshot {
        pc::ReviewLineSnapshot {
            path: path.to_string(),
            old_no: l.old_no,
            new_no: l.new_no,
            kind: l.kind.as_str(),
            text: l.text.clone(),
        }
    }

    fn file_at(path: &str) -> DiffFile {
        DiffFile {
            path: path.to_string(),
            old_path: None,
            status: FileStatus::Modified,
            hunks: Vec::new(),
        }
    }

    /// The bridge that makes the bundled `code-review` plugin's tree equality
    /// mean something: the geometry-free row builder and the pane's own renderer
    /// paint the **same row**.
    ///
    /// Without this the plugin would only be agreeing with a function written in
    /// the same change. This one compares against `unified_diff_line`, which is
    /// untouched by the port — so the chain closes on what the pane paints today.
    #[test]
    fn the_row_tree_paints_the_native_row() {
        const WIDTH: u16 = 60;
        let cases: Vec<(&str, DiffLine, bool)> = vec![
            // Each of the three kinds: the tint is what separates them, and the
            // gutter's sign is one character.
            (
                "context",
                diff_line(
                    DiffLineKind::Context,
                    Some(11),
                    Some(12),
                    "let total = sum;",
                ),
                false,
            ),
            (
                "an insertion",
                diff_line(DiffLineKind::Add, None, Some(13), "let total = sum + 1;"),
                false,
            ),
            (
                "a deletion",
                diff_line(DiffLineKind::Del, Some(12), None, "let total = sum;"),
                false,
            ),
            // The cursor's row: selection has to beat the tint, on an insertion
            // where the two backgrounds would otherwise both apply.
            (
                "the cursor on an insertion",
                diff_line(DiffLineKind::Add, None, Some(14), "fn main() {}"),
                true,
            ),
            (
                "the cursor on context",
                diff_line(DiffLineKind::Context, Some(1), Some(1), "}"),
                true,
            ),
            // Every colour the highlighter assigns, so a wrong token shows up as
            // a wrong cell rather than as a shorter row.
            (
                "a keyword and a type",
                diff_line(
                    DiffLineKind::Context,
                    Some(2),
                    Some(2),
                    "pub fn build(x: Vec<Row>) {",
                ),
                false,
            ),
            (
                "a string",
                diff_line(DiffLineKind::Add, None, Some(3), "let s = \"hi there\";"),
                false,
            ),
            (
                "a number",
                diff_line(DiffLineKind::Context, Some(4), Some(4), "let n = 0x1F_u32;"),
                false,
            ),
            (
                "a comment",
                diff_line(DiffLineKind::Context, Some(5), Some(5), "// why, not what"),
                false,
            ),
            (
                "an unterminated string",
                diff_line(DiffLineKind::Add, None, Some(6), "let s = \"open"),
                false,
            ),
            // An empty body still draws its gutter and its tint.
            (
                "an empty body",
                diff_line(DiffLineKind::Add, None, Some(7), ""),
                false,
            ),
            // Longer than the pane: the native path slices the body by character
            // count and the tree lets the renderer clip it, which agree for the
            // ASCII a diff of code is.
            (
                "longer than the pane",
                diff_line(DiffLineKind::Context, Some(8), Some(8), &"x".repeat(120)),
                false,
            ),
        ];

        let f = file_at("src/build.rs");
        for (name, line, selected) in cases {
            for num_w in [2usize, 4] {
                assert_same_row(
                    &paint_tree(
                        &diff_row_tree(&snap_line(&f.path, &line), num_w, selected),
                        WIDTH,
                    ),
                    &paint_native(&f, &line, WIDTH, num_w, selected),
                    &format!("{name} (gutter width {num_w})"),
                );
            }
        }
    }

    /// The comment marker follows the file's extension in both paths, which is the
    /// one thing `diff_row_tree` reads the path for.
    #[test]
    fn the_row_tree_takes_its_comment_marker_from_the_path() {
        const WIDTH: u16 = 40;
        for (path, text) in [
            ("conf/app.toml", "# a hash comment"),
            ("db/schema.sql", "-- a dash comment"),
            ("src/main.rs", "// a slash comment"),
            // The same text where the marker does *not* apply: a `#` in a Rust
            // file is an attribute, not a comment, so the two paths must agree
            // that it is plain.
            ("src/main.rs", "#[derive(Debug)]"),
        ] {
            let f = file_at(path);
            let line = diff_line(DiffLineKind::Context, Some(1), Some(1), text);
            assert_same_row(
                &paint_tree(&diff_row_tree(&snap_line(path, &line), 2, false), WIDTH),
                &paint_native(&f, &line, WIDTH, 2, false),
                &format!("{path}: {text}"),
            );
        }
    }

    /// A row's structure, asserted directly so the equality above cannot pass on
    /// two nearly-empty lines.
    #[test]
    fn a_row_is_a_gutter_then_tokens_then_a_fill() {
        let line = diff_line(DiffLineKind::Add, None, Some(9), "let x = 1;");
        let tree = diff_row_tree(&snap_line("src/a.rs", &line), 3, false);
        let runs = match &tree {
            ViewNode::Line(runs) => runs,
            other => panic!("expected a line, got {}", other.kind_name()),
        };
        assert!(runs.len() > 3, "one run per token: {runs:#?}");
        match &runs[0] {
            ViewNode::Text { content, style } => {
                // Three-wide columns: an empty old side, the new number right
                // aligned, then the sign — `{old:>3} {new:>3} {sign} `.
                assert_eq!(
                    content, "      9 + ",
                    "the gutter is two columns and a sign"
                );
                assert_eq!(style.tint, Some(DiffTint::Added));
            }
            other => panic!("expected the gutter, got {}", other.kind_name()),
        }
        // The last run is the fill, and it carries the tint — which is the whole
        // reason the row's background reaches the pane's edge.
        match runs.last() {
            Some(ViewNode::Fill { glyph, style }) => {
                assert_eq!(*glyph, ' ');
                assert_eq!(style.tint, Some(DiffTint::Added));
            }
            other => panic!("expected a fill, got {other:?}"),
        }
        // A context row carries no tint at all, rather than a third value.
        let plain = diff_row_tree(
            &snap_line(
                "src/a.rs",
                &diff_line(DiffLineKind::Context, Some(9), Some(9), "let x = 1;"),
            ),
            3,
            false,
        );
        assert!(plain
            .children()
            .iter()
            .all(|run| matches!(run, ViewNode::Text { style, .. } | ViewNode::Fill { style, .. } if style.tint.is_none())));
    }

    /// **Enumerated divergence: a tab.** Node content is sanitized on the way
    /// into the tree (a tab becomes four spaces, control characters are dropped),
    /// while the native span renderer passes the raw byte to the terminal. Both
    /// panes are self-consistent and the tree's reading is the safer one, but the
    /// two are not cell-identical for a body containing a tab — so the frame
    /// comparison above uses tab-free bodies and this pins the reason.
    #[test]
    fn the_tree_expands_a_tab_and_the_native_row_does_not() {
        let line = diff_line(DiffLineKind::Context, Some(1), Some(1), "\tindented");
        let tree = diff_row_tree(&snap_line("src/a.rs", &line), 2, false);
        let text: String = tree
            .children()
            .iter()
            .filter_map(|n| match n {
                ViewNode::Text { content, .. } => Some(content.as_str()),
                _ => None,
            })
            .collect();
        assert!(!text.contains('\t'), "the tree expands the tab: {text:?}");
        assert!(line.text.contains('\t'), "the native row keeps it");
    }

    /// The stream is a list that names its cursor, which is what makes a plugin's
    /// copy scroll — and an empty stream is the native pane's own `Info` line.
    #[test]
    fn the_stream_is_a_selectable_list() {
        let rows: Vec<pc::ReviewRowSnapshot> = (0..5)
            .map(|i| {
                pc::ReviewRowSnapshot::Line(snap_line(
                    "src/a.rs",
                    &diff_line(DiffLineKind::Context, Some(i + 1), Some(i + 1), "x"),
                ))
            })
            .collect();
        match review_stream_tree(&rows, Some(3), 2) {
            ViewNode::List {
                children, selected, ..
            } => {
                assert_eq!(children.len(), 5);
                assert_eq!(selected, Some(3));
            }
            other => panic!("expected a list, got {}", other.kind_name()),
        }
        // The empty state carries no cursor and one muted line.
        match review_stream_tree(&[], None, 2) {
            ViewNode::List {
                children, selected, ..
            } => {
                assert_eq!(selected, None);
                assert_eq!(children.len(), 1);
            }
            other => panic!("expected a list, got {}", other.kind_name()),
        }
    }

    /// A review holding one of everything: two files with different statuses, a
    /// reviewed (and therefore folded) file, a reviewed hunk, a comment of each
    /// classification at each anchor level, and a multi-line body.
    ///
    /// Built once and walked row by row, so the document equality below covers
    /// every row kind against the state that produces it rather than against
    /// hand-built rows that might not occur together.
    fn document_state() -> CodeReviewState {
        let mut s = demo_state();
        s.files[0].path = "src/build.rs".into();
        s.files[0].hunks[0].header = "fn build".into();
        s.files.push(DiffFile {
            path: "src/gone.rs".into(),
            old_path: None,
            status: FileStatus::Deleted,
            hunks: vec![DiffHunk {
                old_start: 40,
                new_start: 40,
                header: "fn dropped".into(),
                lines: vec![DiffLine {
                    kind: DiffLineKind::Del,
                    old_no: Some(41),
                    new_no: None,
                    text: "    let unused = 1;".into(),
                }],
            }],
        });
        s.files.push(DiffFile {
            path: "src/added.rs".into(),
            old_path: None,
            status: FileStatus::Added,
            hunks: vec![DiffHunk {
                old_start: 0,
                new_start: 1,
                header: String::new(),
                lines: vec![DiffLine {
                    kind: DiffLineKind::Add,
                    old_no: None,
                    new_no: Some(1),
                    text: "pub struct Row {}".into(),
                }],
            }],
        });
        // Reviewed, so it folds to its header alone — the row kind a pane cannot
        // derive from `reviewed` on its own.
        s.reviewed_files.insert("src/added.rs".to_string());
        s.reviewed_hunks.insert(("src/build.rs".to_string(), 0));
        let comment = |id: i64, anchor: CommentAnchor, class: Classification, body: &str| {
            crate::session::review::ReviewComment {
                id,
                session_id: SessionId::default(),
                anchor,
                classification: class,
                body: body.to_string(),
                created_at: 0,
                updated_at: 0,
            }
        };
        s.comments = vec![
            comment(
                1,
                CommentAnchor::File {
                    file: "src/build.rs".into(),
                },
                Classification::Issue,
                "this file does two things",
            ),
            comment(
                2,
                CommentAnchor::Line {
                    file: "src/build.rs".into(),
                    side: crate::session::review::Side::New,
                    line: 2,
                },
                Classification::Suggestion,
                "extract a helper",
            ),
            comment(
                3,
                CommentAnchor::Line {
                    file: "src/gone.rs".into(),
                    side: crate::session::review::Side::Old,
                    line: 41,
                },
                Classification::Praise,
                "good riddance",
            ),
            // A multi-line body, which the row marks with an ellipsis.
            comment(
                4,
                CommentAnchor::Review,
                Classification::Note,
                "overall:\nthe shape is right\nbut the naming is not",
            ),
        ];
        s.rebuild_rows();
        s
    }

    /// **The document, row kind by row kind.** Every published row's tree paints
    /// the same cells as the native pane's own row.
    ///
    /// This is the second link of the chain the bundled plugin's equality hangs
    /// from, extended past the diff line: the native side is `row_visual_lines`,
    /// the pane's real per-row dispatch, so a header or a comment is compared
    /// against what the review draws rather than against a builder written here.
    #[test]
    fn every_published_row_paints_the_native_row() {
        const WIDTH: u16 = 100;
        let mut state = document_state();
        let num_w = gutter_number_width(&state.files);
        assert!(
            state.rows.len() > 10,
            "the fixture must cover every row kind: {:?}",
            state.rows.len()
        );

        // Every kind occurs, so a passing run cannot be a run over lines alone.
        let kinds: Vec<&str> = state
            .snapshot_rows()
            .0
            .iter()
            .map(|r| r.wire_name())
            .collect();
        for kind in ["file", "hunk", "line", "comment", "summaryHeader"] {
            assert!(kinds.contains(&kind), "the fixture draws no {kind} row");
        }

        for i in 0..state.rows.len() {
            // The cursor is moved onto each row in turn, so the selection
            // appearance is compared on every kind rather than only on a line.
            state.selected = i;
            let (rows, cursor) = state.snapshot_rows();
            assert_eq!(
                cursor,
                Some(i),
                "row {i} is published with the cursor on it"
            );
            let tree = review_stream_tree(&rows, cursor, num_w);
            let child = &tree.children()[i];

            let native = row_visual_lines(&state, i, WIDTH as usize, num_w, None, 0, false)
                .pop()
                .expect("a row paints one visual line");
            let area = Rect::new(0, 0, WIDTH, 1);
            let mut buf = ratatui::buffer::Buffer::empty(area);
            ratatui::widgets::Widget::render(Paragraph::new(native), area, &mut buf);
            assert_same_row(
                &paint_tree(child, WIDTH),
                &buf,
                &format!("row {i} ({})", rows[i].wire_name()),
            );
        }
    }

    /// The same walk with the cursor parked off the rows being compared, so an
    /// unselected row of every kind is covered too.
    #[test]
    fn every_published_row_paints_the_native_row_unselected() {
        const WIDTH: u16 = 100;
        let mut state = document_state();
        let num_w = gutter_number_width(&state.files);
        state.selected = 0;
        let (rows, cursor) = state.snapshot_rows();
        let tree = review_stream_tree(&rows, cursor, num_w);
        for (i, row) in rows.iter().enumerate().skip(1) {
            let native = row_visual_lines(&state, i, WIDTH as usize, num_w, None, 0, false)
                .pop()
                .expect("a row paints one visual line");
            let area = Rect::new(0, 0, WIDTH, 1);
            let mut buf = ratatui::buffer::Buffer::empty(area);
            ratatui::widgets::Widget::render(Paragraph::new(native), area, &mut buf);
            assert_same_row(
                &paint_tree(&tree.children()[i], WIDTH),
                &buf,
                &format!("unselected row {i} ({})", row.wire_name()),
            );
        }
    }

    /// **Enumerated divergence: truncation.** The native pane cuts a hunk header,
    /// a comment, an informational row and the summary header to the pane's width
    /// and writes a trailing `…`; a view tree carries the whole text and the
    /// renderer clips it at the edge.
    ///
    /// Recorded here rather than closed, because the fact that would close it — the
    /// pane's resolved width — is the same one the side-by-side, wrap and
    /// horizontal-scroll layouts need, and handing a width to every published pane
    /// is a decision about the whole view-tree model rather than about an ellipsis.
    #[test]
    fn a_row_that_overflows_clips_where_the_native_row_ellipsizes() {
        let mut state = document_state();
        state.files[0].hunks[0].header =
            "a section heading long enough to overflow a narrow pane".to_string();
        state.rebuild_rows();
        let num_w = gutter_number_width(&state.files);
        let hunk = state
            .rows
            .iter()
            .position(|r| matches!(r, ReviewRow::HunkHeader(0, 0)))
            .expect("the fixture has a hunk header");
        state.selected = hunk;
        let (rows, cursor) = state.snapshot_rows();
        let tree = review_stream_tree(&rows, cursor, num_w);

        // Wide enough for the whole heading: identical, which is the case the
        // document equality above runs at.
        const WIDE: u16 = 100;
        let native_wide = row_visual_lines(&state, hunk, WIDE as usize, num_w, None, 0, false)
            .pop()
            .expect("one line");
        let mut buf = ratatui::buffer::Buffer::empty(Rect::new(0, 0, WIDE, 1));
        ratatui::widgets::Widget::render(
            Paragraph::new(native_wide),
            Rect::new(0, 0, WIDE, 1),
            &mut buf,
        );
        assert_same_row(&paint_tree(&tree.children()[hunk], WIDE), &buf, "wide");

        // Narrower than the heading: the native row's last cell is the ellipsis,
        // the tree's is the heading's own character.
        const NARROW: u16 = 30;
        let native_narrow = row_visual_lines(&state, hunk, NARROW as usize, num_w, None, 0, false)
            .pop()
            .expect("one line");
        let mut native = ratatui::buffer::Buffer::empty(Rect::new(0, 0, NARROW, 1));
        ratatui::widgets::Widget::render(
            Paragraph::new(native_narrow),
            Rect::new(0, 0, NARROW, 1),
            &mut native,
        );
        let plugin = paint_tree(&tree.children()[hunk], NARROW);
        assert_eq!(
            native[(NARROW - 1, 0)].symbol(),
            "…",
            "the native row ellipsizes"
        );
        assert_ne!(
            plugin[(NARROW - 1, 0)].symbol(),
            "…",
            "the tree clips instead"
        );
        // And the divergence is confined to that last column — everything the two
        // both had room for is identical, which is what makes it an enumerated
        // divergence rather than a different row.
        for x in 0..NARROW - 1 {
            assert_eq!(
                plugin[(x, 0)].symbol(),
                native[(x, 0)].symbol(),
                "column {x} must agree"
            );
        }
    }

    /// The summary header's label is the kernel's, so the two panes cannot drift
    /// over a string that names a keystroke.
    #[test]
    fn the_summary_header_label_crosses_from_the_kernel() {
        let state = document_state();
        let (rows, _) = state.snapshot_rows();
        let label = rows
            .iter()
            .find_map(|r| match r {
                pc::ReviewRowSnapshot::SummaryHeader { label } => Some(label.clone()),
                _ => None,
            })
            .expect("the summary header is published");
        assert_eq!(label, crate::app::code_review::SUMMARY_HEADER_LABEL);
        assert!(
            label.contains("(s to add)"),
            "the label names the keystroke a plugin pane never receives, which is \
             why it crosses rather than being composed in the pane: {label}"
        );
    }
}
