-- Ages: the buttons move time forward (or back to now); each line shows the
-- same age in words and compact.
local relative_time = use "relative-time"
local switch = use "switch"

local EVENTS = {
	{ label = "Checkpoint saved", seconds = 20 },
	{ label = "Agent finished", seconds = 25 * 60 },
	{ label = "PR #850 updated", seconds = 3 * 86400 + 7200 },
	{ label = "Worktree created", seconds = 40 * 86400 },
}

return mod {
	init = function(previous)
		return previous or { offset = 0, compact = false }
	end,

	update = function(state, msg)
		if msg == "toggle-compact" then
			state.compact = not state.compact
		elseif msg == "now" then
			state.offset = 0
		elseif type(msg) == "table" and msg.wait then
			state.offset += msg.wait
		end
	end,

	view = function(state)
		local rows = {}
		for index, event in EVENTS do
			rows[index] = ui.row {
				gap = "normal",
				align = "center",
				ui.text { event.label },
				ui.spacer(),
				relative_time { seconds = event.seconds + state.offset, compact = state.compact, prefix = if state.compact then nil else "updated" },
			}
		end
		return ui.column {
			gap = "normal",
			width = 340,
			ui.row {
				gap = "small",
				ui.button { "+1 hour", small = true, variant = "outline", msg = { wait = 3600 } },
				ui.button { "+1 day", small = true, variant = "outline", msg = { wait = 86400 } },
				ui.button { "+1 week", small = true, variant = "outline", msg = { wait = 604800 } },
				ui.button { "Now", small = true, variant = "ghost", msg = "now" },
			},
			ui.row { switch { on = state.compact, label = "Compact", small = true, msg = "toggle-compact" } },
			ui.column { gap = "small", table.unpack(rows) },
		}
	end,
}
