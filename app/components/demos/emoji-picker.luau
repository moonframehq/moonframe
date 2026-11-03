-- React to a message: the button opens the picker in a popover; search by
-- name, pick one and it joins the reactions, and moves to the front of
-- Frequently used. Escape or a click outside closes it.
local emoji_picker = use "emoji-picker"

return mod {
	init = function(previous)
		return previous or { open = true, query = "", picked = {}, recent = { "👍", "🎉", "❤️", "🚀", "👀" } }
	end,

	update = function(state, msg)
		if msg == "open" then
			state.open = not state.open
			state.query = ""
		elseif msg == "close" then
			state.open = false
		elseif type(msg) == "table" and msg.value ~= nil then
			state.query = msg.value
		elseif type(msg) == "table" and msg.pick then
			table.insert(state.picked, msg.pick)
			local at = table.find(state.recent, msg.pick)
			if at then
				table.remove(state.recent, at)
			end
			table.insert(state.recent, 1, msg.pick)
			state.open = false
		end
	end,

	view = function(state)
		return ui.column {
			gap = "normal",
			ui.row {
				gap = "normal",
				align = "center",
				ui.popover {
					class = "emoji-picker-popover",
					open = state.open,
					dismiss = "close",
					side = "bottom",
					align = "start",
					gap = 6,
					content = emoji_picker {
						recent = state.recent,
						query = state.query,
						search = { search = true },
						cancel = "close",
						focus = true,
					},
					ui.button { "Add reaction", icon = "plus", small = true, variant = "outline", msg = "open" },
				},
				ui.text { if #state.picked > 0 then table.concat(state.picked, " ") else "No reactions yet", tone = "muted" },
			},
		}
	end,
}
