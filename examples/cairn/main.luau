-- cairn's shell: a thin bar across the top, the sessions on the window's
-- own colour, then two cards: the chat (or Home) and the workbench, with
-- the status bar and command palette along the foot. The sidebar says
-- which page shows on `cairn/page`.

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
		{ label = "Terminal", msg = { tab = "terminal" }, to = "workbench", shortcut = "secondary-2" },
		"-",
		{ label = "Show or Hide the Sidebar", msg = "toggle-left", shortcut = "secondary-s" },
		{ label = "Show or Hide the Workbench", msg = "toggle-right", shortcut = "secondary-b" },
		{ label = "Command Palette", msg = "open", to = "palette", shortcut = "secondary-shift-p" },
	} },
	{ title = "Window", items = {
		{ label = "Minimize", command = "minimize", shortcut = "secondary-m" },
		{ label = "Zoom", command = "maximize" },
	} },
}

type State = { left: boolean, right: boolean, page: string }

return mod {
	window = { titlebar = "hidden", traffic_lights = { x = 16, y = 18 }, size = { width = 1387, height = 886 } },

	about = {
		version = "0.2",
		description = "An agent workspace: sessions with a coding agent in your folders, their changes, and a terminal.",
		website = "https://github.com/moonframehq/moonframe",
		copyright = "MIT licensed",
	},

	menus = MENUS,

	subscribe = { "cairn/page" },

	-- A dark app: the first launch settles it, and the user may change it after.
	init = function(previous: any)
		if previous then
			previous.generation = nil
			return previous
		end
		local state: State = { left = true, right = true, page = "home" }
		return state, { { to = "app:appearance", msg = "dark" } }
	end,

	update = function(state: State, msg)
		if msg == "toggle-left" then
			state.left = not state.left
		elseif msg == "toggle-right" then
			state.right = not state.right
		elseif type(msg) == "table" and msg.show_right then
			state.right = true
		elseif type(msg) == "table" and msg.show_left then
			state.left = true
		elseif type(msg) == "table" and msg.topic == "cairn/page" then
			state.page = msg.value or "chat"
		end
	end,

	view = function(state: State, app)
		local bar = ui.column { height = 48, shrink = false, gap = "none", justify = "center",
			title_bar {
				window = app.window,
				leading = {
					icon_button { icon = "command", tooltip = "Command palette", msg = "open", to = "palette" },
					icon_button { icon = "panel-left", tooltip = "Toggle sidebar", msg = "toggle-left" },
				},
				actions = {
					icon_button { icon = "panel-right", tooltip = "Toggle workbench", msg = "toggle-right" },
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
		return ui.column { grow = true, gap = "none", bar, body, ui.mount "palette" }
	end,
}
