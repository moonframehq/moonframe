-- The bar along the window's foot, and the command palette it opens: every
-- session and action of the app, from the keyboard. The top bar's menu
-- opens it too. The bar's right side says which agent the chat uses and how
-- many work.

local status_bar = use "status-bar"
local command_palette = use "command-palette"

type State = { open: boolean, list: { any } }

local ACTIONS: { any } = {
	{ id = "new", label = "New session", icon = "square-pen", group = "Sessions", shortcut = "cmd-n",
		send = { to = "sidebar", msg = "new-session" } },
	{ id = "home", label = "Home", icon = "layout-grid", group = "Sessions",
		send = { to = "sidebar", msg = "home" } },
	{ id = "files", label = "Files Changed", icon = "list-tree", group = "Workbench", shortcut = "cmd-1",
		send = { to = "workbench", msg = { tab = "files" } } },
	{ id = "terminal", label = "Terminal", icon = "terminal", group = "Workbench", shortcut = "cmd-2",
		send = { to = "workbench", msg = { tab = "terminal" } } },
	{ id = "left", label = "Show or hide the sidebar", icon = "panel-left", group = "View", shortcut = "cmd-s",
		send = { to = "shell", msg = "toggle-left" } },
	{ id = "right", label = "Show or hide the workbench", icon = "panel-right", group = "View", shortcut = "cmd-b",
		send = { to = "shell", msg = "toggle-right" } },
}

-- The palette's rows: the actions, then a row per session.
local function commands(list: { any }): { any }
	local rows: { any } = {}
	for _, action in ACTIONS do
		table.insert(rows, { id = action.id, label = action.label, icon = action.icon, group = action.group, shortcut = action.shortcut })
	end
	for _, session in list do
		table.insert(rows, { id = `session:{session.id}`, label = session.title, detail = session.name, icon = "message-square", group = "Open session" })
	end
	return rows
end

-- What the bar says of the agents: the default one, and how many work.
local function agents_note(acp: any): string
	if acp == nil then
		return "Starting…"
	end
	local title, found = acp.agent, false
	for _, agent in acp.agents or {} do
		found = found or agent.installed or agent.fetched
		if agent.name == acp.agent then
			title = agent.title
		end
	end
	if not found then
		return "No coding agent found"
	end
	if (acp.running or 0) > 0 then
		return `{title} · {acp.running} working`
	end
	return title
end

return mod {
	subscribe = { "cairn/sessions" },

	keys = { ["secondary-shift-p"] = "open" } :: any,

	init = function(previous: State?): State
		return { open = false, list = (previous and previous.list) or {} }
	end,

	update = function(state: State, msg)
		if msg == "open" then
			state.open = not state.open
		elseif msg == "close" then
			state.open = false
		elseif type(msg) == "table" and msg.topic == "cairn/sessions" and msg.value then
			state.list = msg.value.list
		elseif type(msg) == "table" and msg.run then
			state.open = false
			local id = string.match(msg.run, "^session:(.+)$")
			if id then
				return state, { { to = "sidebar", msg = { select = id } } }
			end
			for _, action in ACTIONS do
				if action.id == msg.run then
					return state, { action.send }
				end
			end
		end
	end,

	view = function(state: State, app)
		local rows = commands(state.list)
		local shortcut = if app.window.platform == "macos" then "⌘⇧P" else "Ctrl+Shift+P"
		return ui.column { gap = "none", shrink = false,
			status_bar {
				left = { { icon = "command", text = "Commands", msg = "open", tooltip = `Open the command palette ({shortcut})` } },
				right = { { icon = "bot", text = agents_note(app.acp), tone = "muted" } },
			},
			command_palette { open = state.open, dismiss = "close", commands = rows, placeholder = "Jump to a session or run a command…" },
		}
	end,
}
