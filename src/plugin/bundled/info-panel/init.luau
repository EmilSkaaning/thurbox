--!strict
-- thurbox's info panel, in Luau.
--
-- **This is the pane.** `src/ui/info_panel.rs` is deleted (ADR-50): the Info column
-- a user sees is built here, by a plugin holding no surface a third party's plugin
-- could not declare. `tests/bundled_info_panel.rs` holds it to the tree the native
-- pane built, through recordings taken while that builder existed — so the target
-- is still the pane's behaviour and not this file's.
--
-- Two things are worth reading for what they imply about writing a pane:
--
-- 1. **Every number is formatted here.** The host publishes byte counts, token
--    counts, durations and costs raw, so the eight formatters below are the
--    plugin's own. That is deliberate: a host that shipped "8.0/16.0 GB" would
--    leave a pane arranging strings someone else composed.
-- 2. **Nothing here needs a width or a clock.** Gauges are `ui.gauge` because the
--    kernel owns their geometry, and every countdown arrives as a duration
--    because the sandbox loads no `os`.

local thurbox = require("@thurbox")
local ui = thurbox.ui

type ViewNode = thurbox.ViewNode
type StyleToken = thurbox.StyleToken

-- ── formatters ──────────────────────────────────────────────────────────────

-- Reduce a byte count to a value and a unit, always binary and never below KB,
-- so "0 bytes" reads as `0.0 KB` rather than switching units near zero.
local function humanBytes(bytes: number): (number, number, string)
    local GB = 1073741824
    local MB = 1048576
    local KB = 1024
    if bytes >= GB then
        return bytes / GB, GB, "GB"
    elseif bytes >= MB then
        return bytes / MB, MB, "MB"
    end
    return bytes / KB, KB, "KB"
end

local function formatBytes(bytes: number): string
    local value, _, unit = humanBytes(bytes)
    return string.format("%.1f %s", value, unit)
end

-- Both halves in the *total's* unit, so `8.0/16.0 GB` compares at a glance
-- instead of mixing MB against GB.
local function formatBytesPair(used: number, total: number): string
    local totalValue, divisor, unit = humanBytes(total)
    return string.format("%.1f/%.1f %s", used / divisor, totalValue, unit)
end

-- Four decimal places below a dollar: agent turns routinely cost fractions of a
-- cent, and `$0.00` would hide the difference between them.
local function formatCost(usd: number): string
    if usd >= 1 then
        return string.format("$%.2f", usd)
    end
    return string.format("$%.4f", usd)
end

local function formatDuration(ms: number): string
    if ms < 1000 then
        return string.format("%dms", ms)
    end
    local totalSecs = ms // 1000
    local secs = totalSecs % 60
    local mins = (totalSecs // 60) % 60
    local hours = totalSecs // 3600
    if hours > 0 then
        return string.format("%dh %02dm", hours, mins)
    elseif mins > 0 then
        return string.format("%dm %02ds", mins, secs)
    end
    return string.format("%ds", secs)
end

local function formatTokens(count: number): string
    if count >= 1000000 then
        return string.format("%.1fM", count / 1000000)
    elseif count >= 1000 then
        return string.format("%.1fk", count / 1000)
    end
    return string.format("%d", count)
end

-- A rate-limit window's reset, coarse on purpose: the exact second is noise when
-- the window is days wide.
local function formatCountdownSecs(secs: number): string
    if secs == 0 then
        return "now"
    end
    local days = secs // 86400
    local hours = (secs % 86400) // 3600
    local mins = (secs % 3600) // 60
    if days > 0 then
        return string.format("%dd %dh", days, hours)
    elseif hours > 0 then
        return string.format("%dh %dm", hours, mins)
    elseif mins > 0 then
        return string.format("%dm", mins)
    end
    return "<1m"
end

-- An automation's countdown, which reads as a phrase ("in 2m 30s") because it
-- sits in front of the automation's name.
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

-- ── row shapes ──────────────────────────────────────────────────────────────
--
-- Every textual row is a `paragraph`, not a `line`: the values here are
-- agent-supplied (an OSC title, a notification body, a session name) and
-- clipping one at the pane edge would hide exactly the text a user reads when a
-- session wants attention.

local function field(label: string, value: string, token: StyleToken): ViewNode
    return ui.paragraph({ ui.text(label, "muted"), ui.text(value, token) })
end

local function fieldPlain(label: string, value: string): ViewNode
    return ui.paragraph({ ui.text(label, "muted"), ui.text(value) })
end

local function section(title: string): ViewNode
    return ui.paragraph({ ui.text(title, "accent", true) })
end

-- ── sections ────────────────────────────────────────────────────────────────

local function pushSessionRows(rows: { ViewNode }, session: thurbox.Session)
    table.insert(rows, fieldPlain("Name: ", session.name))
    table.insert(
        rows,
        ui.paragraph({
            ui.text("Status: ", "muted"),
            -- The token the host resolved, not one derived from the status name:
            -- that is what keeps this dot the same colour as the session list's.
            ui.text(
                string.format("%s %s", session.status.icon, session.status.label),
                session.status.token,
                true
            ),
        })
    )
    table.insert(
        rows,
        ui.paragraph({
            ui.text("Agent: ", "muted"),
            ui.text(session.agent, "role", true),
        })
    )
    local parent = session.parentName
    if parent then
        table.insert(rows, field("Parent: ", parent, "secondary"))
    end
    local host = session.remoteHost
    if host then
        table.insert(rows, field("Host:  ", string.format("\u{21c5} %s", host), "accent"))
    end
    -- Without this hint a degraded session's dot just stays Idle and looks like
    -- an agent at rest.
    local hooks = session.hookWiring
    if hooks then
        table.insert(rows, field("Hooks: ", string.format("degraded \u{2014} %s", hooks), "muted"))
    end
    local activity = session.activity
    if activity then
        table.insert(rows, field("Activity: ", activity, "secondary"))
    end
    local signal = session.notification
    if signal then
        local token: StyleToken = if session.status.name == "blocked"
            then session.status.token
            else "muted"
        table.insert(rows, field("Signal:   ", signal, token))
    end
end

local function pushRepoRows(rows: { ViewNode }, session: thurbox.Session)
    local repo = session.repoName
    local branch = session.branch
    local primary: string
    if repo and branch then
        primary = string.format("%s/%s", repo, branch)
    elseif repo then
        primary = repo
    elseif branch then
        primary = branch
    else
        -- Nothing to show if there is no repo information at all.
        return
    end
    table.insert(rows, field("Repos: ", primary, "branch"))
    for _, name in session.additionalDirNames do
        -- Indented under the primary rather than relabelled: the label column
        -- is what aligns them.
        table.insert(rows, field("       ", name, "branch"))
    end
end

local function pushGitRows(rows: { ViewNode }, git: thurbox.GitStats)
    if git.filesChanged > 0 or git.dirty then
        local files = if git.filesChanged == 1
            then "1 file"
            else string.format("%d files", git.filesChanged)
        local runs: { ViewNode } = {
            ui.text("Changes: ", "muted"),
            ui.text(files),
            ui.text(" "),
            ui.text(string.format("+%d", git.insertions), "added"),
            ui.text(" / ", "muted"),
            ui.text(string.format("-%d", git.deletions), "status_error"),
        }
        -- Untracked-only changes count as dirty with nothing in the diff, so say
        -- so rather than showing "0 files +0 / -0" and nothing else.
        if git.dirty and git.filesChanged == 0 then
            table.insert(runs, ui.text(" dirty", "muted"))
        end
        table.insert(rows, ui.paragraph(runs))
    end
    if git.ahead > 0 or git.behind > 0 then
        table.insert(
            rows,
            field(
                "Sync:    ",
                string.format("\u{2191}%d \u{2193}%d", git.ahead, git.behind),
                "muted"
            )
        )
    end
end

local function pushSessionResourceRows(rows: { ViewNode }, m: thurbox.SystemMetrics)
    if m.sessionCpuPercent > 0 or m.sessionMemoryBytes > 0 then
        table.insert(rows, ui.gauge("CPU", m.sessionCpuPercent))
        table.insert(
            rows,
            ui.paragraph({
                ui.text("RAM", "muted"),
                ui.text(string.format("  %s", formatBytes(m.sessionMemoryBytes))),
            })
        )
    end
end

local function agentHeader(m: thurbox.AgentMetrics): string
    local model = m.modelDisplayName
    local version = m.cliVersion
    if model and version then
        return string.format("Agent (%s v%s)", model, version)
    elseif model then
        return string.format("Agent (%s)", model)
    elseif version then
        return string.format("Agent (v%s)", version)
    end
    return "Agent"
end

local function pushAgentRows(rows: { ViewNode }, m: thurbox.AgentMetrics)
    table.insert(rows, ui.divider())
    table.insert(rows, section(agentHeader(m)))

    local cost = m.totalCostUsd
    if cost and cost > 0 then
        table.insert(rows, field("Cost:    ", formatCost(cost), "accent"))
    end

    local duration = m.totalDurationMs
    if duration then
        local runs: { ViewNode } = {
            ui.text("Time:    ", "muted"),
            ui.text(formatDuration(duration)),
        }
        local api = m.totalApiDurationMs
        if api and api > 0 then
            table.insert(
                runs,
                ui.text(string.format("  (api %s)", formatDuration(api)), "muted")
            )
        end
        table.insert(rows, ui.paragraph(runs))
    end

    local inTokens = m.totalInputTokens
    local outTokens = m.totalOutputTokens
    if inTokens or outTokens then
        local function render(v: number?): string
            return if v then formatTokens(v) else "-"
        end
        table.insert(
            rows,
            fieldPlain(
                "Tokens:  ",
                string.format("%s in / %s out", render(inTokens), render(outTokens))
            )
        )
    end

    -- The context window as used/total tokens when its size is known, else a
    -- bare percentage: the percentage alone hides how much room is left.
    local used = m.usedPercentage
    if used then
        local size = m.contextWindowSize
        local suffix: string? = nil
        if size then
            local consumed = math.round(size * used / 100)
            suffix = string.format("%s/%s", formatTokens(consumed), formatTokens(size))
        end
        table.insert(rows, ui.gauge("Context", used, suffix))
    end

    local added = m.totalLinesAdded
    local removed = m.totalLinesRemoved
    if added or removed then
        table.insert(
            rows,
            ui.paragraph({
                ui.text("Lines:   ", "muted"),
                ui.text(string.format("+%d", added or 0), "added"),
                ui.text(" / ", "muted"),
                ui.text(string.format("-%d", removed or 0), "status_error"),
            })
        )
    end

    local cacheRead = m.cacheReadInputTokens or 0
    local cacheCreated = m.cacheCreationInputTokens or 0
    if cacheRead > 0 or cacheCreated > 0 then
        table.insert(
            rows,
            fieldPlain(
                "Cache:   ",
                string.format(
                    "%s read / %s created",
                    formatTokens(cacheRead),
                    formatTokens(cacheCreated)
                )
            )
        )
    end
end

-- The heading names the plan and, for a remote session, the host whose
-- credentials the numbers came from — so usage from two accounts is never
-- mistaken for one.
local function usageHeader(usage: thurbox.Usage, host: string?): string
    local plan = usage.plan
    if plan and host then
        return string.format("Usage (%s \u{b7} %s)", plan, host)
    elseif plan then
        return string.format("Usage (%s)", plan)
    elseif host then
        return string.format("Usage (%s)", host)
    end
    return "Usage"
end

local function pushUsageRows(rows: { ViewNode }, usage: thurbox.Usage, host: string?)
    table.insert(rows, ui.divider())
    table.insert(rows, section(usageHeader(usage, host)))

    if #usage.windows == 0 then
        local note = usage.note
        if note then
            -- A paragraph, not a bare text node: the note is vendor-supplied
            -- ("not logged in", an API error) and would otherwise be clipped.
            table.insert(rows, ui.paragraph({ ui.text(note, "muted") }))
        end
        return
    end

    for _, w in usage.windows do
        local resets = w.resetsInSecs
        local suffix = if resets
            then string.format("%.0f%%  %s", w.usedPercent, formatCountdownSecs(resets))
            else string.format("%.0f%%", w.usedPercent)
        table.insert(rows, ui.gauge(w.label, w.usedPercent, suffix))
    end
end

local function pushSystemRows(rows: { ViewNode }, m: thurbox.SystemMetrics)
    table.insert(rows, ui.divider())
    table.insert(rows, section("System"))
    table.insert(rows, ui.gauge("CPU", m.cpuPercent))

    local ramPercent = if m.memoryTotal > 0
        then m.memoryUsed / m.memoryTotal * 100
        else 0
    table.insert(
        rows,
        ui.gauge("RAM", ramPercent, formatBytesPair(m.memoryUsed, m.memoryTotal))
    )

    -- thurbox's own footprint: a heads-up when accumulated worktrees have grown
    -- the data dir.
    local disk = m.thurboxDirBytes
    if disk then
        table.insert(
            rows,
            ui.paragraph({
                ui.text("Disk", "muted"),
                ui.text(string.format("  %s (thurbox dir)", formatBytes(disk))),
            })
        )
    end
end

local function pushAutomationRows(rows: { ViewNode }, automations: { thurbox.UpcomingAutomation })
    if #automations == 0 then
        return
    end
    table.insert(rows, ui.divider())
    table.insert(rows, section(string.format("Automations (%d)", #automations)))
    for _, entry in automations do
        table.insert(
            rows,
            ui.paragraph({
                ui.text(formatDueIn(entry.dueInSecs), "accent"),
                ui.text("  "),
                ui.text(entry.label, "secondary"),
            })
        )
    end
end

-- ── the pane ────────────────────────────────────────────────────────────────

return {
    render = function(_paneId: string): ViewNode
        local rows: { ViewNode } = {}

        -- Each reader is optional in the type because it is optional in fact:
        -- present only if the manifest declared its capability, and returning
        -- nothing before the host has published anything.
        local readSession = thurbox.activeSession
        local readMetrics = thurbox.systemMetrics
        local readAutomations = thurbox.upcomingAutomations

        local session = if readSession then readSession() else nil
        local metrics = if readMetrics then readMetrics() else nil
        local automations = if readAutomations then readAutomations() else {}

        if session then
            pushSessionRows(rows, session)
            pushRepoRows(rows, session)
            local git = session.git
            if git then
                pushGitRows(rows, git)
            end
            if metrics then
                pushSessionResourceRows(rows, metrics)
            end
            local agentMetrics = session.agentMetrics
            if agentMetrics then
                pushAgentRows(rows, agentMetrics)
            end
            local usage = session.usage
            if usage then
                pushUsageRows(rows, usage, session.remoteHost)
            end
        end

        if metrics then
            pushSystemRows(rows, metrics)
        end
        pushAutomationRows(rows, automations)

        return ui.list(rows)
    end,
}
