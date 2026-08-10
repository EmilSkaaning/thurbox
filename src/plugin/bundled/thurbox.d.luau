--!strict
-- Type definitions for the `@thurbox` module, checked by `luau-analyze`.
--
-- Declarative only: the implementation lives in the Rust host
-- (`src/plugin/capabilities.rs`). Keep the two in step — this file is what a
-- plugin author's tooling reads.

-- Colour roles, resolved against the active theme at paint time. The
-- `status_*` family exists because a status indicator is the one case where
-- the token *is* the meaning, so an approximation would be wrong rather than
-- merely different.
export type StyleToken =
    "accent" | "muted" | "danger" | "success" | "warning"
    | "secondary" | "role" | "branch" | "added" | "border"
    | "status_working" | "status_blocked" | "status_done"
    | "status_idle" | "status_error" | "status_unreachable"

export type ViewNode = {
    kind: string,
    id: string?,
    content: (string | number)?,
    style: StyleToken?,
    bold: boolean?,
    children: { ViewNode }?,
    lines: number?,
    motion: Motion?,
    -- `gauge` only.
    label: string?,
    percent: number?,
    suffix: string?,
}

-- Motion the *kernel* drives: the plugin supplies the frames once and never
-- participates in a frame. `fps` is clamped to [1, 30]; `frames` must hold
-- 2-64 subtrees; `loop = false` holds the last frame and stops animating.
export type Motion = {
    kind: "cycle",
    frames: { ViewNode },
    fps: number?,
    loop: boolean?,
}

export type Ui = {
    text: (content: string | number, style: StyleToken?, bold: boolean?) -> ViewNode,
    row: (children: { ViewNode }?) -> ViewNode,
    -- One row of runs, each at its own width — a `label: value` line. Unlike
    -- `row`, which splits its area into equal shares, and unlike a single
    -- `text`, which carries one style. Holds only `text`, `cycle`, or a nested
    -- `line`; anything else is rejected, since its width would come from the
    -- area rather than from its content. Overflow is clipped, not wrapped.
    line: (children: { ViewNode }?) -> ViewNode,
    column: (children: { ViewNode }?) -> ViewNode,
    list: (children: { ViewNode }?) -> ViewNode,
    -- Like `line`, but soft-wraps onto further rows instead of clipping at the
    -- pane edge. Use it for text whose length you do not control; use `line`
    -- when a following row must not be pushed down. Same admissible children.
    paragraph: (children: { ViewNode }?) -> ViewNode,
    divider: () -> ViewNode,
    -- A labelled bar, two rows tall: the label at the left with `suffix` (or the
    -- rounded percentage) flush right, then the bar. The host resolves the
    -- geometry from the pane's width, which a plugin is never told. `percent` is
    -- clamped to 0-100 when drawn and must be a finite number.
    gauge: (label: string, percent: number, suffix: string?) -> ViewNode,
    spacer: (lines: number?) -> ViewNode,
    -- An animation the kernel evaluates on its own clock. The `id` keys its
    -- phase, so re-pushing the same cycle under the same id continues it
    -- rather than restarting it.
    cycle: (
        id: string,
        frames: { ViewNode },
        fps: number?,
        loop: boolean?
    ) -> ViewNode,
}

-- ── kernel state ────────────────────────────────────────────────────────────
--
-- Each reader below returns a *snapshot* the host published, not a live handle:
-- values are frozen at publication and a plugin never reaches into thurbox.
--
-- Two conventions run through all of it. An **absent** value is an absent field,
-- never a zero or an empty string — so `if session.parentName then` is the way to
-- ask. And a **quantity is a number**: byte counts, token counts, durations and
-- percentages arrive raw and the plugin formats them, because presentation is
-- the pane's job. What the host does resolve for you is what a sandboxed plugin
-- cannot compute for itself: anything needing a clock (a countdown), a filesystem
-- path (a repo's display name), a lookup across records (a parent's name), or a
-- rendering decision thurbox owns (a status's glyph and token).

-- How thurbox draws one session status. `token` is the style token *thurbox*
-- resolves the status to; use it rather than deriving one, so a pane cannot
-- disagree with the session list about which colour a state gets.
export type SessionStatusInfo = {
    name: string,
    label: string,
    icon: string,
    token: StyleToken,
}

export type GitStats = {
    filesChanged: number,
    insertions: number,
    deletions: number,
    dirty: boolean,
    ahead: number,
    behind: number,
}

-- What the session's agent reports about itself. Every field is optional: an
-- agent reports what it chooses to.
export type AgentMetrics = {
    modelDisplayName: string?,
    cliVersion: string?,
    totalCostUsd: number?,
    totalDurationMs: number?,
    totalApiDurationMs: number?,
    totalLinesAdded: number?,
    totalLinesRemoved: number?,
    totalInputTokens: number?,
    totalOutputTokens: number?,
    contextWindowSize: number?,
    usedPercentage: number?,
    cacheReadInputTokens: number?,
    cacheCreationInputTokens: number?,
}

-- One account rate-limit window. `resetsInSecs` is already a duration, since a
-- plugin has no clock to subtract an absolute instant from.
export type UsageWindow = {
    label: string,
    usedPercent: number,
    resetsInSecs: number?,
}

export type Usage = {
    windows: { UsageWindow },
    plan: string?,
    note: string?,
}

export type Session = {
    id: string,
    name: string,
    status: SessionStatusInfo,
    agent: string,
    parentName: string?,
    remoteHost: string?,
    hookWiring: string?,
    activity: string?,
    notification: string?,
    -- A display name, not a path: the sandbox has no path library.
    repoName: string?,
    branch: string?,
    additionalDirNames: { string },
    git: GitStats?,
    agentMetrics: AgentMetrics?,
    usage: Usage?,
}

export type SystemMetrics = {
    cpuPercent: number,
    memoryUsed: number,
    memoryTotal: number,
    sessionCpuPercent: number,
    sessionMemoryBytes: number,
    thurboxDirBytes: number?,
}

-- `dueInSecs` is whole seconds, the granularity a countdown is displayed at.
export type UpcomingAutomation = {
    label: string,
    dueInSecs: number,
}

export type Thurbox = {
    -- This plugin's own name. Always present.
    name: string,
    -- View-node constructors. Always present.
    ui: Ui,
    -- Present only when the manifest declares `capabilities = ["log"]`.
    log: ((message: string) -> ())?,
    -- Present only with `state-read` / `state-write`.
    stateRead: ((key: string) -> any)?,
    stateWrite: ((key: string, value: any) -> ())?,
    -- Present only with `sessions`. `nil` when no session is open, and also on
    -- the first render before the host has published anything.
    activeSession: (() -> Session?)?,
    -- Present only with `metrics`. `nil` until the first sample.
    systemMetrics: (() -> SystemMetrics?)?,
    -- Present only with `automations`. Always an array — "there are none" is a
    -- fact, so it needs no nil check — soonest first.
    upcomingAutomations: (() -> { UpcomingAutomation })?,
}

-- What a plugin's `init.luau` returns.
export type Plugin = {
    init: (() -> ())?,
    render: ((paneId: string) -> ViewNode)?,
    onKey: ((paneId: string, key: string) -> boolean)?,
}

-- A command's bound arguments: one entry per argument the manifest declared,
-- already validated and carrying its declared type (string / number / boolean).
-- An optional argument the caller omitted is simply absent.
export type CommandArgs = { [string]: any }

-- What a plugin's `service.luau` returns — the headless half, hosted with or
-- without a TUI.
--
-- `commands` is keyed by the *local* command id from the manifest (thurbox
-- namespaces it as `<plugin>.<id>` everywhere a user or an agent sees it). A
-- declared command with no entry here is an error naming it, not a silent
-- no-op. The returned value is converted to JSON, so return scalars, arrays, or
-- tables of them — not a function.
export type Service = {
    init: (() -> ())?,
    tick: (() -> ())?,
    run: ((verb: string, args: { string }) -> any)?,
    commands: { [string]: (args: CommandArgs) -> any }?,
}

-- The analyser only needs the *shape*; the real table is built by the host at
-- VM construction, so this stands in for it with no runtime meaning.
return (nil :: any) :: Thurbox
