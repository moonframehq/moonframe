-- Daybook's shell: a bar across the top (its mod, `topbar`), the sidebar on
-- the window's own colour, and two cards, the day and the inbox. The look
-- (classic: a dark frame around light cards; sepia; light) comes from the
-- planner provider, which keeps it; dark mode darkens all of them.

local MENUS: { { title: string, items: { any } } } = {
	{ title = "File", items = {
		{ label = "New Task", msg = "focus", to = "topbar", shortcut = "secondary-n" },
		{ label = "Search…", msg = "focus", to = "topbar", shortcut = "secondary-k" },
	} },
	{ title = "Go", items = {
		{ label = "Today", msg = { view = "today" }, to = "sidebar", shortcut = "secondary-1" },
		{ label = "Upcoming", msg = { view = "upcoming" }, to = "sidebar", shortcut = "secondary-2" },
		{ label = "Calendar", msg = { view = "calendar" }, to = "sidebar", shortcut = "secondary-3" },
		{ label = "Notes", msg = { view = "notes" }, to = "sidebar", shortcut = "secondary-4" },
		{ label = "Tasks", msg = { view = "tasks" }, to = "sidebar", shortcut = "secondary-5" },
		{ label = "Events", msg = { view = "events" }, to = "sidebar", shortcut = "secondary-6" },
		"-",
		{ label = "Previous Day", msg = { shift = -1 }, to = "provider:planner", shortcut = "secondary-[" },
		{ label = "Next Day", msg = { shift = 1 }, to = "provider:planner", shortcut = "secondary-]" },
		{ label = "Back to Today", msg = { select = "today" }, to = "provider:planner", shortcut = "secondary-t" },
	} },
	{ title = "View", items = {
		{ label = "Show or Hide the Sidebar", msg = "toggle-left", shortcut = "secondary-s" },
		{ label = "Show or Hide the Inbox", msg = "toggle-right", shortcut = "secondary-i" },
		"-",
		{ label = "Classic Look", msg = { look = "classic" }, to = "provider:planner" },
		{ label = "Sepia Look", msg = { look = "sepia" }, to = "provider:planner" },
		{ label = "Light Look", msg = { look = "light" }, to = "provider:planner" },
	} },
	{ title = "Window", items = {
		{ label = "Minimize", command = "minimize", shortcut = "secondary-m" },
		{ label = "Zoom", command = "maximize" },
	} },
}

return mod {
	window = { titlebar = "hidden", traffic_lights = { x = 20, y = 24 } },

	about = {
		version = "0.1",
		description = "Today's tasks and events, an inbox, a month at a glance, and notes.",
		website = "https://github.com/penso/moonframe",
		copyright = "MIT licensed",
	},

	menus = MENUS,

	init = function(previous)
		return previous or { left = true, right = true }
	end,

	update = function(state, msg)
		if msg == "toggle-left" then
			state.left = not state.left
		elseif msg == "toggle-right" then
			state.right = not state.right
		end
	end,

	view = function(state, app)
		local look = (app.planner and app.planner.look) or "classic"
		local chrome = `chrome chrome-{look}`
		local card = `card card-{look}`
		return ui.column {
			grow = true,
			gap = "none",
			class = chrome,
			-- The top bar's height, so its region does not stretch.
			ui.row { height = 60, shrink = false, gap = "none", ui.region { name = "top", direction = "row" } },
			ui.hsplit {
				id = "body",
				divider = false,
				ui.pane { size = 268, min = 200, max = 400, fixed = true, visible = state.left, class = chrome, ui.region "left" },
				ui.pane { min = 420, class = `{chrome} card-pane`, ui.column { class = card, grow = true, ui.region "main" } },
				ui.pane {
					size = 390,
					min = 300,
					max = 560,
					fixed = true,
					visible = state.right,
					class = `{chrome} card-pane card-pane-last`,
					ui.column { class = card, grow = true, ui.region "right" },
				},
			},
		}
	end,
}
