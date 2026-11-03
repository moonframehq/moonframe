-- Tasks and their status: click a glyph to move the task along; the
-- control switches the size. In progress fills a quarter at a time.
local status_icon = use "status-icon"
local segmented = use "segmented"

local NEXT = { backlog = "todo", todo = "progress", progress = "review", review = "done", done = "cancelled", cancelled = "backlog" }

return mod {
	init = function(previous)
		return previous
			or {
				size = "14",
				tasks = {
					{ title = "Collect feedback from the beta", status = "backlog" },
					{ title = "Write the onboarding copy", status = "todo" },
					{ title = "Upload flow for attachments", status = "progress", progress = 0.25 },
					{ title = "Board view with drag and drop", status = "progress" },
					{ title = "Comment reactions", status = "review" },
					{ title = "Project cards", status = "done" },
					{ title = "Legacy importer", status = "cancelled" },
				},
			}
	end,

	update = function(state, msg)
		if type(msg) ~= "table" then
			return
		end
		if msg.select then
			state.size = msg.select
		elseif msg.advance then
			local task = state.tasks[msg.advance]
			task.status = NEXT[task.status]
			task.progress = nil
		end
	end,

	view = function(state)
		local size = tonumber(state.size)
		local rows = {}
		for index, task in state.tasks do
			rows[index] = ui.row {
				gap = "normal",
				align = "center",
				ui.row { msg = { advance = index }, status_icon { status = task.status, progress = task.progress, size = size } },
				ui.text { task.title, tone = if task.status == "cancelled" then "muted" else nil },
				ui.spacer(),
				status_icon { status = task.status, progress = task.progress, size = size, label = true },
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
