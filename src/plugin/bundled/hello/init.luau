--!strict
-- A minimal thurbox plugin, and the worked example a plugin author copies.
-- Everything it draws is a view tree the kernel renders; the plugin never
-- touches the terminal itself.

local thurbox = require("@thurbox")
local ui = thurbox.ui

return {
    render = function(paneId: string)
        return ui.list({
            ui.text("thurbox plugin host", "accent", true),
            ui.divider(),
            -- A line composes runs at their own width, so a muted label and an
            -- unstyled value share one row. `ui.row` would have given each half
            -- the pane.
            ui.line({
                ui.text("pane: ", "muted"),
                ui.text(paneId),
            }),
            ui.spacer(1),
            ui.text("This pane is Luau, not Rust."),
            ui.text("Edit init.luau and restart to change it.", "muted"),
        })
    end,
}
