-- A writing app: hide the sidebar with its header button and the list
-- slides over; hide the list too and the page has the window. Switch
-- Inbox / All / Last 7 Days and the list swaps with the view.
local side_panels = use "side-panels"
local pill_tabs = use "pill-tabs"
local swap_view = use "swap-view"
local icon_button = use "icon-button"

-- Lists that mix optional fields; typed loosely for the checker.
local function loose(list: { any }): { any }
	return list
end

local VIEWS = {
	inbox = { title = "Inbox", icon = "inbox" },
	all = { title = "All", icon = "copy" },
	recent = { title = "Last 7 Days", icon = "clock" },
}

return mod {
	styles = {
		[".demo-sheet"] = { background = "muted", radius = 8, padding = 10, gap = 2 },
		[".demo-new"] = { border = "all", border_color = "border", radius = 8, padding = 6 },
	},

	init = function(previous)
		return previous or { left = true, middle = true, view = "inbox" }
	end,

	update = function(state, msg)
		if msg == "left" or msg == "middle" then
			state[msg] = not state[msg]
		elseif typeof(msg) == "table" and msg.select then
			state.view = msg.select
		end
	end,

	view = function(state)
		local view = VIEWS[state.view]
		local sidebar = ui.column {
			padding = "normal",
			gap = "normal",
			ui.row {
				align = "center",
				ui.text { "Neverland.", weight = "bold", size = "large" },
				ui.spacer(),
				icon_button { icon = "panel-left-close", tooltip = "Hide sidebar", msg = "left", size = "small" },
			},
			pill_tabs {
				selected = state.view,
				width = 104,
				size = 30,
				items = loose {
					{ id = "inbox", icon = "inbox", label = "Inbox", count = 1, dot = true },
					{ id = "all", icon = "copy", label = "All", count = 1 },
					{ id = "recent", icon = "clock", label = "Last 7 Days", count = 1, label_width = 70 },
				},
			},
			ui.text { "Groups", tone = "muted", size = "small" },
			ui.item { "Simple Group" },
		}
		local list = ui.column {
			padding = "normal",
			gap = "normal",
			ui.row {
				align = "center",
				justify = "end",
				not state.left and icon_button { icon = "panel-left-open", tooltip = "Show sidebar", msg = "left", size = "small" },
				icon_button { icon = "panel-left-close", tooltip = "Hide list", msg = "middle", size = "small" },
			},
			swap_view {
				key = state.view,
				width = 196,
				ui.row {
					gap = "small",
					align = "center",
					ui.icon { name = view.icon :: any, tone = "muted" },
					ui.column {
						gap = "none",
						ui.text { view.title, weight = "bold" },
						ui.text { "1 sheet · 150 words", tone = "muted", size = "small" },
					},
				},
				ui.column {
					class = "demo-sheet",
					ui.text { "Hello world!", weight = "medium" },
					ui.text { "Lorem sint deserunt adipisicing laborum tempor nulla ut in aute.", tone = "muted", size = "small", lines = 2 },
				},
				ui.row { class = "demo-new", justify = "center", ui.text { "+ New sheet", tone = "muted", size = "small" } },
			},
		}
		local main = ui.column {
			padding = "large",
			gap = "normal",
			grow = true,
			ui.row {
				align = "center",
				not state.middle and icon_button { icon = "panel-left-open", tooltip = "Show list", msg = "middle", size = "small" },
				ui.text { "Hello world!", tone = "muted", size = "small" },
			},
			ui.text { "# Hello world!", size = "title", weight = "bold" },
			ui.text { "Lorem sint deserunt adipisicing laborum tempor nulla ut in aute anim labore. Duis enim consectetur ut laborum." },
		}
		return side_panels {
			id = "demo-side-panels",
			height = 340,
			left = state.left,
			middle = state.middle,
			sidebar_size = 220,
			list_size = 220,
			sidebar = sidebar,
			list = list,
			main = main,
		}
	end,
}
