--!strict
-- Type definitions for the `@thurbox` module, checked by `luau-analyze`.
--
-- Declarative only: the implementation lives in the Rust host
-- (`src/plugin/capabilities.rs`). Keep the two in step — this file is what a
-- plugin author's tooling reads.

export type StyleToken = "accent" | "muted" | "danger" | "success" | "warning"

export type ViewNode = {
    kind: string,
    content: (string | number)?,
    style: StyleToken?,
    bold: boolean?,
    children: { ViewNode }?,
    lines: number?,
}

export type Ui = {
    text: (content: string | number, style: StyleToken?, bold: boolean?) -> ViewNode,
    row: (children: { ViewNode }?) -> ViewNode,
    column: (children: { ViewNode }?) -> ViewNode,
    list: (children: { ViewNode }?) -> ViewNode,
    divider: () -> ViewNode,
    spacer: (lines: number?) -> ViewNode,
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
