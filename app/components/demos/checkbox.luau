-- Files to commit: click a file to check it; "All files" checks or clears
-- them all and shows a dash when only some are checked.
local checkbox = use "checkbox"

local FILES = { "src/tabs.rs", "src/menu.rs", "app/style.luau", "README.md" }

return mod {
	init = function(previous)
		return previous or { checked = { ["src/tabs.rs"] = true, ["app/style.luau"] = true }, sign = false }
	end,

	update = function(state, msg)
		if msg == "toggle-all" then
			local all = true
			for _, file in FILES do
				all = all and state.checked[file] == true
			end
			for _, file in FILES do
				state.checked[file] = not all or nil
			end
		elseif msg == "toggle-sign" then
			state.sign = not state.sign
		elseif msg.file then
			state.checked[msg.file] = not state.checked[msg.file] or nil
		end
	end,

	view = function(state)
		local count = 0
		local rows = {}
		for _, file in FILES do
			if state.checked[file] then
				count += 1
			end
			table.insert(rows, checkbox { label = file, checked = state.checked[file] == true, msg = { file = file } })
		end
		return ui.column {
			gap = "normal",
			checkbox {
				label = "All files",
				checked = count == #FILES,
				indeterminate = count > 0 and count < #FILES,
				msg = "toggle-all",
			},
			ui.column { gap = "small", style = { padding = { left = 24 } }, table.unpack(rows) },
			ui.divider(),
			checkbox {
				label = "Sign commit",
				description = "Needs a YubiKey tap.",
				checked = state.sign,
				msg = "toggle-sign",
			},
			checkbox { label = "Amend last commit", disabled = true },
		}
	end,
}
