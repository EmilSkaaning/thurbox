use ratatui::{
    layout::Rect,
    style::Style,
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};
// Only the retained pre-port oracle (`legacy_session_line` and friends) still
// assembles ratatui spans and reaches for a palette colour by hand; the pane
// itself draws the view tree below.
#[cfg(test)]
use ratatui::style::Modifier;

use super::theme::Theme;
use super::{focus_block, FocusLevel};
use crate::session::motion::{FrameTable, Motion, DEFAULT_FPS};
use crate::session::view_tree::{StyleToken, TextStyle, ViewNode};
// The pane's *model* — the order, the reorder, the sort, the match positions and the
// width-free row — lives in the kernel's pure-data layer, because keyboard navigation
// depends on it and rendering must not be a dependency of the model (ADR-60). What is
// left in this module is the drawing, plus the two things that need a width.
use crate::session::session_list::{
    agent_status_text, group_key, repo_set_display, repo_set_key, resolve_rows, RowInputs,
    SessionMatch, SessionRow,
};
use crate::session::{SessionInfo, SessionStatus};

/// Where a still-being-created session's placeholder row belongs, and whether it
/// needs to bring its own repo header along.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingSpawnSlot {
    /// Index into the rendered rows to insert *before*; `rows` (the length) puts
    /// it last.
    pub index: usize,
    /// The group header to draw above the placeholder — `Some` only when the
    /// session opens a repo group that has no rows yet, so an existing group's
    /// header is never duplicated.
    pub header: Option<String>,
}

/// Place a pending session's placeholder row under the header of the repo group
/// it will join, rather than trailing the whole list.
///
/// It lands at the **end of its group** — the same place the real row will
/// appear, since a brand-new session has no `display_order` and so sorts after
/// its ordered siblings (see `session_list::compute_session_order`). A group that doesn't
/// exist yet is appended last with its own header, which is exactly where
/// `compute_session_order` will put it once the session lands (a group's order
/// key is its lowest member `display_order`, and an unordered one is `i64::MAX`).
///
/// `repo_names` is the pending session's repo display names; `sessions`/`headers`
/// are the already-ordered rows.
pub fn pending_spawn_slot(
    sessions: &[&SessionInfo],
    headers: &[Option<String>],
    repo_names: &[String],
) -> PendingSpawnSlot {
    let key = repo_set_key(repo_names);
    // Scan for the group's last row: group membership is contiguous in the
    // rendered order, so the run ends at the next header (or the list's end).
    let start = sessions.iter().position(|info| group_key(info) == key);
    match start {
        Some(start) => {
            let end = (start + 1..sessions.len())
                .find(|&i| headers.get(i).is_some_and(Option::is_some))
                .unwrap_or(sessions.len());
            PendingSpawnSlot {
                index: end,
                header: None,
            }
        }
        None => PendingSpawnSlot {
            index: sessions.len(),
            header: Some(repo_set_display(repo_names)),
        },
    }
}

pub struct LeftPanelState<'a> {
    pub sessions: &'a [&'a SessionInfo],
    pub active_session: usize,
    /// Whether to highlight the active-session row. `false` hides the selection
    /// entirely (e.g. while the automations pane is focused, where the active
    /// session is irrelevant).
    pub show_selection: bool,
    /// Focus level for the session list.
    pub session_focus: FocusLevel,
    /// Per-session fuzzy match positions (parallel to sessions slice).
    pub session_match_positions: &'a [Option<SessionMatch>],
    /// Whether a (global) search is active — non-matching rows are dimmed.
    pub session_search_active: bool,
    /// Parallel to `sessions`: `Some(label)` on each repo group's first row,
    /// rendered as a subtle header above that row. `None` elsewhere.
    pub headers: &'a [Option<String>],
    /// Parallel to `sessions`: tree depth within the repo group
    /// (see `session_list::SessionOrder::depths`). Children render indented.
    pub depths: &'a [u8],
    /// Current frame index of the `Working` spinner (into
    /// [`super::SPINNER_FRAMES`]).
    ///
    /// The index rather than the resolved glyph, because a working row's glyph is
    /// a *motion* node in the pane's view tree and the paint resolves it through a
    /// frame table — see `spinner_frames`.
    pub spinner_frame: usize,
    /// A session being created (git fetch / `worktree add` / spawn), rendered as
    /// a trailing placeholder row so it shows up the moment the wizard is
    /// confirmed rather than after a slow shell-out. Not selectable and not
    /// clickable — it has no `SessionId` to select yet.
    pub pending_spawn: Option<&'a crate::app::PendingSpawn>,
}

pub fn render_left_panel(
    frame: &mut Frame,
    area: Rect,
    state: &LeftPanelState<'_>,
) -> Vec<super::RowHitbox> {
    // The session list fills the whole left panel — search lives in the global
    // `Ctrl+/` strip now, so there's no in-list search bar.
    render_session_section(
        frame,
        area,
        state.sessions,
        state.active_session,
        state.show_selection,
        state.session_focus,
        state.session_match_positions,
        state.session_search_active,
        state.headers,
        state.depths,
        state.spinner_frame,
        state.pending_spawn,
    )
}

#[allow(clippy::too_many_arguments)]
fn render_session_section(
    frame: &mut Frame,
    area: Rect,
    sessions: &[&SessionInfo],
    active_index: usize,
    show_selection: bool,
    level: FocusLevel,
    match_positions: &[Option<SessionMatch>],
    search_active: bool,
    headers: &[Option<String>],
    depths: &[u8],
    spinner_frame: usize,
    pending_spawn: Option<&crate::app::PendingSpawn>,
) -> Vec<super::RowHitbox> {
    let spinner = super::SPINNER_FRAMES[spinner_frame % super::SPINNER_FRAMES.len()];
    let mut block = focus_block(" Sessions ", level);

    if !sessions.is_empty() {
        let dots: Vec<Span> = sessions
            .iter()
            .map(|info| {
                Span::styled(
                    super::status_glyph(info.status, spinner).to_string(),
                    Style::default().fg(super::status_color(info.status)),
                )
            })
            .collect();
        block = block.title_top(Line::from(dots).right_aligned());
    }

    if sessions.is_empty() && pending_spawn.is_none() {
        render_empty_sessions(frame, area, block);
        return Vec::new();
    }

    // Available width inside the block (subtract 2 for borders)
    let inner_width = area.width.saturating_sub(2) as usize;

    // Rows as view-tree nodes, painted through the same renderer a plugin pane's
    // tree goes through — so the window, the row rects and every cell inside a row
    // are resolved once, here and in the bundled plugin that reproduces this pane
    // (ADR-63).
    let items_data = resolve_items(
        &RowInputs {
            sessions,
            active_index,
            show_selection,
            match_positions,
            search_active,
            headers,
            depths,
        },
        inner_width,
    );
    let palette = super::theme::current();
    let frames = spinner_frames(spinner_frame);

    // One list child per item: a group header travels with the row below it, so
    // it scrolls with that row, clicking it selects that group's first session,
    // and the two panes count the same number of rows.
    let mut children = session_list_items(&items_data);

    // The session being created, slotted into the repo group it will land in
    // (see `pending_spawn_slot`) so it appears where it will actually live
    // rather than below every other group. It carries no `SessionInfo`, so it
    // gets no hitbox below and can never be selected — the list's selection
    // indices stay a valid range over `sessions`.
    let pending_row = pending_spawn.map(|pending| {
        let slot = pending_spawn_slot(sessions, headers, &pending.repo_display_names);
        let row = pending_spawn_node(pending, inner_width, spinner);
        // A group with no rows yet brings its own header, so the placeholder is
        // filed under a label instead of floating loose at the end.
        let node = match slot.header.as_deref() {
            Some(label) => ViewNode::Column(vec![group_header_node(label), row]),
            None => row,
        };
        children.insert(slot.index, node);
        slot.index
    });

    // `active_index` indexes `sessions`; the items may carry an extra placeholder
    // row, which shifts every item at or after it down by one.
    let selected_item = active_index + usize::from(pending_row.is_some_and(|p| p <= active_index));
    // The active-row selection background is painted by the row's own nodes (a
    // trailing fill, full width) rather than by patching the item's whole rect —
    // which for a row carrying a prepended repo-group header would bleed the
    // background up onto that header. The header stays muted regardless.
    let tree = ViewNode::List {
        children,
        selected: show_selection.then_some(selected_item),
        scrollbar: false,
    };

    let inner = block.inner(area);
    frame.render_widget(block, area);
    let painted =
        super::plugin_pane::render_tree_rows(&tree, inner, &palette, &frames, frame.buffer_mut());
    // Indicators and hitboxes both come off the paint, so neither can describe a
    // window other than the one on screen.
    super::draw_clipped_indicators(
        frame.buffer_mut(),
        area,
        painted.clipped_above,
        painted.clipped_below,
    );

    painted
        .rows
        .into_iter()
        .filter_map(|hit| {
            // The painter numbers rows from one, in list space. Map each back to
            // its session index: the placeholder occupies a row but is not a
            // session, so rows after it are shifted by one. It gets no hitbox
            // itself — clicking a session that doesn't exist yet has nothing to
            // select — and selection indices stay a valid range over `sessions`.
            let item = hit.index - 1;
            let index = match pending_row {
                Some(p) if item == p => None,
                Some(p) if item > p => Some(item - 1),
                _ => Some(item),
            }?;
            (index < sessions.len()).then_some(super::RowHitbox {
                rect: hit.rect,
                index,
            })
        })
        .collect()
}

/// The placeholder row for a session still being created: a spinner, the name
/// we know it by so far, and the phase it's blocked on. Muted throughout — it is
/// not selectable, and shouldn't read as a live session.
///
/// The spinner is the frame the caller resolved rather than a declared motion:
/// this row is not a session, so it has no identity for a motion lease to key on
/// and the pane rebuilds it each paint anyway.
fn pending_spawn_node(
    pending: &crate::app::PendingSpawn,
    inner_width: usize,
    spinner: &str,
) -> ViewNode {
    let phase = pending.phase.short_label();
    // A spinner only while something is actually running; waiting on the user at
    // a modal gets a static glyph (distinct from the ○/● of a live session).
    let (glyph, glyph_token) = if pending.phase.is_working() {
        (spinner, StyleToken::StatusWorking)
    } else {
        ("\u{25cc}", StyleToken::Muted)
    };
    let mut runs = vec![
        ViewNode::token(format!(" {glyph} "), glyph_token),
        ViewNode::token(pending.label.clone(), StyleToken::Secondary),
    ];
    // Drop the phase text rather than overflow a narrow panel; the spinner and
    // the badge in the status row still carry the "in progress" signal.
    let used = 3 + pending.label.chars().count();
    if inner_width > used + phase.chars().count() + 2 {
        runs.push(ViewNode::token(format!("  {phase}"), StyleToken::Muted));
    }
    ViewNode::Line(runs)
}

/// Render the centered "no sessions yet" placeholder inside the given block.
fn render_empty_sessions(frame: &mut Frame, area: Rect, block: ratatui::widgets::Block) {
    let text = Paragraph::new(vec![
        Line::from(""),
        Line::from(Span::styled(
            "No sessions yet",
            Style::default().fg(Theme::text_muted()),
        )),
        Line::from(Span::styled(
            "Press Ctrl+N to create one",
            Style::default().fg(Theme::text_muted()),
        )),
    ])
    .block(block)
    .alignment(ratatui::layout::Alignment::Center);
    frame.render_widget(text, area);
}

/// One item of the rendered list: a repo-group header, or a session row.
///
/// A flat sequence rather than a header owning its rows, because that is the
/// shape both consumers want — the native pane prepends a header to the item
/// below it, and a plugin emits one list child per element.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionListItem {
    /// A repo-group header, above that group's first row.
    Header(String),
    /// A session.
    Session(SessionRow),
}

/// Separator between the session name and the inline agent status.
const AGENT_STATUS_SEPARATOR: &str = "  ";
/// Minimum columns the inline agent status needs to be worth showing.
const AGENT_STATUS_MIN_WIDTH: usize = 4;
/// Columns the status glyph occupies: the glyph with a space either side.
const STATUS_GLYPH_COLUMNS: usize = 3;
/// The remote-host mark, and the worktree mark, each with its trailing space.
const REMOTE_MARK: &str = "\u{21c5} ";
const WORKTREE_MARK: &str = "\u{2442} ";

/// Identity of the working spinner's motion within the pane.
///
/// Motion identity is `(pane, key, signature)`, so this string only has to be
/// unique inside one pane — a plugin reproducing this pane may use any key it
/// likes and animates identically. The bundled plugin uses this one so the two
/// trees compare equal, which is a property of the test rather than a name a
/// plugin has to know.
pub const SPINNER_MOTION_KEY: &str = "spinner";

/// Resolve the pane's rows for a column `inner_width` wide.
///
/// The **geometry** step: on top of [`resolve_rows`] it decides how much of the
/// agent's reported text fits after the name, and whether it fits at all. A plugin
/// never learns its pane's width, so this stays the kernel's —
/// [`session_list_tree`] below is the part a plugin reproduces.
pub fn resolve_items(inputs: &RowInputs<'_>, inner_width: usize) -> Vec<SessionListItem> {
    let mut items = Vec::with_capacity(inputs.sessions.len());
    for ((header, row), info) in resolve_rows(inputs).into_iter().zip(inputs.sessions) {
        if let Some(label) = header {
            items.push(SessionListItem::Header(label));
        }
        let status_text = agent_status_text(info)
            .and_then(|text| fit_status_text(&text, row_used_columns(&row), inner_width));
        items.push(SessionListItem::Session(SessionRow { status_text, ..row }));
    }
    items
}

/// Columns the row's runs occupy before the agent's text, counted in **chars**.
///
/// Chars rather than display columns because that is what the pre-port span walk
/// counted, and the fit below has to keep giving the same answer.
fn row_used_columns(row: &SessionRow) -> usize {
    STATUS_GLYPH_COLUMNS
        + prefix_text(row.depth, row.cross_group_child)
            .chars()
            .count()
        + if row.remote {
            REMOTE_MARK.chars().count()
        } else {
            0
        }
        + if row.worktree {
            WORKTREE_MARK.chars().count()
        } else {
            0
        }
        + row.name.chars().count()
}

/// Fit the agent's reported text into what is left of the column, or drop it.
///
/// Dropped rather than overflowed when fewer than [`AGENT_STATUS_MIN_WIDTH`]
/// columns remain: the coloured status glyph already carries the state, so a
/// two-column stub of the message is worth less than the name it would push out.
fn fit_status_text(text: &str, used_before: usize, inner_width: usize) -> Option<String> {
    let used = used_before + AGENT_STATUS_SEPARATOR.chars().count();
    let avail = inner_width.saturating_sub(used);
    (avail >= AGENT_STATUS_MIN_WIDTH).then(|| super::truncate_ellipsis(text, avail))
}

/// The pane's whole view tree: one list child per resolved item.
///
/// The **presentation** step, and it carries no geometry at all: glyphs, marks,
/// colour roles, the `selected > dimmed > role` precedence, the search emphasis,
/// and the animation of a working row. That is the part
/// `tests/bundled_session_list.rs` asserts a bundled Luau plugin reproduces
/// exactly.
///
/// The cursor is declared on the list so the *kernel* windows it, which is how a
/// pane that is never told its height scrolls (ADR-30). Since ADR-63 the native
/// pane paints this same tree through the same renderer, so the two window by one
/// rule rather than two.
pub fn session_list_tree(items: &[SessionListItem]) -> ViewNode {
    let children = session_list_items(items);
    // The cursor's index counts **items**, which is why the fold above has to
    // happen before this: a header is part of the row it heads, not a row of its
    // own.
    let selected = items
        .iter()
        .filter(|item| matches!(item, SessionListItem::Session(_)))
        .position(|item| matches!(item, SessionListItem::Session(row) if row.selected));
    ViewNode::selectable_list(children, selected)
}

/// The list's children: one per session row, with a repo-group header folded into
/// the row it heads.
///
/// A header is not a row of the list. It scrolls with its group's first session,
/// a click on it selects that session, and the window counts it as part of that
/// one item — which is what makes an index mean the same row in this pane and in
/// the plugin reproducing it. Expressible since ADR-61, which taught the kernel's
/// windowing rule to measure an item in rows.
fn session_list_items(items: &[SessionListItem]) -> Vec<ViewNode> {
    let mut children = Vec::with_capacity(items.len());
    let mut pending_header: Option<ViewNode> = None;
    for item in items {
        match item {
            SessionListItem::Header(label) => pending_header = Some(group_header_node(label)),
            SessionListItem::Session(row) => {
                let node = session_row_node(row);
                children.push(match pending_header.take() {
                    Some(header) => ViewNode::Column(vec![header, node]),
                    None => node,
                });
            }
        }
    }
    children
}

/// A full-width repo-group header: `── label ──────────`.
///
/// Always muted, and it never reflects selection — highlighting belongs to the
/// session rows alone, so selecting a group's first row must not light up its
/// header. Status lives on the rows, never here. The trailing rule is a fill: it
/// reaches the pane's edge, and a plugin is never told which column that is.
fn group_header_node(label: &str) -> ViewNode {
    ViewNode::Line(vec![
        ViewNode::token(format!("\u{2500}\u{2500} {label} "), StyleToken::Muted),
        ViewNode::Fill {
            glyph: '\u{2500}',
            style: token_style(StyleToken::Muted),
        },
    ])
}

/// A session row: status glyph, prefix marks, name, agent text — and, on the
/// cursor's row, a fill that carries the selection bar to the pane's edge.
fn session_row_node(row: &SessionRow) -> ViewNode {
    let mut runs = vec![status_glyph_node(row)];

    let prefix = prefix_text(row.depth, row.cross_group_child);
    if !prefix.is_empty() {
        // The tree prefix is muted whether or not the row matched a search: it is
        // structure, not content, so dimming it would say nothing.
        runs.push(ViewNode::styled(prefix, run_style(row, StyleToken::Muted)));
    }
    if row.remote {
        runs.push(ViewNode::styled(
            REMOTE_MARK,
            mark_style(row, StyleToken::Accent),
        ));
    }
    if row.worktree {
        runs.push(ViewNode::styled(
            WORKTREE_MARK,
            mark_style(row, StyleToken::Branch),
        ));
    }

    for (range, matched) in super::highlight::highlight_runs(&row.name, &row.name_matches) {
        let style = if matched {
            matched_style(row)
        } else {
            name_style(row)
        };
        runs.push(ViewNode::styled(&row.name[range], style));
    }

    if let Some(text) = row.status_text.as_deref() {
        // The separator takes no role of its own — two spaces carry no meaning —
        // so it inherits the row's selection appearance and nothing else.
        runs.push(ViewNode::styled(
            AGENT_STATUS_SEPARATOR,
            if row.selected {
                SELECTED_STYLE
            } else {
                TextStyle::default()
            },
        ));
        runs.push(ViewNode::styled(text, status_text_style(row)));
    }

    if row.selected {
        // The selection bar is the row, not the text on it: it reaches the pane's
        // right edge. A plugin cannot pad to a width it is never told, so the
        // residue is a fill.
        runs.push(ViewNode::Fill {
            glyph: ' ',
            style: SELECTED_STYLE,
        });
    }

    ViewNode::Line(runs)
}

/// The status glyph, animated while the session is working.
///
/// Ten braille frames at [`DEFAULT_FPS`], declared as motion so the **kernel**
/// drives them: the frames are this pane's choice and the clock is not. A
/// non-working status is a static glyph, which is also what frame 0 of the
/// animation is, so a reduced-motion pane still reads correctly.
fn status_glyph_node(row: &SessionRow) -> ViewNode {
    let style = glyph_style(row);
    if row.status == SessionStatus::Working {
        let frames = super::SPINNER_FRAMES
            .iter()
            .map(|f| ViewNode::styled(format!(" {f} "), style))
            .collect();
        return ViewNode::Motion {
            key: SPINNER_MOTION_KEY.to_string(),
            keyed_by_id: true,
            motion: Motion::cycle(frames, DEFAULT_FPS, true),
        };
    }
    ViewNode::styled(format!(" {} ", row.status.icon()), style)
}

/// The tree prefix between the status glyph and the name: `└ ` per level of
/// nesting, or `↳ ` for a child whose parent renders in another group.
fn prefix_text(depth: u8, cross_group_child: bool) -> String {
    if depth > 0 {
        format!("{}\u{2514} ", "  ".repeat(depth as usize - 1))
    } else if cross_group_child {
        "\u{21b3} ".to_string()
    } else {
        String::new()
    }
}

/// How every run of the cursor's row is drawn.
///
/// One appearance for the whole row, bold, in the theme's selection pair: the
/// row's own colour roles are *replaced* rather than layered under it, which is
/// what makes the bar read as one bar.
const SELECTED_STYLE: TextStyle = TextStyle {
    token: None,
    bold: true,
    dim: false,
    underline: false,
    selected: true,
    tint: None,
    ellipsize: false,
};

/// A colour role with no emphasis — the shape most runs are.
const fn token_style(token: StyleToken) -> TextStyle {
    TextStyle {
        token: Some(token),
        bold: false,
        dim: false,
        underline: false,
        selected: false,
        tint: None,
        // The session list fits its own names in `resolve_rows`; adopting the
        // declaration is that pane's handover's business (ADR-52).
        ellipsize: false,
    }
}

/// A run in `token`, unless the row's selection overrides it.
fn run_style(row: &SessionRow, token: StyleToken) -> TextStyle {
    if row.selected {
        SELECTED_STYLE
    } else {
        token_style(token)
    }
}

/// A prefix mark: muted when a search dimmed the row, otherwise its own role.
fn mark_style(row: &SessionRow, token: StyleToken) -> TextStyle {
    run_style(row, if row.dimmed { StyleToken::Muted } else { token })
}

/// The status glyph: its status's role, or muted on a dimmed row.
fn glyph_style(row: &SessionRow) -> TextStyle {
    mark_style(row, StyleToken::for_status(row.status))
}

/// The session name: the theme's primary text, muted on a dimmed row.
fn name_style(row: &SessionRow) -> TextStyle {
    if row.selected {
        SELECTED_STYLE
    } else if row.dimmed {
        token_style(StyleToken::Muted)
    } else {
        // No token: the theme's primary foreground, which is what an ordinary row
        // has always been.
        TextStyle::default()
    }
}

/// A character the search matched: accent, bold and underlined — and on the
/// cursor's row, underlined over the selection pair, since the bar owns the
/// colour there.
fn matched_style(row: &SessionRow) -> TextStyle {
    if row.selected {
        TextStyle {
            underline: true,
            ..SELECTED_STYLE
        }
    } else {
        TextStyle {
            token: Some(StyleToken::Accent),
            bold: true,
            underline: true,
            ..TextStyle::default()
        }
    }
}

/// The agent's text after the name: an attention notification keeps the status
/// colour (the same role as the glyph) so it stands out; ordinary activity text
/// is muted so the name stays the anchor.
fn status_text_style(row: &SessionRow) -> TextStyle {
    if row.status == SessionStatus::Blocked {
        glyph_style(row)
    } else {
        run_style(row, StyleToken::Muted)
    }
}

/// The frame table the native pane paints its rows with.
///
/// The kernel's clock reaches the renderer as plain data — the same channel a
/// plugin's declared motion is resolved through — which is why `ui` can draw an
/// animation without any path back to a VM.
fn spinner_frames(spinner_frame: usize) -> FrameTable {
    let mut frames = FrameTable::default();
    frames.set(
        SPINNER_MOTION_KEY,
        spinner_frame % super::SPINNER_FRAMES.len(),
    );
    frames
}

/// The pre-port span builders, retained as the oracle that pins the tree to what
/// the pane drew before it: `legacy_session_line` and `legacy_group_header_line`
/// are the code this module shipped until the rows became view-tree nodes, and
/// `the_tree_paints_what_the_span_builder_painted` asserts the two agree cell for
/// cell.
#[cfg(test)]
fn legacy_group_header_line(label: &str, inner_width: usize) -> Line<'static> {
    let mut text = format!("\u{2500}\u{2500} {label} ");
    let used = text.chars().count();
    if inner_width > used {
        text.push_str(&"\u{2500}".repeat(inner_width - used));
    }
    Line::from(Span::styled(text, Style::default().fg(Theme::text_muted())))
}

/// Style for the session name span, as the pre-port pane resolved it.
#[cfg(test)]
fn legacy_name_style(is_active: bool, is_dimmed: bool) -> Style {
    if is_dimmed {
        Style::default().fg(Theme::text_muted())
    } else if is_active {
        Style::default()
            .fg(Theme::selection_fg())
            .add_modifier(Modifier::BOLD)
    } else {
        Theme::normal_item()
    }
}

/// Style for a prefix mark, as the pre-port pane resolved it.
#[cfg(test)]
fn legacy_mark_style(is_dimmed: bool, color: fn() -> ratatui::style::Color) -> Style {
    if is_dimmed {
        Style::default().fg(Theme::text_muted())
    } else {
        Style::default().fg(color())
    }
}

/// The row line, as the pre-port pane built it.
#[cfg(test)]
fn legacy_session_line(row: &SessionRow, inner_width: usize, spinner: &str) -> Line<'static> {
    let name_style = legacy_name_style(row.selected, row.dimmed);
    let status_style = if row.dimmed {
        Style::default().fg(Theme::text_muted())
    } else {
        Style::default().fg(super::status_color(row.status))
    };

    let mut spans = vec![Span::styled(
        format!(" {} ", super::status_glyph(row.status, spinner)),
        status_style,
    )];

    let tree_style = Style::default().fg(Theme::text_muted());
    let prefix = prefix_text(row.depth, row.cross_group_child);
    if !prefix.is_empty() {
        spans.push(Span::styled(prefix, tree_style));
    }
    if row.remote {
        spans.push(Span::styled(
            REMOTE_MARK,
            legacy_mark_style(row.dimmed, Theme::accent),
        ));
    }
    if row.worktree {
        spans.push(Span::styled(
            WORKTREE_MARK,
            legacy_mark_style(row.dimmed, Theme::branch_name),
        ));
    }

    super::highlight::append_highlighted(
        &mut spans,
        &row.name,
        (!row.name_matches.is_empty()).then_some(row.name_matches.as_slice()),
        name_style,
    );

    if let Some(text) = row.status_text.as_deref() {
        let agent_status_style = if row.status == SessionStatus::Blocked {
            status_style
        } else {
            Style::default().fg(Theme::text_muted())
        };
        spans.push(Span::raw(AGENT_STATUS_SEPARATOR));
        spans.push(Span::styled(text.to_string(), agent_status_style));
    }

    let mut line = Line::from(spans);
    if row.selected {
        let selection_style = Style::default()
            .bg(Theme::selection_bg())
            .fg(Theme::selection_fg())
            .add_modifier(Modifier::BOLD);
        let pad = inner_width.saturating_sub(line.width());
        if pad > 0 {
            line.spans.push(Span::raw(" ".repeat(pad)));
        }
        for span in &mut line.spans {
            span.style = span.style.patch(selection_style);
        }
    }
    // The pre-port line borrowed its name from the session; the oracle owns it so
    // a test can hold it past the row.
    Line::from(
        line.spans
            .into_iter()
            .map(|s| Span::styled(s.content.into_owned(), s.style))
            .collect::<Vec<_>>(),
    )
}

#[cfg(test)]
mod tests {
    use ratatui::{backend::TestBackend, buffer::Buffer, Terminal};

    use super::super::highlight::append_highlighted as append_name_spans;
    use super::super::highlight::highlighted_spans as build_highlighted_spans;
    use super::*;
    // The model these drawing tests build their fixtures from now lives beside the
    // model itself; the pane's own tests still need it to produce a rendered order.
    use crate::session::session_list::{compute_session_order, OrderedSessions, NO_REPO_GROUP};
    use crate::session::SessionStatus;

    // --- build_highlighted_spans ---

    #[test]
    fn highlighted_spans_basic() {
        let style = Style::default().fg(Theme::text_primary());
        let spans = build_highlighted_spans("foo-bar", &[0, 4], style);
        assert_eq!(spans.len(), 4);
        assert_eq!(spans[0].content, "f");
        assert_eq!(spans[1].content, "oo-");
        assert_eq!(spans[2].content, "b");
        assert_eq!(spans[3].content, "ar");
    }

    #[test]
    fn highlighted_spans_empty_positions() {
        let style = Style::default().fg(Theme::text_primary());
        let spans = build_highlighted_spans("hello", &[], style);
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].content, "hello");
    }

    #[test]
    fn highlighted_spans_all_chars() {
        let style = Style::default().fg(Theme::text_primary());
        let spans = build_highlighted_spans("abc", &[0, 1, 2], style);
        // All chars highlighted, no normal spans between them.
        assert_eq!(spans.len(), 3);
        assert_eq!(spans[0].content, "a");
        assert_eq!(spans[1].content, "b");
        assert_eq!(spans[2].content, "c");
    }

    // --- append_name_spans ---

    #[test]
    fn append_name_spans_no_match_positions() {
        let style = Style::default().fg(Theme::text_primary());
        let mut spans = Vec::new();
        append_name_spans(&mut spans, "hello", None, style);
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].content, "hello");
    }

    #[test]
    fn append_name_spans_empty_positions() {
        let style = Style::default().fg(Theme::text_primary());
        let mut spans = Vec::new();
        append_name_spans(&mut spans, "hello", Some(&[]), style);
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].content, "hello");
    }

    #[test]
    fn append_name_spans_with_highlights() {
        let style = Style::default().fg(Theme::text_primary());
        let mut spans = vec![Span::raw("prefix ")];
        append_name_spans(&mut spans, "foo-bar", Some(&[0, 4]), style);
        assert_eq!(spans.len(), 5);
        assert_eq!(spans[0].content, "prefix ");
        assert_eq!(spans[1].content, "f");
        assert_eq!(spans[2].content, "oo-");
        assert_eq!(spans[3].content, "b");
        assert_eq!(spans[4].content, "ar");
    }

    // --- OrderedSessions ---

    fn info(name: &str) -> SessionInfo {
        SessionInfo::new(name.to_string())
    }

    #[test]
    fn session_section_hitboxes_span_group_headers() {
        // Two sessions in one "(no repo)" group: the first item is 2 lines
        // tall (header + row) and its hitbox spans both; the second is 1.
        let n1 = info("n1");
        let n2 = info("n2");
        let backend = ratatui::backend::TestBackend::new(30, 12);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        let mut hitboxes = Vec::new();
        terminal
            .draw(|f| {
                let sessions = vec![&n1, &n2];
                let order = compute_session_order(&sessions);
                let ordered = OrderedSessions::from_order(&sessions, &order, &[None, None], 0);
                hitboxes = render_left_panel(
                    f,
                    Rect::new(0, 0, 30, 12),
                    &LeftPanelState {
                        sessions: &ordered.sessions,
                        active_session: ordered.active_index,
                        show_selection: true,
                        session_focus: FocusLevel::Focused,
                        session_match_positions: &ordered.match_positions,
                        session_search_active: false,
                        headers: ordered.headers,
                        depths: ordered.depths,
                        spinner_frame: 0,
                        pending_spawn: None,
                    },
                );
            })
            .unwrap();
        assert_eq!(hitboxes.len(), 2);
        assert_eq!(hitboxes[0].rect, Rect::new(1, 1, 28, 2));
        assert_eq!(hitboxes[0].index, 0);
        assert_eq!(hitboxes[1].rect, Rect::new(1, 3, 28, 1));
        assert_eq!(hitboxes[1].index, 1);
    }

    /// Render the pane at `w x h` with `n` single-group sessions, the cursor on
    /// `active`, and hand back the painted cells plus the hitboxes.
    fn draw_pane(
        infos: &[SessionInfo],
        active: usize,
        w: u16,
        h: u16,
    ) -> (ratatui::buffer::Buffer, Vec<super::super::RowHitbox>) {
        let backend = ratatui::backend::TestBackend::new(w, h);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        let mut hitboxes = Vec::new();
        terminal
            .draw(|f| {
                let sessions: Vec<&SessionInfo> = infos.iter().collect();
                let order = compute_session_order(&sessions);
                let no_matches = vec![None; sessions.len()];
                let ordered = OrderedSessions::from_order(&sessions, &order, &no_matches, active);
                hitboxes = render_left_panel(
                    f,
                    Rect::new(0, 0, w, h),
                    &LeftPanelState {
                        sessions: &ordered.sessions,
                        active_session: ordered.active_index,
                        show_selection: true,
                        session_focus: FocusLevel::Focused,
                        session_match_positions: &ordered.match_positions,
                        session_search_active: false,
                        headers: ordered.headers,
                        depths: ordered.depths,
                        spinner_frame: 0,
                        pending_spawn: None,
                    },
                );
            })
            .unwrap();
        (terminal.backend().buffer().clone(), hitboxes)
    }

    fn row_text(buf: &ratatui::buffer::Buffer, y: u16) -> String {
        (0..buf.area.width)
            .map(|x| buf[(x, y)].symbol())
            .collect::<String>()
    }

    /// The clipped-row indicators are the window's, and the window is the kernel's
    /// shared rule — so what the border reports is what `visible_item_window`
    /// left off screen, counted in **items** (a header travels with its row).
    #[test]
    fn the_border_reports_the_window_the_shared_rule_chose() {
        // Twelve sessions in one group, so item 0 is two lines and the rest one:
        // thirteen lines into six rows of content.
        let infos: Vec<SessionInfo> = (0..12).map(|i| info(&format!("s{i}"))).collect();
        let (buf, hitboxes) = draw_pane(&infos, 11, 30, 8);

        let heights = |i: usize| usize::from(i == 0) + 1;
        let (start, end) = super::super::visible_item_window(12, heights, 11, 6);
        assert!(start > 0 && end == 12, "the fixture must overflow above");

        assert!(
            row_text(&buf, 0).contains(&format!("\u{25b2} {start} ")),
            "top border should report the items above: {:?}",
            row_text(&buf, 0)
        );
        assert!(
            !row_text(&buf, 7).contains('\u{25bc}'),
            "nothing is clipped below when the window reaches the tail: {:?}",
            row_text(&buf, 7)
        );
        // And the hitboxes describe the same slice, in session space.
        assert_eq!(hitboxes.first().map(|h| h.index), Some(start));
        assert_eq!(hitboxes.last().map(|h| h.index), Some(11));
    }

    /// The cursor is always on screen, whichever end of a long list it is at —
    /// the property the convergence must not lose.
    #[test]
    fn the_cursor_is_on_screen_at_either_end() {
        let infos: Vec<SessionInfo> = (0..20).map(|i| info(&format!("s{i}"))).collect();
        for active in [0usize, 9, 19] {
            let (_, hitboxes) = draw_pane(&infos, active, 30, 8);
            assert!(
                hitboxes.iter().any(|h| h.index == active),
                "cursor {active} was windowed off screen: {hitboxes:?}"
            );
        }
    }

    // --- line builder ---

    /// Inner width wide enough that nothing truncates in these tests.
    const WIDE: usize = 80;

    fn line_text(line: &Line) -> String {
        line.spans.iter().map(|s| s.content.as_ref()).collect()
    }

    /// The row the pane resolves for one session at `inner_width`.
    ///
    /// Built directly rather than through [`resolve_items`] so a test can force
    /// the two facts that need a *set* of sessions to arise — a dimmed row needs a
    /// running search that missed it, a cross-group child needs its parent
    /// elsewhere on screen — while still going through the real fit.
    #[allow(clippy::too_many_arguments)]
    fn test_row(
        info: &SessionInfo,
        matches: Option<&SessionMatch>,
        selected: bool,
        dimmed: bool,
        depth: u8,
        cross_group_child: bool,
        inner_width: usize,
    ) -> SessionRow {
        let row = SessionRow {
            status: info.status,
            name: info.name.clone(),
            name_matches: matches.map(|m| m.name.clone()).unwrap_or_default(),
            depth,
            cross_group_child,
            remote: info.remote_host.is_some(),
            worktree: !info.worktrees.is_empty(),
            selected,
            dimmed,
            status_text: None,
        };
        let status_text = agent_status_text(info)
            .and_then(|t| fit_status_text(&t, row_used_columns(&row), inner_width));
        SessionRow { status_text, ..row }
    }

    /// Paint a node the way the pane paints its rows, as an owned line.
    fn paint(node: &ViewNode, inner_width: usize, spinner_frame: usize) -> Line<'static> {
        Line::from(
            super::super::plugin_pane::line_spans(
                node,
                inner_width as u16,
                &super::super::theme::current(),
                &spinner_frames(spinner_frame),
            )
            .into_iter()
            .map(|s| Span::styled(s.content.into_owned(), s.style))
            .collect::<Vec<_>>(),
        )
    }

    /// The line the pane paints for one session — the replacement for the
    /// pre-port `build_session_line`, through the view tree.
    #[allow(clippy::too_many_arguments)]
    fn session_line(
        info: &SessionInfo,
        matches: Option<&SessionMatch>,
        selected: bool,
        dimmed: bool,
        depth: u8,
        cross_group_child: bool,
        inner_width: usize,
    ) -> Line<'static> {
        let row = test_row(
            info,
            matches,
            selected,
            dimmed,
            depth,
            cross_group_child,
            inner_width,
        );
        paint(&session_row_node(&row), inner_width, 0)
    }

    /// The line the pane paints for a repo-group header.
    fn header_line(label: &str, inner_width: usize) -> Line<'static> {
        paint(&group_header_node(label), inner_width, 0)
    }

    // ── the pre-port renderer, retained as an oracle ────────────────────────
    //
    // `legacy_session_line` / `legacy_group_header_line` (above, at module scope)
    // are the span builders this pane used before its rows became view-tree nodes.
    // They are kept so the port is a *checked* refactor: the differential test
    // below asserts the tree paints the same cells, which is the only way to know
    // that moving the pane onto the tree changed nothing a user sees.

    /// Every row appearance the pane has, as `(name, row)` pairs.
    fn differential_rows() -> Vec<(&'static str, SessionRow)> {
        let base = |name: &str, status| SessionRow {
            status,
            name: name.to_string(),
            name_matches: Vec::new(),
            depth: 0,
            cross_group_child: false,
            remote: false,
            worktree: false,
            selected: false,
            dimmed: false,
            status_text: None,
        };
        vec![
            ("idle", base("plain", SessionStatus::Idle)),
            ("done", base("finished", SessionStatus::Done)),
            ("error", base("crashed", SessionStatus::Error)),
            ("unreachable", base("offline", SessionStatus::Unreachable)),
            ("working", base("busy", SessionStatus::Working)),
            (
                "blocked with a notification",
                SessionRow {
                    status_text: Some("needs approval".to_string()),
                    ..base("waiting", SessionStatus::Blocked)
                },
            ),
            (
                "activity text",
                SessionRow {
                    status_text: Some("Compacting".to_string()),
                    ..base("streaming", SessionStatus::Working)
                },
            ),
            (
                "nested child",
                SessionRow {
                    depth: 2,
                    ..base("worker", SessionStatus::Idle)
                },
            ),
            (
                "cross-group child",
                SessionRow {
                    cross_group_child: true,
                    ..base("elsewhere", SessionStatus::Idle)
                },
            ),
            (
                "remote and worktree",
                SessionRow {
                    remote: true,
                    worktree: true,
                    ..base("devbox", SessionStatus::Idle)
                },
            ),
            (
                "selected",
                SessionRow {
                    selected: true,
                    ..base("current", SessionStatus::Working)
                },
            ),
            (
                "selected with everything on it",
                SessionRow {
                    selected: true,
                    remote: true,
                    worktree: true,
                    depth: 1,
                    name_matches: vec![0, 2],
                    status_text: Some("Thinking".to_string()),
                    ..base("hilite", SessionStatus::Blocked)
                },
            ),
            (
                "dimmed by a search",
                SessionRow {
                    dimmed: true,
                    remote: true,
                    worktree: true,
                    status_text: Some("Compacting".to_string()),
                    ..base("filtered", SessionStatus::Working)
                },
            ),
            (
                "matched characters",
                SessionRow {
                    name_matches: vec![0, 4],
                    ..base("foo-bar", SessionStatus::Idle)
                },
            ),
            (
                "matched multi-byte characters",
                SessionRow {
                    name_matches: vec![1],
                    ..base("héllo", SessionStatus::Idle)
                },
            ),
        ]
    }

    /// Paint one row both ways: through the view tree, and through the pre-port
    /// span builder.
    fn render_both(row: &SessionRow, w: u16, spinner_frame: usize) -> (Buffer, Buffer) {
        let area = Rect::new(0, 0, w, 1);
        let inner_width = w as usize;

        let mut tree_buf = Buffer::empty(area);
        let mut terminal = Terminal::new(TestBackend::new(w, 1)).unwrap();
        terminal
            .draw(|f| {
                f.render_widget(
                    Paragraph::new(paint(&session_row_node(row), inner_width, spinner_frame)),
                    area,
                )
            })
            .unwrap();
        tree_buf.merge(terminal.backend().buffer());

        let mut legacy_buf = Buffer::empty(area);
        let mut legacy_terminal = Terminal::new(TestBackend::new(w, 1)).unwrap();
        let spinner = super::super::SPINNER_FRAMES[spinner_frame];
        legacy_terminal
            .draw(|f| {
                f.render_widget(
                    Paragraph::new(legacy_session_line(row, inner_width, spinner)),
                    area,
                )
            })
            .unwrap();
        legacy_buf.merge(legacy_terminal.backend().buffer());
        (tree_buf, legacy_buf)
    }

    /// Assert two paints agree on every cell that carries ink.
    ///
    /// One latitude, and it is the whole difference between the pre-port pane and
    /// this one: the two-space separator before the agent's text was a
    /// `Span::raw`, which leaves the foreground **unset**, while a token-less run
    /// resolves the theme's primary foreground. A space carries no ink, so the
    /// cells are indistinguishable on screen — but they are not equal `Cell`s, and
    /// a comparison that ignored the difference everywhere would also ignore a
    /// real one. So a blank cell may differ in its foreground and in nothing else:
    /// background, modifiers and underline must match on every cell, and every
    /// cell with a glyph in it must match exactly.
    fn assert_same_ink(tree: &Buffer, legacy: &Buffer, context: &str) {
        assert_eq!(tree.area, legacy.area, "{context}: different areas");
        for (i, (a, b)) in tree.content.iter().zip(legacy.content.iter()).enumerate() {
            assert_eq!(a.symbol(), b.symbol(), "{context}: cell {i} glyph");
            assert_eq!(a.bg, b.bg, "{context}: cell {i} background");
            assert_eq!(a.modifier, b.modifier, "{context}: cell {i} modifiers");
            assert_eq!(a.underline_color, b.underline_color, "{context}: cell {i}");
            if a.symbol() != " " {
                assert_eq!(a.fg, b.fg, "{context}: cell {i} foreground");
            }
        }
    }

    /// The port's own guarantee: the tree paints the row the pre-port span builder
    /// painted — every inked cell exactly, at a width where nothing is fitted and
    /// at one where the selection bar has little room left.
    #[test]
    fn the_tree_paints_what_the_span_builder_painted() {
        for (name, row) in differential_rows() {
            for width in [40u16, 18] {
                let (tree, legacy) = render_both(&row, width, 0);
                assert_same_ink(&tree, &legacy, &format!("`{name}` at width {width}"));
            }
        }
    }

    /// The one latitude `assert_same_ink` grants, pinned so it cannot widen
    /// unnoticed: it applies to the separator's blanks and to nothing else, and it
    /// is a foreground on a cell with no glyph in it.
    #[test]
    fn the_only_divergence_is_a_blank_cells_foreground() {
        let row = SessionRow {
            status_text: Some("Compacting".to_string()),
            ..differential_rows().remove(0).1
        };
        let (tree, legacy) = render_both(&row, 40, 0);
        let differing: Vec<usize> = tree
            .content
            .iter()
            .zip(legacy.content.iter())
            .enumerate()
            .filter(|(_, (a, b))| a != b)
            .map(|(i, _)| i)
            .collect();
        assert!(
            !differing.is_empty(),
            "if the separator no longer differs, delete the latitude in \
             `assert_same_ink` rather than leaving it granted"
        );
        for i in differing {
            assert_eq!(tree.content[i].symbol(), " ", "cell {i} carries ink");
            assert_eq!(tree.content[i].bg, legacy.content[i].bg);
            assert_eq!(tree.content[i].modifier, legacy.content[i].modifier);
        }
    }

    /// The animation is not exempt from the guarantee above: every frame of the
    /// spinner has to paint what the pre-port pane painted at that frame.
    #[test]
    fn every_spinner_frame_paints_what_the_span_builder_painted() {
        let row = SessionRow {
            selected: false,
            ..differential_rows()
                .into_iter()
                .find(|(name, _)| *name == "working")
                .expect("a working row")
                .1
        };
        for frame in 0..super::super::SPINNER_FRAMES.len() {
            let (tree, legacy) = render_both(&row, 40, frame);
            assert_same_ink(&tree, &legacy, &format!("frame {frame}"));
        }
    }

    /// A group header paints identically too — its trailing rule is a fill, and a
    /// fill resolving a different residue would show up here.
    #[test]
    fn the_header_tree_paints_what_the_span_builder_painted() {
        for width in [40usize, 12, 6] {
            let area = Rect::new(0, 0, width as u16, 1);
            let mut tree_buf = Buffer::empty(area);
            let mut terminal = Terminal::new(TestBackend::new(width as u16, 1)).unwrap();
            terminal
                .draw(|f| f.render_widget(Paragraph::new(header_line("repo", width)), area))
                .unwrap();
            tree_buf.merge(terminal.backend().buffer());

            let mut legacy_buf = Buffer::empty(area);
            let mut legacy_terminal = Terminal::new(TestBackend::new(width as u16, 1)).unwrap();
            legacy_terminal
                .draw(|f| {
                    f.render_widget(
                        Paragraph::new(legacy_group_header_line("repo", width)),
                        area,
                    )
                })
                .unwrap();
            legacy_buf.merge(legacy_terminal.backend().buffer());
            assert_same_ink(&tree_buf, &legacy_buf, &format!("header at width {width}"));
        }
    }

    /// The tree must describe content only: the fit lives in [`resolve_items`], so
    /// nothing in a node may depend on a width — which shows up as a run padded to
    /// fill one. A trailing [`ViewNode::Fill`] is the exception, and the only one:
    /// it is a *declaration* that the host resolves a residue, which is how the
    /// selection bar reaches the edge without the tree knowing where the edge is.
    #[test]
    fn the_tree_carries_no_geometry() {
        fn assert_no_padding(node: &ViewNode) {
            if let ViewNode::Text { content, .. } = node {
                assert!(
                    !content.ends_with("   "),
                    "a padded run means the tree resolved a width: {content:?}"
                );
            }
            node.children().iter().for_each(assert_no_padding);
        }
        for (_, row) in differential_rows() {
            assert_no_padding(&session_row_node(&row));
        }
        assert_no_padding(&group_header_node("a repo group"));
    }

    /// A working row animates through a *declared* motion, keyed so an identical
    /// re-push keeps its phase, at the rate thurbox's own spinner has always run
    /// at. Asserted on the node rather than on a frame, because the whole point of
    /// the declaration is that the kernel owns the clock.
    #[test]
    fn a_working_row_declares_its_animation() {
        let row = SessionRow {
            status: SessionStatus::Working,
            ..differential_rows().remove(0).1
        };
        let node = session_row_node(&row);
        let motion = node
            .children()
            .iter()
            .find_map(|child| match child {
                ViewNode::Motion {
                    key,
                    keyed_by_id,
                    motion,
                } => Some((key.clone(), *keyed_by_id, motion.clone())),
                _ => None,
            })
            .expect("a working row's glyph is a motion node");
        assert_eq!(motion.0, SPINNER_MOTION_KEY);
        assert!(motion.1, "a structural key would restart on a shape change");
        assert_eq!(motion.2.fps, DEFAULT_FPS);
        assert_eq!(motion.2.frames().len(), super::super::SPINNER_FRAMES.len());
        assert!(motion.2.repeats());

        // A row at rest declares none, so no lease is taken for a static glyph.
        let resting = SessionRow {
            status: SessionStatus::Idle,
            ..row
        };
        assert!(!session_row_node(&resting)
            .children()
            .iter()
            .any(|c| matches!(c, ViewNode::Motion { .. })));
    }

    /// The fit is the pane's only use of a width, so both things it does with one
    /// — shorten the agent's text, or drop it — are asserted directly.
    #[test]
    fn the_agent_text_is_fitted_or_dropped() {
        let mut info = info("session");
        info.agent_activity = Some("a very long activity line indeed".to_string());
        let wide = test_row(&info, None, false, false, 0, false, 80);
        assert_eq!(
            wide.status_text.as_deref(),
            Some("a very long activity line indeed"),
            "a wide column fits the text whole"
        );

        let narrow = test_row(&info, None, false, false, 0, false, 24);
        let narrow_text = narrow.status_text.expect("some of it still fits");
        assert!(narrow_text.ends_with('…'), "got {narrow_text:?}");
        assert!(narrow_text.chars().count() <= 24 - (" session".len() + 4));

        // Under four columns of room the text is dropped rather than stubbed: the
        // coloured glyph already carries the state.
        assert!(test_row(&info, None, false, false, 0, false, 14)
            .status_text
            .is_none());
    }

    #[test]
    fn line_shows_worktree_glyph_when_worktree_present() {
        let mut s = info("feature");
        s.worktrees.push(crate::session::WorktreeInfo {
            repo_path: std::path::PathBuf::from("/repos/thurbox"),
            worktree_path: std::path::PathBuf::from("/tmp/wt/feat"),
            branch: "feat".to_string(),
        });
        let line = session_line(&s, None, false, false, 0, false, WIDE);
        assert!(line_text(&line).contains('\u{2442}'));
    }

    #[test]
    fn line_no_worktree_glyph_for_plain_session() {
        let s = info("plain");
        let line = session_line(&s, None, false, false, 0, false, WIDE);
        assert!(!line_text(&line).contains('\u{2442}'));
    }

    #[test]
    fn line_shows_remote_glyph_when_remote_host_present() {
        let mut s = info("remote");
        s.remote_host = Some("devbox".to_string());
        let line = session_line(&s, None, false, false, 0, false, WIDE);
        assert!(line_text(&line).contains('\u{21c5}'));
    }

    #[test]
    fn line_no_remote_glyph_for_local_session() {
        let s = info("local");
        let line = session_line(&s, None, false, false, 0, false, WIDE);
        assert!(!line_text(&line).contains('\u{21c5}'));
    }

    #[test]
    fn line_shows_tree_prefix_for_nested_child() {
        let s = info("worker");
        let line = session_line(&s, None, false, false, 1, false, WIDE);
        assert!(line_text(&line).contains('\u{2514}')); // └
    }

    #[test]
    fn line_shows_arrow_for_cross_group_child() {
        let s = info("worker");
        let line = session_line(&s, None, false, false, 0, true, WIDE);
        let text = line_text(&line);
        assert!(text.contains('\u{21b3}')); // ↳
        assert!(!text.contains('\u{2514}'));
    }

    #[test]
    fn line_carries_no_status_text_without_agent_report() {
        let mut s = info("quiet");
        s.status = SessionStatus::Done;
        let text = line_text(&session_line(&s, None, false, false, 0, false, WIDE));
        assert!(!text.contains("Waiting"));
        assert!(text.trim_end().ends_with("quiet"));
    }

    #[test]
    fn line_appends_agent_activity_after_name() {
        let mut s = info("busy");
        s.status = SessionStatus::Working;
        s.agent_activity = Some("Compacting conversation".to_string());
        let text = line_text(&session_line(&s, None, false, false, 0, false, WIDE));
        assert!(text.contains("busy  Compacting conversation"));
    }

    #[test]
    fn active_line_paints_selection_background_to_full_width() {
        let s = info("active");
        let line = session_line(&s, None, true, false, 0, false, WIDE);
        // The bar is painted on the row itself (every span), so it can't bleed
        // onto a prepended group header, and it fills the full inner width.
        assert!(line
            .spans
            .iter()
            .all(|sp| sp.style.bg == Some(Theme::selection_bg())));
        assert_eq!(line.width(), WIDE);
    }

    #[test]
    fn active_line_tints_spans_without_padding_when_too_narrow() {
        // Inner width smaller than the rendered row: nothing to pad, but the
        // selection background must still cover the spans (and not panic).
        let s = info("a-fairly-long-session-name");
        let line = session_line(&s, None, true, false, 0, false, 4);
        assert!(line
            .spans
            .iter()
            .all(|sp| sp.style.bg == Some(Theme::selection_bg())));
    }

    #[test]
    fn inactive_line_has_no_selection_background() {
        let s = info("idle");
        let line = session_line(&s, None, false, false, 0, false, WIDE);
        assert!(line.spans.iter().all(|sp| sp.style.bg.is_none()));
    }

    #[test]
    fn group_header_is_always_muted_without_background() {
        // The header never reflects selection: selecting a group's first row
        // must not light up its header. Always muted, never a background tint.
        let line = header_line("repo", WIDE);
        assert!(line.spans.iter().all(|sp| sp.style.bg.is_none()));
        assert_eq!(line.spans[0].style.fg, Some(Theme::text_muted()));
        assert!(!line.spans[0].style.add_modifier.contains(Modifier::BOLD));
    }

    #[test]
    fn group_header_starts_with_the_rule_and_carries_no_status_glyph() {
        // Status belongs to the session rows; the header is a plain rule.
        let text = line_text(&header_line("repo", WIDE));
        assert!(text.starts_with("\u{2500}\u{2500} repo "), "{text:?}");
        for glyph in ["\u{25c6}", "\u{25cf}", "\u{25cb}", "\u{2717}", "\u{2298}"]
            .iter()
            .chain(super::super::SPINNER_FRAMES.iter())
        {
            assert!(!text.contains(glyph), "{text:?} contains {glyph:?}");
        }
    }

    #[test]
    fn line_attention_appends_notification() {
        let mut s = info("attn");
        s.status = SessionStatus::Blocked;
        s.notification = Some("Review this diff".to_string());
        let text = line_text(&session_line(&s, None, false, false, 0, false, WIDE));
        assert!(text.contains("attn  Review this diff"));
    }

    #[test]
    fn line_truncates_agent_status_with_ellipsis() {
        let mut s = info("busy");
        s.agent_activity = Some("a very long activity title that cannot fit".to_string());
        let line = session_line(&s, None, false, false, 0, false, 20);
        let text = line_text(&line);
        assert!(text.chars().count() <= 20);
        assert!(text.ends_with('\u{2026}'));
    }

    #[test]
    fn line_exact_fit_agent_status_is_not_truncated() {
        let mut s = info("busy");
        s.agent_activity = Some("Ready".to_string());
        // used = " ● " (3) + "busy" (4) + separator (2) = 9; "Ready" fits exactly.
        let text = line_text(&session_line(&s, None, false, false, 0, false, 14));
        assert!(text.ends_with("busy  Ready"));
        assert!(!text.contains('\u{2026}'));
    }

    #[test]
    fn line_agent_status_min_width_boundary() {
        let mut s = info("busy");
        s.agent_activity = Some("Ready".to_string());
        // avail = AGENT_STATUS_MIN_WIDTH → shown truncated; one column less → skipped.
        let shown = line_text(&session_line(&s, None, false, false, 0, false, 13));
        assert!(shown.ends_with("Rea\u{2026}"));
        let skipped = line_text(&session_line(&s, None, false, false, 0, false, 12));
        assert!(skipped.trim_end().ends_with("busy"));
    }

    #[test]
    fn line_skips_agent_status_when_no_room() {
        let mut s = info("a-rather-long-session-name");
        s.agent_activity = Some("activity".to_string());
        let text = line_text(&session_line(&s, None, false, false, 0, false, 30));
        assert!(!text.contains("activity"));
        assert!(text.trim_end().ends_with("a-rather-long-session-name"));
    }

    // --- compute_session_order (grouping + manual order) ---

    fn info_repo(name: &str, repo: &str, status: SessionStatus) -> SessionInfo {
        info_repos(name, &[repo], status)
    }

    fn info_repos(name: &str, repos: &[&str], status: SessionStatus) -> SessionInfo {
        let mut s = SessionInfo::new(name.to_string());
        s.status = status;
        s.repo_display_names = repos.iter().map(|r| r.to_string()).collect();
        s
    }

    /// Build the ordered rows the renderer sees, then place a pending session.
    fn slot_for(sessions: &[&SessionInfo], repo_names: &[&str]) -> PendingSpawnSlot {
        let order = compute_session_order(sessions);
        let ordered: Vec<&SessionInfo> = order.order.iter().map(|&i| sessions[i]).collect();
        let names: Vec<String> = repo_names.iter().map(|s| s.to_string()).collect();
        pending_spawn_slot(&ordered, &order.headers, &names)
    }

    #[test]
    fn pending_row_lands_at_the_end_of_its_own_repo_group() {
        // Ordered: infra(c) | webapp(a, b). A session being created for infra
        // belongs under the infra header, not below webapp.
        let a = info_repo("a", "webapp", SessionStatus::Working);
        let b = info_repo("b", "webapp", SessionStatus::Working);
        let c = info_repo("c", "infra", SessionStatus::Blocked);
        let sessions = vec![&a, &b, &c];

        assert_eq!(
            slot_for(&sessions, &["infra"]),
            PendingSpawnSlot {
                index: 1,
                header: None
            },
            "slots after infra's last row, reusing the existing header"
        );
        assert_eq!(
            slot_for(&sessions, &["webapp"]),
            PendingSpawnSlot {
                index: 3,
                header: None
            },
            "the last group's pending row still goes last"
        );
    }

    #[test]
    fn pending_row_for_a_new_repo_brings_its_own_header() {
        let a = info_repo("a", "webapp", SessionStatus::Working);
        let sessions = vec![&a];

        assert_eq!(
            slot_for(&sessions, &["infra"]),
            PendingSpawnSlot {
                index: 1,
                header: Some("infra".to_string())
            },
            "a group with no rows yet is appended, labelled"
        );
    }

    /// A pending multi-repo session forms its own group, exactly as the real
    /// session will — it must not be filed under either constituent repo.
    #[test]
    fn pending_multi_repo_row_groups_by_its_repo_set() {
        let a = info_repo("a", "webapp", SessionStatus::Working);
        let both = info_repos("both", &["webapp", "infra"], SessionStatus::Working);
        let sessions = vec![&a, &both];

        // Order is webapp(a) | webapp + infra(both).
        assert_eq!(
            slot_for(&sessions, &["webapp", "infra"]),
            PendingSpawnSlot {
                index: 2,
                header: None
            },
            "joins the existing multi-repo group"
        );
        // Repo-set equality is order-insensitive, matching `group_key`.
        assert_eq!(
            slot_for(&sessions, &["infra", "webapp"]).header,
            None,
            "the same set in another order is the same group"
        );
        assert_eq!(
            slot_for(&sessions, &["infra"]),
            PendingSpawnSlot {
                index: 2,
                header: Some("infra".to_string())
            },
            "a lone infra session is its own group, not the webapp+infra one"
        );
    }

    #[test]
    fn pending_row_with_no_repo_joins_the_no_repo_group() {
        let a = info_repo("a", "webapp", SessionStatus::Working);
        let n = info("no-repo");
        let sessions = vec![&a, &n];

        let slot = slot_for(&sessions, &[]);
        assert_eq!(slot.header, None, "reuses the existing (no repo) header");
        let empty: Vec<&SessionInfo> = Vec::new();
        assert_eq!(
            pending_spawn_slot(&empty, &[], &[]),
            PendingSpawnSlot {
                index: 0,
                header: Some(NO_REPO_GROUP.to_string())
            },
            "the very first session labels its own group"
        );
    }
}
