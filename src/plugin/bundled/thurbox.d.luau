--!strict
-- Type definitions for the `@thurbox` module, checked by `luau-analyze`.
--
-- Declarative only: the implementation lives in the Rust host
-- (`src/plugin/capabilities.rs`). Keep the two in step — this file is what a
-- plugin author's tooling reads.

export type StyleToken = "accent" | "muted" | "danger" | "success" | "warning"

export type ViewNode = {
    kind: string,
    id: string?,
    content: (string | number)?,
    style: StyleToken?,
    bold: boolean?,
    children: { ViewNode }?,
    lines: number?,
    motion: Motion?,
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
    divider: () -> ViewNode,
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
}

-- What a plugin's `init.luau` returns.
export type Plugin = {
    init: (() -> ())?,
    render: ((paneId: string) -> ViewNode)?,
}

-- The analyser only needs the *shape*; the real table is built by the host at
-- VM construction, so this stands in for it with no runtime meaning.
return (nil :: any) :: Thurbox
