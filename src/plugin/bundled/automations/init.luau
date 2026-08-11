--!strict
-- thurbox's automations pane, in Luau.
--
-- Not a reproduction any more: `src/ui/automations_panel.rs` is deleted and this **is**
-- the pane (ADR-56). `tests/bundled_automations_panel.rs` still holds it to the native
-- pane's tree — the thirteen checked-in recordings were generated from
-- `ui::automations_panel::automations_tree` while it existed — so this file is measured
-- against the pane rather than against itself.
--
-- Four things are worth reading, because together they are the v2 pane model:
--
-- 1. **This file draws; it does not drive.** The manifest declares
--    `key_context = "Automations"`, so all seven of the pane's scoped actions resolve
--    while the pane holds focus and the *kernel* performs them against its own state —
--    `j`/`k` move its cursor (and wrap into the session list), `Space` toggles enabled,
--    `r` requests a run, `d` deletes, `n` creates, `Enter`/`e` opens the central editor.
--    Nothing here receives a key, and nothing here needed a capability to be driven.
--
--    This is a *reduction*: the port that preceded the handover held `input` and
--    `automations-write`, kept a cursor of its own across renders, and answered five of
--    those seven keys. It could not answer the other two — creating a record has no
--    write operation by construction, and the central editor is a focus a plugin cannot
--    take — and it lost the editor and the run history entirely. Giving the keys back to
--    the kernel answers all seven and costs two capabilities. What that port proved is
--    kept in ADR-56, because it is still the answer for a pane that wants keys of its
--    own.
--
-- 2. **The summary is composed here, from parts.** The host publishes a schedule
--    *label*, an action *name* and a duration in seconds — the two things a sandbox
--    cannot derive (thurbox's cron vocabulary, and a clock) plus one it can. The
--    separator, the ordering and the three-way `when` precedence are this plugin's, and
--    the test compares them against thurbox's own rule
--    (`ui::automations_list_modal::row_summary`, which the `Ctrl+P` modal still
--    composes) rather than against a third copy.
--
-- 3. **The cursor is the host's.** `list.cursor` is the row thurbox's keyboard is on and
--    `list.cursorVisible` is whether it is drawn — two answers rather than one, because
--    this pane windows to its cursor even while the cursor is not drawn (an unfocused
--    pane still shows the part of the list the keyboard is in). Nothing here tracks a
--    cursor: the keys that move it are the kernel's.
--
-- 4. **Nothing here knows the pane's size**, and the rows still scroll *and* fit. It
--    names the row the cursor is on and the host windows the list to it; it declares
--    that the name's runs yield their width (`ellipsize`) and the host cuts them with
--    one ellipsis, keeping the marker's and the whole summary's columns (ADR-52/ADR-55).
--    Both are the same trade the gauge node made: declare the intent, the kernel
--    resolves the geometry.

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

-- The same run, declaring that it **yields its width**: when the row is wider than
-- the column the host cuts this run with an ellipsis and leaves every other run on
-- the line its columns, so the marker *and* the whole `— <summary>` tail survive a
-- long name. Without it a long name would be clipped at the pane's edge and take
-- the schedule, the action and the countdown with it — which is the row's content.
--
-- Still no width here: the declaration is the intent and the host resolves the
-- geometry. It goes through the style *table* form because the positional arguments
-- are full.
--
-- A name split at the host's matched offsets is several such runs, and the host cuts
-- consecutive yielding runs as **one** string — so the ellipsis falls where the whole
-- name would have been cut rather than once per run.
local function fittedText(content: string, style: RowStyle): ViewNode
    return ui.text(content, {
        token = style.token,
        bold = style.bold,
        underline = style.underline,
        dim = style.dim,
        ellipsize = true,
    })
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
        -- The name yields its width; the marker before it and the summary after it
        -- do not, which is what decides where an overflowing row is cut.
        table.insert(runs, fittedText(run.text, if run.matched then MATCHED else base))
    end

    table.insert(runs, text(` — {rowSummary(entry)} `, base))
    return ui.line(runs)
end

-- ── the pane ────────────────────────────────────────────────────────────────

local function readAutomations(): AutomationList?
    local read = thurbox.automations
    return if read then read() else nil
end

return {
    render = function(_paneId: string): ViewNode
        local list = readAutomations()
        local entries = if list then list.entries else {}

        if #entries == 0 then
            -- The empty state names the key that adds an automation only when the pane
            -- can receive it, which is what `focused` is for. The chord is spelled out
            -- here, and that is a known wart rather than an oversight: it is
            -- *rebindable* and no published section carries a keymap, so a user who
            -- moved `OpenAutomations` sees the wrong letter. It is inside the pane's
            -- rows, so moving it into kernel seat chrome would change the pane's
            -- content — and the native pane printed exactly this.
            local focused = if list then list.focused else false
            local hint = if focused then "none — Ctrl+N to add" else "none"
            return ui.list({ ui.text(hint, "muted") })
        end

        -- The host's cursor: which row thurbox's keyboard is on, and whether it is
        -- drawn. Two answers, because this pane windows to its cursor even while the
        -- cursor is not drawn — an unfocused pane still shows the part of the list the
        -- keyboard is in.
        local at = if list then list.cursor else nil
        local drawn = if list then list.cursorVisible else false
        local rows: { ViewNode } = {}
        for i, entry in ipairs(entries) do
            table.insert(rows, rowNode(entry, drawn and i == at))
        end
        -- Declaring the anchor is what makes the pane scroll: the host keeps that row
        -- in view from a height this plugin is never told. It is declared even when the
        -- cursor is not *drawn* — see `AutomationList` in `thurbox.d.luau`.
        return ui.list(rows, at)
    end,
}
