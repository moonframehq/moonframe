-- Sorrel's shell: the sidebar on the window's own grey, and beside it one
-- white card: the project's bar across the top, then the main thread with
-- the agent's plan on the left and the sheet it fills on the right.

local icon_button = use "icon-button"

local MENUS: { { title: string, items: { any } } } = {
	{ title = "Research", items = {
		{ label = "Pause or Resume the Agent", msg = "toggle", to = "thread", shortcut = "secondary-." },
		{ label = "Run the Demo Again", msg = "restart", to = "thread", shortcut = "secondary-shift-r" },
		"-",
		{ label = "Search…", msg = "focus", to = "topbar", shortcut = "secondary-k" },
	} },
	{ title = "View", items = {
		{ label = "Show or Hide the Sidebar", msg = "toggle-left", shortcut = "secondary-s" },
		{ label = "Show or Hide the Thread", msg = "toggle-thread", shortcut = "secondary-j" },
	} },
	{ title = "Window", items = {
		{ label = "Minimize", command = "minimize", shortcut = "secondary-m" },
		{ label = "Zoom", command = "maximize" },
	} },
}

return mod {
	window = { titlebar = "hidden", traffic_lights = { x = 18, y = 18 }, size = { width = 1440, height = 920 } },

	about = {
		version = "0.1",
		description = "An agent's research workspace: its plan in a thread, and the sheet it fills cell by cell (a scripted demo).",
		website = "https://github.com/penso/moonframe",
		copyright = "MIT licensed",
	},

	menus = MENUS,

	init = function(previous)
		return previous or { left = true, thread = true }
	end,

	update = function(state, msg)
		if msg == "toggle-left" then
			state.left = not state.left
		elseif msg == "toggle-thread" then
			state.thread = not state.thread
		end
	end,

	view = function(state, app)
		-- With the sidebar hidden, the top bar leads with a button to bring it
		-- back, set clear of the traffic lights on macOS.
		local window = app.window
		local lights = window.platform == "macos" and not window.fullscreen
		local show_left = if state.left
			then nil
			else ui.row {
				width = if lights then 104 else 48,
				shrink = false,
				align = "center",
				justify = "end",
				icon_button { icon = "panel-left", tooltip = "Show the sidebar", msg = "toggle-left" },
			}
		local body = ui.hsplit {
			id = "body",
			divider = false,
			ui.pane { size = 216, min = 190, max = 340, fixed = true, visible = state.left, class = "frame", ui.region "left" },
			ui.pane {
				class = "frame",
				ui.column {
					class = "card",
					grow = true,
					gap = "none",
					ui.row { height = 54, shrink = false, gap = "none", class = "topline", show_left, ui.region { name = "top", direction = "row" } },
					ui.hsplit {
						id = "work",
						ui.pane { size = 420, min = 320, max = 620, fixed = true, visible = state.thread, ui.region "thread" },
						ui.pane { min = 520, class = "bay", ui.region "main" },
					},
				},
			},
		}
		-- Send feedback (the built-in mods/feedback): ask an agent to change this app.
		return ui.column { grow = true, gap = "none", body, ui.row { class = "feedback-corner", float = { at = "bottom-right", inset = 20 }, ui.mount "feedback" } }
	end,
}
