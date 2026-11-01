-- Duly's shell: the sidebar on the left, and the page it picks on the
-- window's grey beside it. The sidebar owns which page shows and tells the
-- shell; every page is a mod mounted here by name.

local MENUS: { { title: string, items: { any } } } = {
	{ title = "Go", items = {
		{ label = "Today", msg = { page = "today" }, to = "sidebar", shortcut = "secondary-1" },
		{ label = "Contracts", msg = { page = "contracts" }, to = "sidebar", shortcut = "secondary-2" },
		{ label = "Payments", msg = { page = "payments" }, to = "sidebar", shortcut = "secondary-3" },
		{ label = "Reports", msg = { page = "reports" }, to = "sidebar", shortcut = "secondary-4" },
	} },
	{ title = "View", items = {
		{ label = "Show or Hide the Sidebar", msg = "toggle-left", shortcut = "secondary-s" },
	} },
	{ title = "Window", items = {
		{ label = "Minimize", command = "minimize", shortcut = "secondary-m" },
		{ label = "Zoom", command = "maximize" },
	} },
}

-- Pages with a mod of their own; the rest share `elsewhere`.
local PAGES = { today = true, contracts = true, payments = true, reports = true }

type State = { left: boolean, page: string, label: string }

return mod {
	window = { titlebar = "hidden", traffic_lights = { x = 18, y = 18 }, size = { width = 1440, height = 1120 } },

	about = {
		version = "0.1",
		description = "Compliance notices for a public buyer: what is due, exceptions, contracts, payments and reports. Shows sample data.",
		website = "https://github.com/penso/moonframe",
		copyright = "MIT licensed",
	},

	menus = MENUS,

	init = function(previous: State?): State
		return previous or { left = true, page = "today", label = "Today" }
	end,

	update = function(state: State, msg)
		if msg == "toggle-left" then
			state.left = not state.left
		elseif type(msg) == "table" and msg.page then
			state.page = msg.page
			state.label = msg.label or msg.page
		end
	end,

	view = function(state: State)
		local page = if PAGES[state.page]
			then ui.mount(state.page)
			else ui.mount { "elsewhere", id = state.page, props = { title = state.label } }
		return ui.hsplit {
			id = "body",
			divider = false,
			ui.pane { size = 238, min = 200, max = 340, fixed = true, visible = state.left, class = "side-pane", ui.region "left" },
			ui.pane { class = "frame", ui.column { class = "frame", grow = true, gap = "none", page } },
		}
	end,
}
