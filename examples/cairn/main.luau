-- cairn's shell: a thin bar across the top, the sessions on the window's
-- own colour, then two cards: the chat (or Home) and the workbench. The
-- sidebar says which page shows on `cairn/page`; the bar's pill reads the
-- greeter service on `cairn/greeter`, and opens its tab.

local title_bar = use "title-bar"
local icon_button = use "icon-button"

local MENUS: { { title: string, items: { any } } } = {
	{ title = "Session", items = {
		{ label = "New Session", msg = "new-session", to = "sidebar", shortcut = "secondary-n" },
		{ label = "Search Sessions", msg = "search", to = "sidebar", shortcut = "secondary-f" },
		{ label = "Home", msg = "home", to = "sidebar", shortcut = "secondary-shift-h" },
	} },
	{ title = "View", items = {
		{ label = "Files Changed", msg = { tab = "files" }, to = "workbench", shortcut = "secondary-1" },
		{ label = "Hello", msg = { tab = "hello" }, to = "workbench", shortcut = "secondary-2" },
		{ label = "Echo", msg = { tab = "echo" }, to = "workbench", shortcut = "secondary-3" },
		"-",
		{ label = "Show or Hide the Sidebar", msg = "toggle-left", shortcut = "secondary-s" },
		{ label = "Show or Hide the Workbench", msg = "toggle-right", shortcut = "secondary-b" },
		{ label = "Playground", msg = "open", to = "playground", shortcut = "secondary-shift-d" },
	} },
	{ title = "Window", items = {
		{ label = "Minimize", command = "minimize", shortcut = "secondary-m" },
		{ label = "Zoom", command = "maximize" },
	} },
}

type State = { left: boolean, right: boolean, page: string, generation: number }

return mod {
	window = { titlebar = "hidden", traffic_lights = { x = 16, y = 18 }, size = { width = 1387, height = 886 } },

	about = {
		version = "0.1",
		description = "A demo agent workspace: sample sessions, a scripted chat, and two mods talking through a service.",
		website = "https://github.com/moonframehq/moonframe",
		copyright = "MIT licensed",
	},

	menus = MENUS,

	subscribe = { "cairn/page", "cairn/greeter" },

	styles = {
		[".service-pill"] = { background = "#1c1c1c", radius = 999, padding = { 4, 12, 4, 10 }, gap = 6, color = "#d8d8d8" },
		[".service-pill:hover"] = { background = "#262626" },
	},

	-- A dark app: the first launch settles it, and the user may change it after.
	init = function(previous: State?)
		if previous then
			return previous
		end
		local state: State = { left = true, right = true, page = "chat", generation = 1 }
		return state, { { to = "app:appearance", msg = "dark" } }
	end,

	update = function(state: State, msg)
		if msg == "toggle-left" then
			state.left = not state.left
		elseif msg == "toggle-right" then
			state.right = not state.right
		elseif msg == "show-echo" then
			state.right = true
			return state, { { to = "workbench", msg = { tab = "echo" } } }
		elseif type(msg) == "table" and msg.show_right then
			state.right = true
		elseif type(msg) == "table" and msg.topic == "cairn/page" then
			state.page = msg.value or "chat"
		elseif type(msg) == "table" and msg.topic == "cairn/greeter" and msg.value then
			state.generation = msg.value.generation
		end
	end,

	view = function(state: State, app)
		local bar = ui.column { height = 48, shrink = false, gap = "none", justify = "center",
			title_bar {
				window = app.window,
				leading = {
					icon_button { icon = "menu", tooltip = "Playground", msg = "open", to = "playground" },
					icon_button { icon = "panel-left", tooltip = "Toggle sidebar", msg = "toggle-left" },
				},
				actions = {
					ui.row { class = "service-pill", align = "center", msg = "show-echo", tooltip = "Open Echo",
						ui.icon { "wifi", tone = "muted" },
						ui.text { `Echo · gen {state.generation}`, weight = "medium", size = "small" },
					},
				},
			},
		}
		local body = ui.hsplit {
			id = "body",
			divider = false,
			ui.pane { size = 286, min = 200, max = 420, fixed = true, visible = state.left, class = "side", ui.mount "sidebar" },
			ui.pane { min = 420, class = "middle",
				ui.column { class = "card", grow = true, gap = "none",
					if state.page == "home" then ui.mount "home" else ui.mount "chat",
				},
			},
			ui.pane { size = 508, min = 320, max = 760, fixed = true, visible = state.right, class = "right",
				ui.column { class = "card", grow = true, gap = "none", ui.mount "workbench" },
			},
		}
		return ui.column { grow = true, gap = "none", bar, body, ui.mount "playground" }
	end,
}
