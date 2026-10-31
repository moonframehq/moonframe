-- A list in the left sidebar. Choosing an entry tells the editor and the
-- inspector, by returning sends from `update`.
local FILES = { "main.luau", "explorer.luau", "editor.luau", "inspector.luau", "status.luau" }

return mod {
	region = "left",
	title = "Explorer",

	init = function(previous)
		return previous or { selected = FILES[1] }
	end,

	update = function(state, msg)
		if type(msg) == "table" and msg.open then
			state.selected = msg.open
			return state, {
				{ to = "editor", msg = { open = msg.open } },
				{ to = "inspector", msg = { select = msg.open } },
			}
		end
	end,

	view = function(state)
		local items = {}
		for index, name in FILES do
			items[index] = ui.item {
				name,
				selected = name == state.selected,
				msg = { open = name },
			}
		end
		return ui.panel {
			title = "Explorer",
			grow = true,
			scroll = true,
			-- A little air between the files, so the selected one stands apart.
			gap = "small",
			padding = "small",
			table.unpack(items),
		}
	end,
}
