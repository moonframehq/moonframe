-- Tasks and their priority: click a glyph to raise it, from none to
-- urgent and back; the control switches the size.
local priority_icon = use "priority-icon"
local segmented = use "segmented"

local NEXT = { none = "low", low = "medium", medium = "high", high = "urgent", urgent = "none" }

return mod {
	init = function(previous)
		return previous
			or {
				size = "14",
				tasks = {
					{ title = "Tidy the settings page", priority = "none" },
					{ title = "Dark mode for the board", priority = "low" },
					{ title = "Faster project search", priority = "medium" },
					{ title = "Attachments over 25 MB fail", priority = "high" },
					{ title = "Data loss when two people edit", priority = "urgent" },
				},
			}
	end,

	update = function(state, msg)
		if type(msg) ~= "table" then
			return
		end
		if msg.select then
			state.size = msg.select
		elseif msg.raise then
			local task = state.tasks[msg.raise]
			task.priority = NEXT[task.priority]
		end
	end,

	view = function(state)
		local size = tonumber(state.size)
		local rows = {}
		for index, task in state.tasks do
			rows[index] = ui.row {
				gap = "normal",
				align = "center",
				ui.row { msg = { raise = index }, priority_icon { priority = task.priority, size = size } },
				ui.text { task.title },
				ui.spacer(),
				priority_icon { priority = task.priority, size = size, label = true },
			}
		end
		return ui.column {
			gap = "normal",
			width = 420,
			segmented {
				selected = state.size,
				small = true,
				options = { { id = "14", label = "14 px" }, { id = "16", label = "16 px" }, { id = "24", label = "24 px" } },
			},
			ui.column { gap = "small", table.unpack(rows) },
		}
	end,
}
