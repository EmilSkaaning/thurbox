--!strict
-- thurbox's session list, in Luau.
--
-- A faithful reproduction of `ui::project_list::session_list_tree`: the same
-- group header and trailing rule, the same status glyph — animated, for a working
-- session — the same nesting prefix, the same remote and worktree marks, the same
-- search emphasis, the same reported text, and the same selection bar carried to
-- the pane's edge. `tests/bundled_session_list.rs` asserts the two view trees are
-- *equal* and that they paint the same frame, which is what makes this the
-- measurement of whether a third party could have written the pane the whole
-- design hinges on.
--
-- Four things are worth reading for what they imply about writing a pane:
--
-- 1. **The animation is declared, not driven.** A working row's glyph is a
--    `ui.cycle` of ten frames pushed *once*; the kernel decides which frame is
--    showing, from its own clock, and repaints that pane at the declared rate.
--    There is deliberately no call by which this plugin asks for a frame — a
--    push-per-frame spinner would cost a VM call, a tree rebuild and a diff for
--    something the kernel evaluates for free. Frame 1 has to be a correct
--    rendering on its own, because it is what a reduced-motion user sees.
-- 2. **The cursor is the kernel's.** Each row says whether the cursor is on it;
--    this pane receives no keys and owns no selection. That is not a limitation
--    working around a missing binding — it is the reason a keypress moves the
--    highlight in the same frame it was handled instead of waiting for this VM.
-- 3. **The host publishes no row.** A row arrives as facts — a status, some
--    flags, byte offsets, two unresolved strings — and every glyph, mark,
--    separator and colour role below is this plugin's choice. The one exception
--    is `status.token`, which thurbox resolves because two of its panes share
--    that mapping and a second copy could disagree with it.
-- 4. **Nothing here knows the pane's size.** No name is truncated, no window is
--    chosen: the selection bar and the header's rule reach the right edge through
--    `ui.fill`, whose width the host resolves, and the list scrolls because the
--    last line hands the cursor's row back.
--
-- What this pane deliberately does *not* draw is the native pane's chrome — its
-- bordered block, the one-dot-per-session strip on that border, and the clipped-row
-- indicators — and the placeholder row for a session still being created. The
-- first three are a pane's frame, which is the host's; the last is a row for a
-- record that does not exist yet, inserted at an index the kernel computes.
--
-- A repo-group header is **not** a row of the list: it is emitted inside the same
-- child as the session it heads (a `ui.column` of the two), so it scrolls with
-- that row, a click anywhere on the pair selects that session, and the host's
-- window counts one row where a reader sees one.

local thurbox = require("@thurbox")
local ui = thurbox.ui

type ViewNode = thurbox.ViewNode
type StyleToken = thurbox.StyleToken
type SessionRow = thurbox.SessionRow

-- A resolved text style, in the parts a `text` node carries. `selected` is not an
-- emphasis: it asks the host for the theme's selection pair, which *replaces* the
-- token's colour rather than layering over it.
type RowStyle = {
    token: StyleToken?,
    bold: boolean,
    underline: boolean,
    selected: boolean,
}

local function text(content: string, style: RowStyle): ViewNode
    return ui.text(content, style.token, style.bold, style.underline, false, style.selected)
end

-- ── the pane's own glyphs ───────────────────────────────────────────────────

-- Braille frames for a working session, in order. Ten at 8 fps reads as a smooth
-- spinner; the first is what a reduced-motion pane holds.
local SPINNER_FRAMES = { "⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏" }
local SPINNER_FPS = 8

-- Keys the spinner's phase. Identity is per pane, so this string coordinates with
-- nothing outside this file.
local SPINNER_ID = "spinner"

-- A session whose agent runs on another machine, and one on its own worktree.
-- Each carries the single trailing space that keeps it off the name.
local REMOTE_MARK = "⇅ "
local WORKTREE_MARK = "⑂ "

-- Between the name and the agent's reported text.
local SEPARATOR = "  "

-- ── styles ──────────────────────────────────────────────────────────────────

-- How every run of the cursor's row is drawn: one appearance for the whole row,
-- bold, in the theme's selection pair. The row's own colour roles are replaced
-- rather than layered under it, which is what makes the bar read as one bar.
local SELECTED: RowStyle = { token = nil, bold = true, underline = false, selected = true }

-- The theme's primary foreground: no token, no emphasis.
local PLAIN: RowStyle = { token = nil, bold = false, underline = false, selected = false }

local function tokenStyle(token: StyleToken): RowStyle
    return { token = token, bold = false, underline = false, selected = false }
end

-- A run in `token`, unless the row's selection overrides it.
local function runStyle(row: SessionRow, token: StyleToken): RowStyle
    if row.selected then
        return SELECTED
    end
    return tokenStyle(token)
end

-- A mark: muted when a search dimmed the row, otherwise its own role.
local function markStyle(row: SessionRow, token: StyleToken): RowStyle
    return runStyle(row, if row.dimmed then "muted" else token)
end

-- The status glyph takes the token thurbox resolved for that state.
local function glyphStyle(row: SessionRow): RowStyle
    return markStyle(row, row.status.token)
end

-- The session name: primary text, muted on a dimmed row.
local function nameStyle(row: SessionRow): RowStyle
    if row.selected then
        return SELECTED
    elseif row.dimmed then
        return tokenStyle("muted")
    end
    return PLAIN
end

-- A character the search matched: accent, bold and underlined — and on the
-- cursor's row, underlined over the selection pair, since the bar owns the colour
-- there.
local function matchedStyle(row: SessionRow): RowStyle
    if row.selected then
        return { token = nil, bold = true, underline = true, selected = true }
    end
    return { token = "accent", bold = true, underline = true, selected = false }
end

-- The agent's text after the name: an attention notification keeps the status
-- colour, so it stands out; ordinary activity text is muted, so the name stays the
-- anchor.
local function reportedStyle(row: SessionRow): RowStyle
    if row.status.name == "blocked" then
        return glyphStyle(row)
    end
    return runStyle(row, "muted")
end

-- ── row parts ───────────────────────────────────────────────────────────────

-- Which of the two reported strings the row shows, or none.
--
-- A blocked session shows what its agent is asking for, falling back to the state's
-- own label so the row never goes silent while it waits. Otherwise the agent's
-- activity title, when it sent one that is not just whitespace. The coloured glyph
-- already carries the state, so a row with nothing reported carries no text.
local function reportedText(row: SessionRow): string?
    if row.status.name == "blocked" then
        return row.notification or row.status.label
    end
    local activity = row.activity
    if activity then
        local trimmed = string.match(activity, "^%s*(.-)%s*$")
        if trimmed ~= nil and trimmed ~= "" then
            return trimmed
        end
    end
    return nil
end

-- The prefix between the glyph and the name: `└ ` per level of nesting, or `↳ `
-- for a child whose parent renders in another group.
local function prefixText(row: SessionRow): string
    if row.depth > 0 then
        return string.rep("  ", row.depth - 1) .. "└ "
    elseif row.crossGroupChild then
        return "↳ "
    end
    return ""
end

-- How many bytes the UTF-8 character starting at a given byte occupies.
local function charLen(byte: number): number
    if byte >= 240 then
        return 4
    elseif byte >= 224 then
        return 3
    elseif byte >= 192 then
        return 2
    end
    return 1
end

-- Split `name` into `{ text, matched }` runs at the host's byte offsets.
--
-- Mirrors the host's segmentation exactly, including both of its edge rules — an
-- offset past the end of the string stops the scan, and one that does not land on
-- a character boundary is skipped — because the offsets are computed against a
-- string the host may have shortened, and splitting a multi-byte character in half
-- would emphasise the wrong bytes for the rest of the row.
local function matchedRuns(name: string, positions: { number }): { { text: string, matched: boolean } }
    local runs: { { text: string, matched: boolean } } = {}
    local lastEnd = 0
    for _, pos in ipairs(positions) do
        if pos > #name then
            break
        end
        local byte = string.byte(name, pos + 1)
        if byte == nil then
            continue
        end
        -- A continuation byte: the offset is inside a character, not at its start.
        if byte >= 128 and byte < 192 then
            continue
        end
        local len = charLen(byte)
        if pos > lastEnd then
            table.insert(runs, { text = string.sub(name, lastEnd + 1, pos), matched = false })
        end
        table.insert(runs, { text = string.sub(name, pos + 1, pos + len), matched = true })
        lastEnd = pos + len
    end
    if lastEnd < #name then
        table.insert(runs, { text = string.sub(name, lastEnd + 1), matched = false })
    end
    return runs
end

-- ── nodes ───────────────────────────────────────────────────────────────────

-- The status glyph, animated while the session is working.
--
-- Pushed as a cycle of complete runs rather than of bare glyphs, so each frame is
-- styled and padded exactly as the static form is and the name never shifts by a
-- column as the animation runs.
local function statusGlyph(row: SessionRow): ViewNode
    local style = glyphStyle(row)
    if row.status.name == "working" then
        local frames: { ViewNode } = {}
        for _, frame in ipairs(SPINNER_FRAMES) do
            table.insert(frames, text(" " .. frame .. " ", style))
        end
        return ui.cycle(SPINNER_ID, frames, SPINNER_FPS)
    end
    return text(" " .. row.status.icon .. " ", style)
end

-- A repo-group header: `── label ──────────`, always muted, never reflecting
-- selection — highlighting belongs to the session rows, so selecting a group's
-- first row must not light up its header. The trailing rule is a fill because it
-- reaches the pane's edge and this plugin is never told which column that is.
local function headerNode(label: string): ViewNode
    return ui.line({
        ui.text("── " .. label .. " ", "muted"),
        ui.fill("─", "muted"),
    })
end

local function rowNode(row: SessionRow): ViewNode
    local runs: { ViewNode } = { statusGlyph(row) }

    local prefix = prefixText(row)
    if prefix ~= "" then
        -- Muted whether or not the row matched a search: it is structure, not
        -- content, so dimming it would say nothing.
        table.insert(runs, text(prefix, runStyle(row, "muted")))
    end
    if row.remote then
        table.insert(runs, text(REMOTE_MARK, markStyle(row, "accent")))
    end
    if row.worktree then
        table.insert(runs, text(WORKTREE_MARK, markStyle(row, "branch")))
    end

    for _, run in ipairs(matchedRuns(row.name, row.matchPositions)) do
        table.insert(runs, text(run.text, if run.matched then matchedStyle(row) else nameStyle(row)))
    end

    local reported = reportedText(row)
    if reported then
        -- The separator takes no role of its own — two spaces carry no meaning —
        -- so it inherits the row's selection appearance and nothing else.
        table.insert(runs, text(SEPARATOR, if row.selected then SELECTED else PLAIN))
        table.insert(runs, text(reported, reportedStyle(row)))
    end

    if row.selected then
        -- The selection bar is the row, not the text on it: it reaches the pane's
        -- right edge, and the residue between the two is a fill.
        table.insert(runs, ui.fill(" ", { bold = true, selected = true }))
    end

    return ui.line(runs)
end

-- ── the pane ────────────────────────────────────────────────────────────────

return {
    render = function(_paneId: string): ViewNode
        local read = thurbox.sessionList
        local list = if read then read() else nil
        local rows = if list then list.rows else {}

        if #rows == 0 then
            -- The native pane centres these two lines, and no node carries an
            -- alignment, so this is the one place the reproduction is knowingly
            -- different. The words are its words.
            return ui.list({
                ui.text("No sessions yet", "muted"),
                ui.text("Press Ctrl+N to create one", "muted"),
            })
        end

        local children: { ViewNode } = {}
        -- The row the cursor is on, as an index into `children`. A group's first
        -- row carries its header *inside* the same child, so the two arrays run
        -- one for one — the header scrolls with the row it heads, a click on it
        -- selects that row, and the host counts one row where a reader sees one.
        local selected: number? = nil
        for _, row in ipairs(rows) do
            local group = row.group
            local node = rowNode(row)
            if group then
                node = ui.column({ headerNode(group), node })
            end
            table.insert(children, node)
            if row.selected then
                selected = #children
            end
        end
        -- Handing the cursor back is how the pane scrolls: the *kernel* windows
        -- the list to keep that row visible, from a height this plugin is never
        -- told.
        return ui.list(children, selected)
    end,
}
