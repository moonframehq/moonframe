-- Home: what the sessions add up to, and the ones with something new.

local stat_tile = use "stat-tile"
local empty_state = use "empty-state"

local function tokens(value: number): string
	return if value < 1000 then tostring(value) else string.format("%.1fK", value / 1000)
end

return mod {
	subscribe = { "opencode/sessions" },

	styles = {
		[".home"] = { padding = { 28, 28 }, gap = 18 },
		[".home-title"] = { size = "title", weight = "semibold" },
		[".home-row"] = { padding = { 9, 12 }, gap = 10, radius = 8, background = "raised" },
		[".home-row:hover"] = { background = "#2a2a2a" },
		[".unread"] = { background = "accent", radius = 999 },
	},

	init = function(previous)
		return previous or { list = {} }
	end,

	update = function(state, msg)
		if type(msg) == "table" and msg.topic == "opencode/sessions" and msg.value then
			state.list = msg.value.list
		end
	end,

	view = function(state)
		local total_tokens, total_cost = 0, 0
		local rows = {}
		for _, session: any in state.list do
			total_tokens += session.tokens
			total_cost += session.cost
			if session.unread then
				table.insert(rows, ui.row { class = "home-row", align = "center", msg = { select = session.id }, to = "sidebar",
					ui.row { class = "unread", width = 7, height = 7, shrink = false },
					ui.text { session.title, truncate = true },
					ui.spacer(),
					ui.text { session.project, tone = "muted", size = "small" },
				})
			end
		end
		return ui.column { class = "home", grow = true, scroll = true,
			ui.text { "Home", class = "home-title" },
			ui.row { gap = "normal",
				ui.column { grow = true, stat_tile { label = "Sessions", value = tostring(#state.list), icon = "layout-grid" } },
				ui.column { grow = true, stat_tile { label = "Tokens", value = tokens(total_tokens), icon = "hash" } },
				ui.column { grow = true, stat_tile { label = "Spent", value = string.format("$%.2f", total_cost), icon = "activity" } },
			},
			ui.text { "Unread", tone = "muted" },
			#rows == 0 and empty_state { icon = "inbox", title = "All read", message = "New answers show up here." },
			table.unpack(rows),
		}
	end,
}
