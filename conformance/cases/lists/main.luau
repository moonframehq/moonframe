-- A long list's rows, built from data the view prepared.
local FILES = { "README.md", "main.luau", "style.luau", "notes.txt" }

return mod {
	init = function(previous)
		return previous or { selected = 1 }
	end,
	update = function(state, msg)
		if type(msg) == "table" and msg.pick then
			state.selected = msg.pick
		end
	end,
	view = function(state)
		local names = {}
		for _, name in FILES do
			table.insert(names, string.upper(name))
		end
		return ui.panel {
			title = "Files",
			grow = true,
			ui.list {
				id = "files",
				count = #names,
				row_height = 24,
				grow = true,
				row = function(i)
					return ui.item { names[i], selected = i == state.selected, msg = { pick = i } }
				end,
			},
		}
	end,
}
