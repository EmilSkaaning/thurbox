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
    | "secondary" | "role" | "branch" | "accent_bright" | "added"
    | "diff_added" | "diff_removed" | "border"
    | "status_working" | "status_blocked" | "status_done"
    | "status_idle" | "status_error" | "status_unreachable"

-- Which side of a change a diff row is on. Like `selected` it is a *role*: the
-- host resolves it to the theme's added-row or removed-row background, so the
-- pane names no colour. Unlike `selected` it leaves the foreground to `style`,
-- because a diff body's colours are the pane's while the tint is the only thing
-- carrying add versus remove. A context row carries no tint at all.
export type DiffTint = "added" | "removed"

-- A style given as one table instead of positionally. Every field is optional,
-- and the fields are the node's own — so this is a spelling, not a second model.
-- It exists because a style now carries more than a call can reasonably order:
-- `tint` has no positional slot and never will.
-- `ellipsize` is the one field that is neither a colour nor an attribute: the run
-- **yields its width** to the others on its line. When the line overflows the host
-- cuts this run with an `…` and every other run keeps its columns, which is how a
-- trailing marker survives a title too long for the pane. Consecutive yielding runs
-- are cut as one piece of text, so a title split at its search-match offsets gets a
-- single ellipsis. It does nothing in a `paragraph`, which wraps rather than clips.
export type TextStyle = {
    token: StyleToken?,
    bold: boolean?,
    dim: boolean?,
    underline: boolean?,
    selected: boolean?,
    tint: DiffTint?,
    ellipsize: boolean?,
}

export type ViewNode = {
    kind: string,
    id: string?,
    content: (string | number)?,
    style: StyleToken?,
    -- Emphasis: terminal attributes applied over whatever colour `style`
    -- resolves to, never instead of it. A list row needs all three — the
    -- selected row is bold, a row a search filtered out is dim, and a matched
    -- run is underlined.
    bold: boolean?,
    underline: boolean?,
    dim: boolean?,
    -- Not an emphasis: asks the host for the theme's selection foreground on its
    -- selection background, which *replaces* whatever colour `style` resolves to
    -- rather than layering over it. Use it for the runs of the row a cursor is
    -- on; combine it with `bold` freely.
    selected: boolean?,
    -- Declares the run's row an insertion or a deletion; `selected` beats it.
    tint: DiffTint?,
    -- `fill` only: the single character repeated across the line's residue.
    glyph: string?,
    children: { ViewNode }?,
    -- `list` only: which child (1-based) the cursor is on. Named apart from the
    -- boolean `selected` above because they are different questions — one asks
    -- *which row*, the other says *this run is on it*. When the list has more
    -- children than rows, the host windows it to keep that child visible, which
    -- is the only way a pane scrolls since a plugin is never told its height.
    -- Must index an existing child; `0` is an error, not "none".
    selectedRow: number?,
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
    -- The emphasis flags are positional because the node's own fields are the
    -- canonical form and `bold` cannot stop being the third argument; they read
    -- in declaration order (bold, underline, dim).
    -- `style` is either a token name (the positional form, whose flags follow it)
    -- or a whole `TextStyle` table. Both build the same node; the table is the
    -- only form that reaches `tint` and `ellipsize`.
    text: (
        content: string | number,
        style: (StyleToken | TextStyle)?,
        bold: boolean?,
        underline: boolean?,
        dim: boolean?,
        selected: boolean?
    ) -> ViewNode,
    -- One glyph repeated across whatever width is left on the line after every
    -- other run has taken its own. The host resolves that width, which is the
    -- point: it is how a run reaches the pane's right edge without a plugin ever
    -- being told how wide the pane is. Put it last for a trailing rule or a row
    -- background; put it *before* a run to push that run flush right. Admissible
    -- wherever `text` is.
    fill: (glyph: string, style: (StyleToken | TextStyle)?) -> ViewNode,
    row: (children: { ViewNode }?) -> ViewNode,
    -- One row of runs, each at its own width — a `label: value` line. Unlike
    -- `row`, which splits its area into equal shares, and unlike a single
    -- `text`, which carries one style. Holds only `text`, `cycle`, or a nested
    -- `line`; anything else is rejected, since its width would come from the
    -- area rather than from its content. Overflow is clipped, not wrapped.
    line: (children: { ViewNode }?) -> ViewNode,
    column: (children: { ViewNode }?) -> ViewNode,
    -- A list of rows. `selected` is the 1-based index of the row the cursor is
    -- on; give it and the host scrolls the list to keep that row visible when
    -- there are more rows than the pane has lines. Omit it and the list draws
    -- from its first row and is clipped. An index outside the children is an
    -- error naming the field — including `0`, so a 0-based index fails loudly
    -- rather than pinning the cursor to the first row.
    --
    -- `scrollbar` asks for the scroll track a pane shows on its rightmost column
    -- when the list is longer than the lines it got. You declare *that* the list
    -- scrolls; the host owns which column the track occupies, whether the list is
    -- long enough to need one, and how the thumb looks — so nothing here reports a
    -- width or a height back to you. A click on the track is never delivered as a
    -- click on a row.
    list: (children: { ViewNode }?, selectedRow: number?, scrollbar: boolean?) -> ViewNode,
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
    -- The row's id, which is what `setAutomationEnabled`, `runAutomation` and
    -- `deleteAutomation` take. Present whether or not you hold a write capability;
    -- a pane that only draws ignores it.
    id: number,
    label: string,
    dueInSecs: number,
}

-- One row of the automations pane — *every* automation, unlike
-- `UpcomingAutomation`, which is filtered to the ones due to fire. Both are read
-- through the same `automations` capability, because "reads the automations you
-- have scheduled" is one question to ask a user whichever view you want.
--
-- The interesting field is the one that is **not** here: the row's summary. The
-- pane shows `<schedule> · <action> · <when>` and you compose it, from the parts
-- below. Two of them the host resolves because a sandbox cannot: `schedule` (a
-- cron expression's meaning is thurbox's own vocabulary) and `dueInSecs` (you have
-- no clock). The separator, the ordering, and the precedence that shows `disabled`
-- for a disabled automation, a countdown for a scheduled one and a placeholder for
-- an enabled one with no next run, are yours.
--
-- `dueInSecs` is **absent**, not zero, when there is no next run: "never" and "due
-- now" are different rows.
--
-- `matchPositions` are zero-based *byte* offsets into `name`, with the same two
-- edge rules `TaskEntry` documents.
export type AutomationEntry = {
    -- The row's id, and the only thing to pass to `setAutomationEnabled`,
    -- `runAutomation` and `deleteAutomation`. Address the row you drew rather than
    -- an index: your cursor and the host's list agree today and an index would
    -- silently act on the wrong record the moment they did not.
    id: number,
    name: string,
    enabled: boolean,
    action: string,
    schedule: string,
    dueInSecs: number?,
    dimmed: boolean,
    matchPositions: { number },
}

-- The automations pane's rows, with the cursor's position in them.
--
-- `cursor` is the 1-based scroll anchor — hand it straight to `ui.list`'s second
-- argument and your pane scrolls. `cursorVisible` is whether that row is **drawn**
-- as the cursor, which is a different question: thurbox's pane windows to its
-- cursor whether or not it holds focus and highlights it only when it does (or when
-- a global search is previewing a row there). Anchoring on the appearance instead
-- would make your copy jump back to its first row whenever thurbox's pane lost
-- focus.
--
-- Two fields where `TaskList` has an anchor plus a `selected` on every row, and the
-- difference is deliberate: here the whole cursor appears and disappears with the
-- pane's focus, so a per-row flag would be one fact in n + 1 places — and a
-- publication could then highlight one row while scrolling to another.
--
-- `focused` is whether **the interface's** automations pane holds focus. Read it
-- twice, because it has two readings and which one applies is your manifest's
-- choice:
--
--  * a pane that declares no `key_context` is a *reproduction*, and this is a fact
--    about thurbox's own pane rather than about yours — the host tells such a pane
--    nothing about its own focus, so it knows only that a key arrived, which only a
--    focused pane receives;
--  * a pane that declares `key_context = "Automations"` **is** that pane, so this is
--    true exactly while your pane holds focus.
export type AutomationList = {
    entries: { AutomationEntry },
    cursor: number?,
    cursorVisible: boolean,
    focused: boolean,
}

-- One row of the task list.
--
-- The status is a *name*, not a glyph and not a token: which checkbox a state
-- draws as, and in what colour, is the pane's decision. What the host does
-- resolve is what a plugin cannot observe — the cursor's row (which also moves
-- under a global-search preview), the search's verdict on this row, and whether
-- the task has an open related session.
--
-- `matchPositions` are zero-based *byte* offsets into `title`. Splitting them
-- into runs is presentation, so it is yours; an offset past the end of the title
-- ends the scan and one that does not land on a UTF-8 character boundary is
-- skipped, because a title may have been shortened after the offsets were
-- computed.
export type TaskEntry = {
    -- The row's id, which is what `setTaskStatus` and `deleteTask` take. Address
    -- the row you drew rather than matching on a title you fitted yourself.
    id: number,
    title: string,
    status: string,
    selected: boolean,
    dimmed: boolean,
    linked: boolean,
    matchPositions: { number },
}

-- `focused` is whether the interface's task pane holds focus, which changes what an
-- empty list should say: naming the key that adds a task is only honest when the
-- pane can receive it. As with `AutomationList`, it describes *thurbox's* pane for a
-- reproduction and **your** pane once you declare `key_context = "Tasks"` — the two
-- coincide, because then there is one task pane and it is yours.
--
-- `cursor` is the 1-based row the cursor is on — hand it straight to `ui.list`'s
-- second argument and your pane scrolls. It is **not** the same fact as a row's
-- `selected`: this one says which row, that one says the row is drawn as the
-- cursor's, and only the second waits for the pane to be focused. Anchoring on
-- `selected` instead would make your copy jump back to its first row whenever
-- thurbox's own pane loses focus.
export type TaskList = {
    entries: { TaskEntry },
    cursor: number?,
    focused: boolean,
}

-- One row of the file tree the file viewer has open.
--
-- `name` is a **basename**: there is no path here, and none is needed to draw the
-- pane. `depth` is what indentation is built from — a root is 0.
--
-- `matched` is the running search's verdict on this row, and it is `true` when no
-- search is running, so an unsearched tree draws in its ordinary colours without
-- the pane asking whether a search exists. The verdict crosses and the query does
-- not: you need no matcher, so yours cannot disagree with thurbox's.
export type FileNode = {
    name: string,
    depth: number,
    isDir: boolean,
    expanded: boolean,
    matched: boolean,
}

-- The open file tree.
--
-- `selected` is the 1-based row the cursor is on — hand it straight to
-- `ui.list`'s second argument and the pane scrolls. Absent when there is no
-- cursor. `nerdFont` says which marker set to draw; the glyphs themselves are
-- yours to choose.
export type FileTree = {
    nodes: { FileNode },
    selected: number?,
    nerdFont: boolean,
}

-- One row of the open review, tagged by `row`.
--
-- Two tags, and they mean different things: `row` is which **kind of row** this is
-- (`"file"`, `"hunk"`, `"line"`, `"comment"`, `"summaryHeader"`, `"info"`), while a
-- line row's `kind` is which side of the change it is on. The fields present depend
-- on `row`, so branch on it first.
--
-- Facts, not glyphs. A file row gives you `status` as a name (`"modified"`,
-- `"added"`, `"deleted"`, `"renamed"`), `folded` and `reviewed` — the status letter,
-- the fold chevron and the reviewed mark are yours to draw. A comment row gives you
-- `classification` (`"issue"`, `"suggestion"`, `"note"`, `"praise"`), the body's
-- first line in `text`, and `more` when there are further lines.
--
-- `oldSpan`/`newSpan` are counted over the hunk's *whole* line list, which the
-- published window may not contain — like `numberWidth`, they cross so your `@@`
-- ranges are the review's own.
--
-- On a `line` row, `oldNo`/`newNo` are absent on the side the line is not on: an
-- insertion has no old-side number, and `0` would be a number a pane could print.
-- Its `text` is **raw** — not split into coloured runs, not windowed to any scroll
-- offset, not padded to any width. How a diff body reads is the pane's decision, so
-- the highlighting is yours to do, and `path` is there because a comment marker
-- depends on the file's extension.
--
-- `label` on a `summaryHeader` is the one row whose **text** crosses: it names a
-- thurbox keystroke, which a plugin pane never receives, so composing the string
-- yourself would advertise an action you cannot perform. Draw it verbatim.
export type ReviewRow = {
    row: string,
    -- `file`
    path: string?,
    status: string?,
    added: number?,
    removed: number?,
    folded: boolean?,
    reviewed: boolean?,
    -- `hunk`
    oldStart: number?,
    oldSpan: number?,
    newStart: number?,
    newSpan: number?,
    heading: string?,
    -- `line` (`path`, above, is a line's too)
    oldNo: number?,
    newNo: number?,
    kind: string?,
    -- `line`, `comment` and `info`
    text: string?,
    -- `comment`
    classification: string?,
    more: boolean?,
    -- `summaryHeader`
    label: string?,
}

-- The open review's row stream.
--
-- `cursor` is the 1-based row the review's cursor is on — hand it to `ui.list`'s
-- second argument and the pane scrolls. `numberWidth` is the width of each of the
-- gutter's two number columns; it crosses because it is computed over the *whole*
-- review, which the published window is not, so a pane deriving it would draw a
-- narrower gutter than thurbox's own.
export type ReviewDiff = {
    rows: { ReviewRow },
    cursor: number?,
    numberWidth: number,
}

-- One row of the session list.
--
-- `status` is the row's state in the four forms a pane needs, exactly as
-- `Session.status` carries it — including the token, so a pane cannot disagree
-- with thurbox about which colour a state gets.
--
-- `group` is the repo-group label when this row is the **first** of its group and
-- absent on every other row, so `if row.group then` is where a header goes.
-- `depth` is nesting under a parent within that group (a root is 0);
-- `crossGroupChild` is a child whose parent renders in a *different* group, which
-- is marked rather than indented.
--
-- `matchPositions` are zero-based byte offsets into `name`, segmented exactly as
-- `TaskEntry.matchPositions` are. `activity` and `notification` are what the
-- agent last reported, **unresolved**: which of the two a row shows — and whether
-- it shows one at all — is your pane's rule. Nothing here is fitted to a column.
export type SessionRow = {
    name: string,
    status: SessionStatusInfo,
    group: string?,
    depth: number,
    crossGroupChild: boolean,
    remote: boolean,
    worktree: boolean,
    selected: boolean,
    dimmed: boolean,
    matchPositions: { number },
    activity: string?,
    notification: string?,
}

-- The session list, in the order the pane renders it.
--
-- There is deliberately no cursor index here, unlike `FileTree.selected`: a
-- group's first row is preceded by a header, so the rows and a list's children
-- are not one for one and the index `ui.list` wants is one only your pane knows.
-- Each row says whether the cursor is on *it*; count the children you emit.
export type SessionList = {
    rows: { SessionRow },
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
    -- Present only with `sessions`, the same grant `activeSession` needs. Always
    -- a table — "no session is open" is a fact — whose `rows` are in the order
    -- thurbox's own session list renders them, group headers included as a label
    -- on the row that opens each group. Names and reported text are *not* fitted
    -- to any width: your pane's size is not yours to know.
    sessionList: (() -> SessionList)?,
    -- Present only with `metrics`. `nil` until the first sample.
    systemMetrics: (() -> SystemMetrics?)?,
    -- Present only with `automations`. Always an array — "there are none" is a
    -- fact, so it needs no nil check — soonest first, and filtered to the
    -- automations that are enabled and scheduled.
    upcomingAutomations: (() -> { UpcomingAutomation })?,
    -- Present only with `automations`, the same grant `upcomingAutomations` needs.
    -- Always a table. The wider view: *every* automation, enabled and disabled, in
    -- the order thurbox's own pane lists them, with the cursor's row. Names are
    -- **not** fitted to any width: your pane's size is not yours to know.
    automations: (() -> AutomationList)?,
    -- Present only with `tasks`. Always a table — "there are no tasks" is a
    -- fact — whose `entries` are in the order the pane lists them. Titles are
    -- *not* fitted to any width: your pane's size is not yours to know, so draw
    -- the row and let the host clip it.
    tasks: (() -> TaskList)?,
    -- Present only with `files`. Always a table — "the viewer has no tree open"
    -- is a fact. Reports the tree the *user* has expanded in thurbox's file
    -- viewer: one basename per visible row, with its depth and expansion state.
    --
    -- This is **not** a filesystem binding. It lists no directory, reads no file,
    -- reveals no path, and causes no I/O: it reads a snapshot the host already
    -- built from the pane's own state. A directory the user has not expanded is
    -- not in it, dotfiles never entered it, and nothing outside the active
    -- session's own directories can appear.
    files: (() -> FileTree)?,
    -- Present only with `review`. Always a table — "no review is open" is a fact.
    -- Reports the diff the *user* has open in thurbox's code-review view.
    --
    -- This is **not** a git binding. It produces no diff of your choosing, names
    -- no revision range, reads no file and runs no command: it reads a snapshot
    -- the host already built from the pane's own state. The list is bounded, so a
    -- large diff arrives truncated with no cursor rather than as a tree the host
    -- would refuse for exceeding its node budget.
    review: (() -> ReviewDiff)?,

    -- ── changing records ────────────────────────────────────────────────────
    --
    -- The only bindings here that do not read. Each takes the row id the matching
    -- reader published, changes exactly that one record, and returns whether it
    -- was there — `false` is "no such record", not a failure. An error is raised
    -- only when the argument is not one the kernel accepts, or when storage cannot
    -- be reached.
    --
    -- What is deliberately absent, under either write capability: creating a task
    -- or an automation, editing a title, description, schedule, action or command,
    -- naming a program, running one, touching a session, or reaching the
    -- filesystem. A pane reproduces a native pane's *keys*; authoring happens in
    -- thurbox's own editor.

    -- Present only with `tasks-write`. `status` is one of "todo", "in_progress" or
    -- "done" — any other string is an error rather than a silent default, because
    -- writing a status you did not ask for is worse than failing.
    setTaskStatus: ((id: number, status: string) -> boolean)?,
    -- Present only with `tasks-write`. Soft-deletes, exactly as the tasks pane's
    -- delete key does.
    deleteTask: ((id: number) -> boolean)?,
    -- Present only with `automations-write`. Enabling recomputes the automation's
    -- next occurrence from its schedule; disabling clears it — the same rule the
    -- native pane's toggle follows, so the two cannot leave different state.
    setAutomationEnabled: ((id: number, enabled: boolean) -> boolean)?,
    -- Present only with `automations-write`. Marks the automation due; the
    -- **kernel** fires it on its next pass, under the claim that de-duplicates a
    -- running TUI and a headless tick. Your VM runs nothing, spawns nothing, and
    -- cannot choose what the automation does. Marking a pending run again is the
    -- same request, not a second run.
    runAutomation: ((id: number) -> boolean)?,
    -- Present only with `automations-write`. Removes the automation and its run
    -- history; unlike a task's delete this is not recoverable.
    deleteAutomation: ((id: number) -> boolean)?,
}

-- What a plugin's `init.luau` returns.
export type Plugin = {
    init: (() -> ())?,
    render: ((paneId: string) -> ViewNode)?,
    -- Called for each key while one of this plugin's panes is focused, if the
    -- manifest declared the `input` capability. Return `true` to consume the key;
    -- anything else falls through to thurbox, so a pane can never trap the user.
    --
    -- `binding` is the id of the `[[keybindings]]` entry the keypress resolved to,
    -- or `nil` when it resolved to none. Switch on it rather than on `key`: the
    -- user may rebind the chord in the F1 editor, and the binding's id does not
    -- change when they do. `key` is still delivered in both cases, which is what a
    -- pane collecting text needs.
    onKey: ((paneId: string, key: string, binding: string?) -> boolean)?,
    -- Called when one of this pane's rows is clicked, if the manifest declared
    -- `input` (a click is input, and is gated by the same capability). `row` is the
    -- 1-based index in your pane's **outermost list** — the same numbering
    -- `ui.list`'s `selected` argument uses, so the row you highlight and the row a
    -- user clicks are the same number.
    --
    -- You are never told *where* the click landed: no coordinate, no rect, no
    -- width. A click below your last row, or on a pane whose tree has no list,
    -- focuses the pane and calls nothing. Returning `true` says you handled it;
    -- unlike a key there is nothing for thurbox to fall through to.
    onClick: ((paneId: string, row: number) -> boolean)?,
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
