-- A tab's menu: click a row to pick it. "Pin tab" toggles its check,
-- "Split right" counts panes, and the picked row stays highlighted.
local menu = use "menu"

return mod {
	init = function(previous)
		return previous or { pinned = true, panes = 2, last = "rename" }
	end,

	update = function(state, msg)
		if msg.pick then
			state.last = msg.pick
			if msg.pick == "pin" then
				state.pinned = not state.pinned
			elseif msg.pick == "split" then
				state.panes = state.panes % 4 + 1
			end
		end
	end,

	view = function(state)
		return ui.column {
			gap = "normal",
			ui.column {
				width = 240,
				style = { border = "all", border_color = "border", radius = 8, padding = 4 },
				menu {
					highlighted = state.last,
					items = {
						{ heading = "Tab" },
						{ id = "rename", label = "Rename…", icon = "pencil", shortcut = "cmd-r" },
						{ id = "duplicate", label = "Duplicate", icon = "copy", shortcut = "cmd-d" },
						{ id = "pin", label = "Pin tab", icon = "pin", checked = state.pinned },
						{ id = "split", label = "Split right", icon = "columns-2", detail = `{state.panes} panes` },
						{ id = "reveal", label = "Reveal in Finder", icon = "folder-open", disabled = true },
						"separator",
						{ id = "close", label = "Close tab", icon = "close", shortcut = "cmd-w", danger = true },
					} :: { any },
				},
			},
			ui.text { `Picked: {state.last}`, tone = "muted", size = "small" },
		}
	end,
}
