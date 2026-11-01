-- Inbox / All / Last 7 Days: click a circle and it widens into the pill
-- while the old pill shrinks back; the list under it swaps with the view.
local pill_tabs = use "pill-tabs"
local swap_view = use "swap-view"

-- Lists that mix optional fields; typed loosely for the checker.
local function loose(list: { any }): { any }
	return list
end

local VIEWS = {
	inbox = { title = "Inbox", icon = "inbox", sheets = 1 },
	all = { title = "All", icon = "copy", sheets = 3 },
	recent = { title = "Last 7 Days", icon = "clock", sheets = 2 },
}

return mod {
	init = function(previous)
		return previous or { view = "inbox" }
	end,

	update = function(state, msg)
		if typeof(msg) == "table" and msg.select then
			state.view = msg.select
		end
	end,

	view = function(state)
		local view = VIEWS[state.view]
		return ui.column {
			gap = "normal",
			pill_tabs {
				selected = state.view,
				items = loose {
					{ id = "inbox", icon = "inbox", label = "Inbox", count = 1, dot = true },
					{ id = "all", icon = "copy", label = "All", count = 3 },
					{ id = "recent", icon = "clock", label = "Last 7 Days", count = 2 },
				},
			},
			swap_view {
				key = state.view,
				width = 240,
				ui.row {
					gap = "small",
					align = "center",
					ui.icon { name = view.icon :: any, tone = "muted" },
					ui.text { view.title, weight = "bold" },
				},
				ui.text { if view.sheets == 1 then "1 sheet" else `{view.sheets} sheets`, tone = "muted", size = "small" },
			},
		}
	end,
}
