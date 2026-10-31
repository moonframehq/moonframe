-- A list in the left sidebar, with a filter field over it. Choosing an
-- entry tells the editor and the inspector, by returning sends from
-- `update`.
local FILES = { "main.luau", "explorer.luau", "editor.luau", "inspector.luau", "status.luau" }

local search = use "search-field"

return mod {
	region = "left",
	title = "Explorer",

	init = function(previous)
		local state = previous or { selected = FILES[1] }
		state.filter = state.filter or ""
		return state
	end,

	update = function(state, msg)
		if type(msg) ~= "table" then
			return
		end
		-- The filter field sends { msg = "filter", value = <text> } on each
		-- edit, and { msg = "clear", value = <text> } on Esc.
		if msg.msg == "filter" then
			state.filter = msg.value
		elseif msg.msg == "clear" then
			state.filter = ""
		elseif msg.open then
			state.selected = msg.open
			return state, {
				{ to = "editor", msg = { open = msg.open } },
				{ to = "inspector", msg = { select = msg.open } },
			}
		end
	end,

	view = function(state)
		local items = {}
		local filter = string.lower(state.filter)
		for _, name in FILES do
			if string.find(string.lower(name), filter, 1, true) then
				table.insert(
					items,
					ui.item {
						name,
						selected = name == state.selected,
						msg = { open = name },
					}
				)
			end
		end
		return ui.panel {
			title = "Explorer",
			grow = true,
			scroll = true,
			-- A little air between the files, so the selected one stands apart.
			gap = "small",
			padding = "small",
			search { id = "filter", value = state.filter, placeholder = "Filter files…", change = "filter", cancel = "clear" },
			table.unpack(items),
		}
	end,
}
