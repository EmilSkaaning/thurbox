--!strict
-- thurbox's tasks pane, in Luau.
--
-- A faithful reproduction of `ui::tasks_panel::tasks_tree`: the same glyph per
-- status, the same colour token, the same selected > dimmed > status precedence,
-- the same emphasis on matched characters, the same trailing marker, the same
-- empty-state line. `tests/bundled_tasks_panel.rs` asserts the two view trees are
-- *equal*, which makes this the measurement of whether a third party could have
-- written the pane.
--
-- Three things are worth reading for what they imply about writing a list pane:
--
-- 1. **The host publishes no rendering.** A row arrives with a status *name*;
--    the glyph and the colour token below are this plugin's choices. That is the
--    opposite of a session status, whose glyph and token the host resolves
--    because two native panes share that mapping.
-- 2. **A matched run is segmented here.** The host publishes byte offsets, so the
--    UTF-8 walk that turns them into runs is the plugin's — and it has to be
--    exactly the host's walk, including its two edge rules, or a title with a
--    multi-byte character in it would highlight the wrong bytes.
-- 3. **Nothing here knows the pane's size.** No title is truncated and no window
--    is chosen: the host resolves both, and this pane draws what it is given and
--    lets the renderer clip. The two consequences of that are enumerated in
--    `tests/bundled_tasks_panel.rs` rather than hidden.

local thurbox = require("@thurbox")
local ui = thurbox.ui

type ViewNode = thurbox.ViewNode
type StyleToken = thurbox.StyleToken
type TaskEntry = thurbox.TaskEntry

-- A resolved text style, in the four parts a `text` node carries.
type RowStyle = {
    token: StyleToken?,
    bold: boolean,
    underline: boolean,
    dim: boolean,
}

local function text(content: string, style: RowStyle): ViewNode
    return ui.text(content, style.token, style.bold, style.underline, style.dim)
end

-- ── status ──────────────────────────────────────────────────────────────────

-- The checkbox glyphs. An unknown status cannot arrive — the host's vocabulary is
-- closed — but a fallback keeps a row drawable rather than erroring the pane if a
-- later thurbox adds a state this plugin has not learned.
local function statusGlyph(status: string): string
    if status == "in_progress" then
        return "◐"
    elseif status == "done" then
        return "☑"
    end
    return "☐"
end

-- A todo has *no* token: it draws in the theme's primary text, which is what an
-- untouched task has always looked like. In-progress takes the accent, and a
-- finished task recedes into muted.
local function statusToken(status: string): StyleToken?
    if status == "in_progress" then
        return "accent"
    elseif status == "done" then
        return "muted"
    end
    return nil
end

-- ── row styles ──────────────────────────────────────────────────────────────

-- Selected wins over dimmed, which wins over the status colour — the precedence
-- thurbox's three list panes share, so they feel like one list.
local function baseStyle(entry: TaskEntry): RowStyle
    if entry.selected then
        -- The shared selected-item style: accent and bold.
        return { token = "accent", bold = true, underline = false, dim = false }
    elseif entry.dimmed then
        -- A row the search filtered out: muted *and* dim, so it recedes below
        -- ordinary muted text.
        return { token = "muted", bold = false, underline = false, dim = true }
    end
    return { token = statusToken(entry.status), bold = false, underline = false, dim = false }
end

-- A matched character: accent, bold and underlined, whatever the row's base was.
local MATCHED: RowStyle = { token = "accent", bold = true, underline = true, dim = false }

-- The trailing marker on a task with a live session. Accent on its own — not the
-- row's base — so it reads as a marker rather than as more title; a dimmed row
-- keeps the dim tone so a filtered-out row stays uniformly quiet.
local LINKED_MARKER = " ⇄"

-- ── matched runs ────────────────────────────────────────────────────────────

-- How many bytes the UTF-8 character starting at `i` (1-based) occupies.
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

-- Split `title` into `{ text, matched }` runs at the host's byte offsets.
--
-- Mirrors the host's segmentation exactly, including both of its edge rules,
-- because the offsets are computed against a string the host may have shortened:
--
-- * an offset past the end of the string stops the scan, and
-- * an offset that does not land on a character boundary is skipped — it would
--   otherwise split a multi-byte character in half.
--
-- Offsets arrive zero-based (the host's own convention); `string.sub` is
-- one-based and inclusive, hence the `+ 1`s.
local function matchedRuns(title: string, positions: { number }): { { text: string, matched: boolean } }
    local runs: { { text: string, matched: boolean } } = {}
    local lastEnd = 0
    for _, pos in ipairs(positions) do
        if pos > #title then
            break
        end
        local byte = string.byte(title, pos + 1)
        -- Past the last character, or inside one: nothing to emphasise here.
        if byte == nil then
            continue
        end
        if byte >= 128 and byte < 192 then
            continue
        end
        local len = charLen(byte)
        if pos > lastEnd then
            table.insert(runs, { text = string.sub(title, lastEnd + 1, pos), matched = false })
        end
        table.insert(runs, { text = string.sub(title, pos + 1, pos + len), matched = true })
        lastEnd = pos + len
    end
    if lastEnd < #title then
        table.insert(runs, { text = string.sub(title, lastEnd + 1), matched = false })
    end
    return runs
end

-- ── rows ────────────────────────────────────────────────────────────────────

local function rowNode(entry: TaskEntry): ViewNode
    local base = baseStyle(entry)
    local runs: { ViewNode } = { text(statusGlyph(entry.status) .. " ", base) }

    -- A dimmed row did not match, so it has nothing to emphasise. Stated rather
    -- than assumed: the host publishes no offsets for such a row either, and the
    -- two agreeing is what keeps the rule from mattering.
    local positions = if entry.dimmed then {} else entry.matchPositions
    for _, run in ipairs(matchedRuns(entry.title, positions)) do
        table.insert(runs, text(run.text, if run.matched then MATCHED else base))
    end

    if entry.linked then
        local markerStyle: RowStyle = if entry.dimmed
            then base
            else { token = "accent", bold = false, underline = false, dim = false }
        table.insert(runs, text(LINKED_MARKER, markerStyle))
    end

    return ui.line(runs)
end

-- ── the pane ────────────────────────────────────────────────────────────────

return {
    render = function(_paneId: string): ViewNode
        local readTasks = thurbox.tasks
        local tasks = if readTasks then readTasks() else nil
        local entries = if tasks then tasks.entries else {}

        if #entries == 0 then
            -- The empty state names the key that adds a task only when the pane
            -- can receive it — which is why `focused` is published at all.
            local focused = if tasks then tasks.focused else false
            local hint = if focused then "no tasks — n to add" else "no tasks"
            return ui.list({ ui.text(hint, "muted") })
        end

        local rows: { ViewNode } = {}
        for _, entry in ipairs(entries) do
            table.insert(rows, rowNode(entry))
        end
        return ui.list(rows)
    end,
}
