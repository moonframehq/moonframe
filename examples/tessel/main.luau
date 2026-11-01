-- Tessel's shell: the sidebar on its grey beside the page, or, on an
-- agent's own page (or with the sidebar folded), a narrow rail of icons.
-- Where you are travels on the `tessel/nav` topic: `{ detail = "<agent id>" }`
-- on an agent's page, `detail = ""` on the Agents page. The workflow modal is
-- mounted over everything.

local MENUS: { { title: string, items: { any } } } = {
	{ title = "Agents", items = {
		{ label = "Create an Agent…", msg = "create", to = "agents", shortcut = "secondary-n" },
		{ label = "Find…", msg = "find", to = "agents", shortcut = "secondary-f" },
		"-",
		{ label = "My Agents", msg = { tab = "mine" }, to = "agents", shortcut = "secondary-1" },
		{ label = "Agent Hub", msg = { tab = "hub" }, to = "agents", shortcut = "secondary-2" },
	} },
	{ title = "View", items = {
		{ label = "As Cards", msg = { layout = "grid" }, to = "agents" },
		{ label = "As a Table", msg = { layout = "list" }, to = "agents" },
		"-",
		{ label = "Show or Hide the Sidebar", msg = "toggle-left", shortcut = "secondary-s" },
	} },
	{ title = "Window", items = {
		{ label = "Minimize", command = "minimize", shortcut = "secondary-m" },
		{ label = "Zoom", command = "maximize" },
	} },
}

type State = { left: boolean, detail: string }

return mod {
	window = { titlebar = "hidden", traffic_lights = { x = 18, y = 22 }, size = { width = 1440, height = 1000 } },

	about = {
		version = "0.1",
		description = "Your agents, what they ran, and the workflows behind them. Sample data.",
		website = "https://github.com/penso/moonframe",
		copyright = "MIT licensed",
	},

	menus = MENUS,

	subscribe = { "tessel/nav" },

	init = function(previous: State?): State
		return { left = if previous then previous.left else true, detail = "" }
	end,

	update = function(state: State, msg)
		if msg == "toggle-left" then
			state.left = not state.left
		elseif msg == "show-left" then
			-- From the rail: back to the Agents page with the sidebar out.
			state.left = true
			return state, { { to = "topic:tessel/nav", msg = { detail = "" } } }
		elseif type(msg) == "table" and msg.topic == "tessel/nav" and type(msg.value) == "table" then
			state.detail = msg.value.detail or ""
		end
	end,

	view = function(state: State)
		local full = state.left and state.detail == ""
		return ui.hsplit {
			id = "body",
			divider = false,
			ui.pane { size = 56, fixed = true, visible = not full, class = "side-frame", ui.region "rail" },
			ui.pane { size = 260, min = 220, max = 360, fixed = true, visible = full, class = "side-frame", ui.region "left" },
			ui.pane {
				min = 720,
				class = "main-frame",
				ui.mount(if state.detail ~= "" then "agent" else "agents"),
				ui.mount "workflow",
			},
		}
	end,
}
