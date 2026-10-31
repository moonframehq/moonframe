-- Unread counts: + and − change the count (past 99 it shows 99+), the
-- segmented control sets the tone; the bubble floats on the bell's corner.
local count_bubble = use "count-bubble"
local stepper = use "stepper"
local segmented = use "segmented"

return mod {
	init = function(previous)
		return previous or { count = 7, tone = "danger" }
	end,

	update = function(state, msg)
		if type(msg) ~= "table" then
			return
		end
		if msg.step then
			state.count = math.max(0, state.count + msg.step)
		elseif msg.big then
			state.count += msg.big
		elseif msg.select then
			state.tone = msg.select
		end
	end,

	view = function(state)
		return ui.column {
			gap = "large",
			ui.row {
				gap = "normal",
				align = "center",
				stepper { label = "Count", value = state.count, min = 0, width = 52 },
				ui.button { "+50", small = true, variant = "ghost", msg = { big = 50 } },
			},
			segmented {
				selected = state.tone,
				small = true,
				options = {
					{ id = "accent", label = "Accent" },
					{ id = "danger", label = "Danger" },
					{ id = "warning", label = "Warning" },
					{ id = "success", label = "Success" },
					{ id = "muted", label = "Muted" },
				},
			},
			ui.row {
				gap = "large",
				align = "center",
				ui.row {
					padding = "small",
					ui.icon { name = "bell", size = "large" },
					count_bubble { count = state.count, tone = state.tone, float = { top = -4, right = -4 } },
				},
				ui.row {
					gap = "small",
					align = "center",
					ui.text "Pull requests",
					count_bubble { count = state.count, tone = state.tone, zero = true },
				},
			},
		}
	end,
}
