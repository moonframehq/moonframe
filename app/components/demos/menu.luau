-- A tab's menu: click a row to pick it. "Pin tab" toggles its check,
-- "Split right" counts panes, and the picked row stays highlighted. "Move
-- to" opens a submenu beside it; picking there moves the tab.
local menu = use "menu"

return mod {
	init = function(previous)
		return previous or { pinned = true, panes = 2, last = "rename", place = "left", expanded = "move" :: string? }
	end,

	update = function(state, msg)
		if msg.expand then
			state.expanded = if state.expanded == msg.expand then nil else msg.expand
		elseif msg.pick then
			state.last = msg.pick
			state.expanded = nil
			local place = string.match(msg.pick, "^move:(.+)$")
			if place then
				state.place = place
			end
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
					expanded = state.expanded,
					items = {
						{ heading = "Tab" },
						{ id = "rename", label = "Rename…", icon = "pencil", shortcut = "cmd-r" },
						{ id = "duplicate", label = "Duplicate", icon = "copy", shortcut = "cmd-d" },
						{ id = "pin", label = "Pin tab", icon = "pin", checked = state.pinned },
						{ id = "split", label = "Split right", icon = "columns-2", detail = `{state.panes} panes` },
						{
							id = "move",
							label = "Move to",
							icon = "columns-2",
							items = {
								{ id = "move:left", label = "Left pane", checked = state.place == "left" },
								{ id = "move:right", label = "Right pane", checked = state.place == "right" },
								{ id = "move:window", label = "New window", checked = state.place == "window" },
							},
						},
						{ id = "reveal", label = "Reveal in Finder", icon = "folder-open", disabled = true },
						"separator",
						{ id = "close", label = "Close tab", icon = "close", shortcut = "cmd-w", danger = true },
					} :: { any },
				},
			},
			ui.text { `Picked: {state.last} · tab in {state.place}`, tone = "muted", size = "small" },
		}
	end,
}
