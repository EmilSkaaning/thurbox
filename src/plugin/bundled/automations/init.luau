--!strict
-- thurbox's automations pane, in Luau.
--
-- A faithful reproduction of `ui::automations_panel::automations_tree`: the same
-- two markers, the same colour role per state, the same
-- selected > dimmed > resting precedence, the same emphasis on matched
-- characters, the same summary composition, the same empty-state line.
-- `tests/bundled_automations_panel.rs` asserts the two view trees are *equal*.
--
-- Three things are worth reading for what they imply about writing a pane:
--
-- 1. **The summary is composed here, from parts.** The host publishes a schedule
--    *label*, an action *name* and a duration in seconds — the two things a
--    sandbox cannot derive (thurbox's cron vocabulary, and a clock) plus one it
--    can. The separator, the ordering and the three-way `when` precedence are
--    this plugin's, and the test compares them against thurbox's own rule rather
--    than against a third copy.
-- 2. **The cursor is this plugin's own.** Every earlier bundled pane was
--    read-only, and the tasks port recorded a pane's keys as unportable because a
--    plugin "cannot name the row the user is looking at". That is true of
--    *thurbox's* cursor: the host marks it only while thurbox's own pane holds
--    focus, which is never while this pane does. But a plugin holding `input`
--    receives keys only while its own pane is focused, and this VM persists
--    across renders — so `cursor` below is ordinary state, and the row the user
--    is looking at is the row it names. It starts as `nil`, meaning "follow the
--    host's", which is what makes an undriven pane an exact copy.
-- 3. **A write addresses a row by id, never by index.** `entries[i].id` is what
--    the three write bindings take. The pane a user is driving and the list the
--    host published are the same rows in the same order, but an index would
--    silently act on the wrong record the moment they were not.
--
-- What it cannot do, and does not pretend to: create an automation (`n` — the
-- write seam has no creation binding) or open the central-pane editor
-- (`Enter`/`e` — a seat the pane-slot vocabulary does not offer). And it cannot
-- complete the left column's circular wrap: at either edge it *declines* the key,
-- which is the honest half, and moving focus into the session list is the
-- kernel's, because focus is view state no capability writes.
--
-- One interaction differs from thurbox's pane, and it is the visible cost of not
-- being told about its own focus: a write key does nothing until a movement key has
-- placed a cursor this pane draws. thurbox's pane can act immediately because the
-- kernel knows it is focused and is already drawing a cursor; this one would be
-- acting on a row the user cannot see, so it refuses instead of guessing.

local thurbox = require("@thurbox")
local ui = thurbox.ui

type ViewNode = thurbox.ViewNode
type StyleToken = thurbox.StyleToken
type AutomationEntry = thurbox.AutomationEntry
type AutomationList = thurbox.AutomationList

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

-- ── the summary ─────────────────────────────────────────────────────────────

-- The countdown, character for character `ui::format_countdown` — and character
-- for character `formatDueIn` in the bundled info-panel plugin, which is the
-- second time two bundled panes have needed the same formatter. Recorded in the
-- port's design as the first non-speculative case for a `thurbox.format.*` table;
-- copied rather than shared here because adding that table would edit a shipped
-- plugin in the change that measures the need.
local function formatDueIn(secs: number): string
    if secs == 0 then
        return "due"
    elseif secs < 60 then
        return string.format("in %ds", secs)
    elseif secs < 3600 then
        local m = secs // 60
        local s = secs % 60
        if s == 0 then
            return string.format("in %dm", m)
        end
        return string.format("in %dm %ds", m, s)
    end
    local h = secs // 3600
    local m = (secs % 3600) // 60
    if m == 0 then
        return string.format("in %dh", h)
    end
    return string.format("in %dh %dm", h, m)
end

-- `<schedule> · <action> · <when>`, where `when` has a three-way precedence: a
-- disabled automation says so rather than counting down to a run that will not
-- happen, an enabled one with a next run counts down, and an enabled one with
-- none shows a placeholder. `dueInSecs` is absent, not zero, for the last case —
-- "never" and "due now" are different rows.
local function rowSummary(entry: AutomationEntry): string
    local when: string
    if not entry.enabled then
        when = "disabled"
    elseif entry.dueInSecs then
        when = formatDueIn(entry.dueInSecs)
    else
        when = "—"
    end
    return `{entry.schedule} · {entry.action} · {when}`
end

-- ── row styles ──────────────────────────────────────────────────────────────

-- The enabled marker: filled for an enabled automation, hollow for a disabled
-- one. The host publishes the *flag*; which glyph it draws as is the pane's.
local function marker(enabled: boolean): string
    return if enabled then "●" else "○"
end

-- Selected wins over dimmed, which wins over the enabled/disabled colour role —
-- the precedence thurbox's list panes share, so they feel like one list.
local function baseStyle(entry: AutomationEntry, selected: boolean): RowStyle
    if selected then
        -- The shared selected-item style: accent and bold.
        return { token = "accent", bold = true, underline = false, dim = false }
    elseif entry.dimmed then
        -- A row the search filtered out: muted *and* dim, so it recedes below
        -- ordinary muted text.
        return { token = "muted", bold = false, underline = false, dim = true }
    end
    return {
        token = if entry.enabled then "secondary" else "muted",
        bold = false,
        underline = false,
        dim = false,
    }
end

-- A matched character: accent, bold and underlined, whatever the row's base was.
local MATCHED: RowStyle = { token = "accent", bold = true, underline = true, dim = false }

-- ── matched runs ────────────────────────────────────────────────────────────

-- How many bytes the UTF-8 character starting at this lead byte occupies.
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
-- Mirrors the host's segmentation exactly, including both of its edge rules,
-- because the offsets are computed against a string the host may have shortened:
--
-- * an offset past the end of the string stops the scan, and
-- * an offset that does not land on a character boundary is skipped — it would
--   otherwise split a multi-byte character in half.
--
-- Offsets arrive zero-based (the host's own convention); `string.sub` is
-- one-based and inclusive, hence the `+ 1`s.
local function matchedRuns(name: string, positions: { number }): { { text: string, matched: boolean } }
    local runs: { { text: string, matched: boolean } } = {}
    local lastEnd = 0
    for _, pos in ipairs(positions) do
        if pos > #name then
            break
        end
        local byte = string.byte(name, pos + 1)
        -- Past the last character, or inside one: nothing to emphasise here.
        if byte == nil then
            continue
        end
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

-- ── rows ────────────────────────────────────────────────────────────────────

local function rowNode(entry: AutomationEntry, selected: boolean): ViewNode
    local base = baseStyle(entry, selected)
    -- The marker is its own run; the name follows so the host's byte offsets
    -- (which index `name`) line up.
    local runs: { ViewNode } = { text(` {marker(entry.enabled)} `, base) }

    -- A dimmed row did not match, so it has nothing to emphasise. Stated rather
    -- than assumed: the host publishes no offsets for such a row either, and the
    -- two agreeing is what keeps the rule from mattering.
    local positions = if entry.dimmed then {} else entry.matchPositions
    for _, run in ipairs(matchedRuns(entry.name, positions)) do
        table.insert(runs, text(run.text, if run.matched then MATCHED else base))
    end

    table.insert(runs, text(` — {rowSummary(entry)} `, base))
    return ui.line(runs)
end

-- ── this pane's own cursor ──────────────────────────────────────────────────

-- `nil` means "whatever the host says", which is how an undriven pane stays an
-- exact copy of thurbox's. The first movement key adopts the host's row and moves
-- from there; from then on this is the pane's cursor and thurbox's is untouched.
--
-- One value rather than one per pane id because this plugin declares one pane.
local cursor: number? = nil

local function readAutomations(): AutomationList?
    local read = thurbox.automations
    return if read then read() else nil
end

-- The row the pane draws its cursor on: its own once it has one, else the host's.
local function anchor(list: AutomationList?): number?
    if cursor then
        return cursor
    end
    return if list then list.cursor else nil
end

-- Whether that row is drawn *as* the cursor.
--
-- Once this plugin has a cursor of its own it draws it unconditionally, because
-- the host tells a pane nothing about its **own** focus — `list.focused` is
-- thurbox's pane, not this one. The consequence is enumerated in the port rather
-- than hidden: after `Esc` the highlight stays. Before the first key the host's
-- answer is used, which is what keeps the reproduction exact.
local function cursorDrawn(list: AutomationList?): boolean
    if cursor then
        return true
    end
    return if list then list.cursorVisible else false
end

-- Move the cursor by `delta`, or **decline** the key when there is nowhere to go.
--
-- Declining at an edge is this plugin's half of the left column's circular wrap:
-- in thurbox's own pane the same keypress moves focus into the session list, and
-- moving focus is view state no capability writes. Returning `false` says "I could
-- not move"; completing it is the kernel's, and it does not yet.
local function move(list: AutomationList?, delta: number): boolean
    local entries = if list then list.entries else {}
    if #entries == 0 then
        return false
    end
    local from = anchor(list) or 1
    local to = from + delta
    if to < 1 or to > #entries then
        return false
    end
    cursor = to
    return true
end

-- The automation the cursor is on, or `nil` when there is no row a write may
-- safely act on.
--
-- **Gated on the cursor being drawn**, which is the one place the missing
-- pane-focus fact costs behaviour rather than only appearance. While this pane
-- holds focus thurbox's does not, so `cursorVisible` is false and the pane draws
-- no cursor until a movement key gives it one of its own — and a `Space` in that
-- state would toggle whichever row thurbox's cursor happened to be left on, which
-- the user cannot see. Refusing is the honest answer: press `j`/`k` first, and the
-- row you act on is the row that is highlighted. Never guess at a row nobody is
-- looking at.
local function selectedEntry(list: AutomationList?): AutomationEntry?
    if not cursorDrawn(list) then
        return nil
    end
    local entries = if list then list.entries else {}
    local at = anchor(list)
    if not at or at < 1 or at > #entries then
        return nil
    end
    return entries[at]
end

-- ── the pane ────────────────────────────────────────────────────────────────

return {
    render = function(_paneId: string): ViewNode
        local list = readAutomations()
        local entries = if list then list.entries else {}

        if #entries == 0 then
            -- Whatever the pane last had a cursor on is gone with the rows.
            cursor = nil
            -- The empty state names the key that adds an automation only when the
            -- pane can receive it — which is thurbox's pane, since that key is one
            -- this plugin cannot reproduce. So it is the host's `focused` here, not
            -- a fact about this pane.
            local focused = if list then list.focused else false
            local hint = if focused then "none — Ctrl+N to add" else "none"
            return ui.list({ ui.text(hint, "muted") })
        end

        -- A list that shortened under the cursor: clamp rather than draw nothing,
        -- which is what thurbox's pane does with a stale selection.
        if cursor and cursor > #entries then
            cursor = #entries
        end

        local at = anchor(list)
        local drawn = cursorDrawn(list)
        local rows: { ViewNode } = {}
        for i, entry in ipairs(entries) do
            table.insert(rows, rowNode(entry, drawn and i == at))
        end
        -- Declaring the anchor is what makes the pane scroll: the host keeps that
        -- row in view from a height this plugin is never told. It is declared even
        -- when the cursor is not *drawn*, because a list still has to scroll to
        -- where it is — see `AutomationList` in `thurbox.d.luau`.
        return ui.list(rows, at)
    end,

    -- Switch on the binding id, not the key: the user may rebind the chord in the
    -- F1 editor and the id does not change when they do. The arrow keys are
    -- handled by name because a `[[keybindings]]` entry declares one chord where
    -- thurbox's own action declares a list, so `j` is declared and `down` is not.
    onKey = function(_paneId: string, key: string, binding: string?): boolean
        local list = readAutomations()

        if binding == "next" or key == "down" then
            return move(list, 1)
        elseif binding == "prev" or key == "up" then
            return move(list, -1)
        end

        -- The three write keys act on the row this pane drew its cursor on,
        -- addressed by the id the host published with it.
        local entry = selectedEntry(list)
        if not entry then
            return false
        end

        if binding == "toggle" then
            local setEnabled = thurbox.setAutomationEnabled
            if not setEnabled then
                return false
            end
            setEnabled(entry.id, not entry.enabled)
            return true
        elseif binding == "run" then
            local run = thurbox.runAutomation
            if not run then
                return false
            end
            -- A *request*: this marks the automation due and the kernel fires it
            -- on its own pass, under the claim that de-duplicates a running TUI
            -- and a headless tick. Nothing runs on this thread — which is the
            -- property that makes the key safe to hand a plugin at all, since an
            -- automation the user authored may run a shell command.
            run(entry.id)
            return true
        elseif binding == "delete" then
            local remove = thurbox.deleteAutomation
            if not remove then
                return false
            end
            remove(entry.id)
            -- The row under the cursor is gone; step back so the cursor does not
            -- sit past the end until the next publication arrives.
            if cursor and cursor > 1 then
                cursor = cursor - 1
            end
            return true
        end

        return false
    end,

    -- A click selects, exactly as it does in thurbox's pane. `row` is the index in
    -- this pane's outermost list, which is the same numbering `ui.list`'s
    -- `selected` uses — so the row highlighted and the row clicked are one number.
    onClick = function(_paneId: string, row: number): boolean
        local list = readAutomations()
        local entries = if list then list.entries else {}
        if row < 1 or row > #entries then
            return false
        end
        cursor = row
        return true
    end,
}
