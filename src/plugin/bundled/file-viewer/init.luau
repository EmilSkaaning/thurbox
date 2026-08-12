--!strict
-- thurbox's file viewer, in Luau.
--
-- Not a reproduction any more: `src/ui/file_viewer.rs` is deleted and this is the
-- pane (ADR-58). `tests/bundled_file_viewer.rs` still holds it to that pane's tree —
-- the ten checked-in recordings were generated from `ui::file_viewer::file_tree`
-- while it existed, so they are a statement about the pane rather than about this
-- file, and they are what makes this the measurement of whether a third party could
-- have written it.
--
-- Three things are worth reading for what they imply about writing a tree pane:
--
-- 1. **It scrolls without knowing its height.** The last line hands the cursor's
--    row back to the host on the list node, and the *kernel* windows the list to
--    keep that row visible. There is deliberately no way for this plugin to ask
--    how many rows it got — that would make rendering height-dependent, so a
--    resize would have to re-enter this VM before the frame that needed it.
-- 2. **It reads no directory.** `thurbox.files()` is not a filesystem binding: it
--    reports the tree the *user* has expanded in the pane, one basename per row.
--    Nothing here can list a directory, read a file, or learn where on disk the
--    tree is — and it needs none of that, because four of the five facts a row
--    draws (depth, expansion, the search's verdict, the cursor) are not on disk
--    at all. Nor does it need any of it to *answer a key*: the two keys that reach
--    outside the process — expanding a directory, opening a file — are the
--    kernel's, performed on the kernel's own dispatch.
-- 3. **The glyphs are this plugin's.** The host publishes whether nerd-font
--    glyphs are enabled; which glyph a collapsed directory draws as is a
--    presentation decision, and presentation is the pane's.
--
-- What this pane does *not* draw is the search **bar** — the bordered block with the
-- query and the match counter below the tree. It is **kernel chrome**: the query, the
-- caret and the counter are kernel state, the kernel owns the `/` key that collects
-- them, and a pane redrawing them would be a second renderer for one fact. So the seat
-- reserves its three rows and the kernel paints it there, exactly where the native pane
-- drew it. The search's effect on the rows *is* this pane's: see `matched` below.

local thurbox = require("@thurbox")
local ui = thurbox.ui

type ViewNode = thurbox.ViewNode
type StyleToken = thurbox.StyleToken
type FileNode = thurbox.FileNode

-- A resolved text style, in the parts a `text` node carries. `selected` is not an
-- emphasis: it asks the host for the theme's selection pair, which replaces the
-- token's colour rather than layering over it.
type RowStyle = {
    token: StyleToken?,
    bold: boolean,
    selected: boolean,
}

local function text(content: string, style: RowStyle): ViewNode
    return ui.text(content, style.token, style.bold, false, false, style.selected)
end

-- ── markers ─────────────────────────────────────────────────────────────────

-- Two spaces per level of nesting, so a child sits under its parent's name.
local function indent(depth: number): string
    return string.rep("  ", depth)
end

-- A disclosure triangle, or a nerd-font folder/file when the user has those
-- enabled. A file's plain-glyph marker is two spaces: it takes the same width as
-- a directory's triangle so names line up whether or not a row can be expanded.
local function marker(node: FileNode, nerdFont: boolean): string
    if nerdFont then
        if not node.isDir then
            return "\u{f15b} "
        end
        return if node.expanded then "\u{f07c} " else "\u{f07b} "
    end
    if not node.isDir then
        return "  "
    end
    return if node.expanded then "▾ " else "▸ "
end

-- ── row styles ──────────────────────────────────────────────────────────────

-- The indent and marker. Never bold — only the name is, which is what makes the
-- name the thing the eye lands on.
local function prefixStyle(selected: boolean): RowStyle
    if selected then
        return { token = nil, bold = false, selected = true }
    end
    return { token = "muted", bold = false, selected = false }
end

-- The name. Selection wins over everything; then a row the running search
-- excluded recedes to muted; then a directory takes the accent and bold, which is
-- what separates structure from contents at a glance. A file has *no* token: it
-- draws in the theme's primary text, which is what a file has always looked like.
local function nameStyle(node: FileNode, selected: boolean): RowStyle
    if selected then
        return { token = nil, bold = true, selected = true }
    elseif not node.matched then
        return { token = "muted", bold = false, selected = false }
    elseif node.isDir then
        return { token = "accent", bold = true, selected = false }
    end
    return { token = nil, bold = false, selected = false }
end

-- ── rows ────────────────────────────────────────────────────────────────────

local function rowNode(node: FileNode, selected: boolean, nerdFont: boolean): ViewNode
    return ui.line({
        text(indent(node.depth) .. marker(node, nerdFont), prefixStyle(selected)),
        text(node.name, nameStyle(node, selected)),
    })
end

-- ── the pane ────────────────────────────────────────────────────────────────

return {
    render = function(_paneId: string): ViewNode
        local readFiles = thurbox.files
        local files = if readFiles then readFiles() else nil
        local nodes = if files then files.nodes else {}

        if #nodes == 0 then
            -- The same line the native pane draws for a session with no
            -- directories — and what a plugin sees before the viewer has ever
            -- been opened, since the host publishes the tree the pane has open.
            return ui.list({ ui.text("No folders", "muted") })
        end

        local nerdFont = if files then files.nerdFont else false
        local selected = if files then files.selected else nil

        local rows: { ViewNode } = {}
        for i, node in ipairs(nodes) do
            table.insert(rows, rowNode(node, i == selected, nerdFont))
        end
        -- The cursor's row goes back to the host, which is what makes this list
        -- scroll. One-based, like the table it indexes. The third argument asks
        -- for the scroll track the native pane shows while the tree overflows;
        -- which column it occupies is the host's, so this pane still learns no
        -- dimension. It is inert here in a way the native pane's is not — the
        -- thumb reports a cursor this plugin does not own, so there is nothing a
        -- drag could move.
        return ui.list(rows, selected, true)
    end,
}
