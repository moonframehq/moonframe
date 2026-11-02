-- Tally's shell: the sidebar on the window's own colour, and beside it the
-- top bar over one white card. The card holds the page the sidebar picked
-- (it publishes `tally/page`): the board with, on the right, the agent's
-- panel (which folds away), or the meetings.

local MENUS: { { title: string, items: { any } } } = {
	{ title = "Deal", items = {
		{ label = "New Deal", msg = "new-deal", to = "board", shortcut = "secondary-n" },
		{ label = "Search…", msg = "focus", to = "topbar", shortcut = "secondary-k" },
	} },
	{ title = "Meeting", items = {
		{ label = "Schedule Meeting", msg = "schedule", to = "meetings", shortcut = "secondary-e" },
		{ label = "Reschedule", msg = "reschedule", to = "meetings" },
		{ label = "Copy Meeting Link", msg = "join", to = "meetings" },
		"-",
		{ label = "Today", msg = "today", to = "meetings", shortcut = "secondary-t" },
		{ label = "Previous", msg = "previous", to = "meetings", shortcut = "secondary-[" },
		{ label = "Next", msg = "next", to = "meetings", shortcut = "secondary-]" },
	} },
	{ title = "View", items = {
		{ label = "Pipeline", msg = { select = "pipeline" }, to = "sidebar", shortcut = "secondary-shift-p" },
		{ label = "Meetings", msg = { select = "meetings" }, to = "sidebar", shortcut = "secondary-shift-m" },
		"-",
		{ label = "Board", msg = { view = "board" }, to = "board", shortcut = "secondary-1" },
		{ label = "List", msg = { view = "list" }, to = "board", shortcut = "secondary-2" },
		"-",
		{ label = "Day", msg = { view = "day" }, to = "meetings", shortcut = "secondary-3" },
		{ label = "Week", msg = { view = "week" }, to = "meetings", shortcut = "secondary-4" },
		{ label = "Month", msg = { view = "month" }, to = "meetings", shortcut = "secondary-5" },
		"-",
		{ label = "Show or Hide the Sidebar", msg = "toggle-left", shortcut = "secondary-s" },
		{ label = "Show or Hide the Agent", msg = "toggle-agent", shortcut = "secondary-j" },
		{ label = "New Chat", msg = "reset", to = "agent", shortcut = "secondary-shift-n" },
	} },
	{ title = "Window", items = {
		{ label = "Minimize", command = "minimize", shortcut = "secondary-m" },
		{ label = "Zoom", command = "maximize" },
	} },
}

return mod {
	window = { titlebar = "hidden", traffic_lights = { x = 18, y = 22 }, size = { width = 1760, height = 1080 } },

	about = {
		version = "0.1",
		description = "Deals on a board by stage, the quarter's figures, and a demo agent that reads them.",
		website = "https://github.com/moonframehq/moonframe",
		copyright = "MIT licensed",
	},

	menus = MENUS,

	subscribe = { "tally/page" },

	init = function(previous)
		local state = previous or { left = true, agent = true }
		state.page = state.page or "pipeline"
		return state
	end,

	update = function(state, msg)
		if type(msg) == "table" and msg.topic == "tally/page" then
			state.page = msg.value or "pipeline"
		elseif msg == "toggle-left" then
			state.left = not state.left
		elseif msg == "toggle-agent" then
			state.agent = not state.agent
		end
	end,

	view = function(state)
		local body = ui.hsplit {
			id = "body",
			divider = false,
			ui.pane { size = 224, min = 190, max = 360, fixed = true, visible = state.left, class = "frame", ui.region "left" },
			ui.pane {
				class = "frame",
				ui.column {
					grow = true,
					gap = "none",
					-- With the sidebar hidden, the bar starts past the traffic lights.
					ui.row { height = 52, shrink = false, gap = "none", style = { padding = { 0, 0, 0, if state.left then 0 else 72 } }, ui.region { name = "top", direction = "row" } },
					ui.column {
						class = "sheet",
						grow = true,
						state.page == "meetings" and ui.region "meetings",
						state.page ~= "meetings" and ui.hsplit {
							id = "work",
							ui.pane { min = 480, ui.region "main" },
							ui.pane { size = 350, min = 280, max = 560, fixed = true, visible = state.agent, ui.region "right" },
						},
					},
				},
			},
		}
		-- Send feedback (the built-in mods/feedback): ask an agent to change this app.
		return ui.column { grow = true, gap = "none", body, ui.row { class = "feedback-corner", float = { top = 12, right = 16 }, ui.mount "feedback" } }
	end,
}
