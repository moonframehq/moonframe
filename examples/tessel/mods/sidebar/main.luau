-- The sidebar: the brand beside the traffic lights with search and fold
-- buttons, New Chat, Automations (Agents, Intelligence), projects, the
-- latest agent chats with their pixel icons, the other chats, and who is
-- signed in at the foot. Only Agents opens a page in this example; the rest
-- select.

local pixel_icon = use "pixel-icon"

type State = { selected: string }

local LOGO = { "0000", "0111", "0001", "0001", "0001", "0001", "0000" }

local function find_agent(app, id: string): any?
	local data = app.tessel
	if data and data.agents then
		for _, agent in data.agents do
			if agent.id == id then
				return agent
			end
		end
	end
	return nil
end

return mod {
	region = "left",
	title = "Sidebar",

	styles = {
		[".side"] = { padding = { 0, 12, 0, 12 }, gap = 1 },
		[".side-top"] = { padding = { 0, 4, 0, 8 }, gap = 8 },
		[".side-tool"] = { color = "muted-foreground", radius = 6, padding = 4 },
		[".side-tool:hover"] = { background = "list-hover", color = "foreground" },
		[".side-new"] = { padding = { 8, 10 }, radius = 8, gap = 8, margin = { 4, 0, 6, 0 } },
		[".side-new:hover"] = { background = "list-hover" },
		[".side-new-icon"] = { color = "brand" },
		[".side-heading"] = { padding = { 12, 4, 4, 12 }, gap = 6 },
		[".side-heading-text"] = { size = "small", color = "muted-foreground", weight = "medium" },
		[".side-add"] = { color = "muted-foreground", radius = 5, padding = 2 },
		[".side-add:hover"] = { background = "list-hover", color = "foreground" },
		[".side-row"] = { padding = { 7, 10 }, radius = 8, gap = 10 },
		[".side-row:hover"] = { background = "list-hover" },
		[".side-row:selected"] = { background = "#0000000d" },
		[".side-chat"] = { padding = { 7, 10, 7, 12 }, radius = 8 },
		[".side-chat:hover"] = { background = "list-hover" },
		[".side-chat:selected"] = { background = "#0000000d" },
		[".side-beta"] = { size = "small", color = "#0e8fa3", background = "#d9f3f6", radius = 5, padding = { 0, 6 } },
		[".side-foot"] = { padding = { 10, 14, 14, 18 }, gap = 10 },
		[".side-foot:hover"] = { background = "list-hover" },
		[".side-plan"] = { size = "small", color = "brand" },
		[".side-muted"] = { color = "muted-foreground" },
	},

	init = function(previous: State?): State
		return { selected = if previous then previous.selected else "agents" }
	end,

	update = function(state: State, msg)
		if type(msg) == "table" and msg.select then
			state.selected = msg.select
			if msg.select == "agents" then
				return state, { { to = "agents", msg = { tab = "mine" } } }
			end
		end
	end,

	view = function(state: State, app)
		local window = app.window
		local data = app.tessel

		local function heading(title: string)
			return ui.row {
				class = "side-heading",
				align = "center",
				ui.text { title, class = "side-heading-text" },
				ui.spacer(),
				ui.row { class = "side-add", msg = { select = state.selected }, tooltip = `New in {title}`, ui.icon { name = "plus", size = "small" } },
			}
		end

		local function row(id: string, label: string, leading: any, trailing: any?)
			return ui.row {
				class = "side-row",
				align = "center",
				selected = state.selected == id,
				msg = { select = id },
				leading,
				ui.text { label, truncate = true },
				ui.spacer(),
				trailing,
			}
		end

		local rows: { any } = {
			ui.row {
				class = "side-top",
				drag = true,
				height = 56,
				shrink = false,
				align = "center",
				window.platform == "macos" and not window.fullscreen and ui.row { width = 66 },
				pixel_icon { pattern = LOGO, colors = { "#f2542d", "#ffffff" }, size = 26 },
				ui.spacer(),
				ui.row { class = "side-tool", msg = { select = "search" }, tooltip = "Search chats", ui.icon "search" },
				ui.row { class = "side-tool", msg = "toggle-left", to = "shell", tooltip = "Fold the sidebar", ui.icon "panel-left" },
			},
			ui.row {
				class = "side-new",
				align = "center",
				msg = { select = "new" },
				ui.icon { name = "plus", class = "side-new-icon" },
				ui.text { "New Chat", weight = "medium" },
			},
			ui.row { class = "side-heading", ui.text { "Automations", class = "side-heading-text" } },
			row("agents", "Agents", ui.icon "bot"),
			row("intelligence", "Intelligence", ui.icon "sparkles", ui.text { "Beta", class = "side-beta" }),
			heading "Projects",
		}
		if data and data.projects then
			for _, project in data.projects do
				table.insert(rows, row(project.id, project.label, ui.icon { name = project.icon :: any }))
			end
		end
		table.insert(rows, heading "Latest Agent Chats")
		if data and data.agent_chats then
			for _, chat in data.agent_chats do
				local agent = find_agent(app, chat.agent)
				table.insert(rows, row(
					chat.id,
					chat.label,
					agent and pixel_icon { pattern = agent.icon.pattern, colors = agent.icon.colors, size = 18 } or ui.icon "bot"
				))
			end
		end
		table.insert(rows, heading "Chats")
		if data and data.chats then
			for _, chat in data.chats do
				table.insert(rows, ui.row {
					class = "side-chat",
					selected = state.selected == chat.id,
					msg = { select = chat.id },
					ui.text { chat.label, truncate = true },
				})
			end
		end

		local user = data and data.user
		return ui.column {
			grow = true,
			gap = "none",
			ui.column {
				grow = true,
				scroll = true,
				gap = "none",
				ui.column { class = "side", shrink = false, table.unpack(rows) },
			},
			user and ui.row {
				class = "side-foot",
				align = "center",
				shrink = false,
				msg = { select = "account" },
				ui.row {
					width = 32,
					height = 32,
					align = "center",
					justify = "center",
					style = { background = user.color, radius = 999, color = "#ffffff" },
					ui.text { (string.match(user.name, "^" .. utf8.charpattern) or ""), weight = "bold" },
				},
				ui.column {
					gap = "none",
					grow = true,
					ui.text { user.name, weight = "medium", truncate = true },
					ui.text { user.plan, class = "side-plan" },
				},
				ui.icon { name = "chevrons-up-down", class = "side-muted" },
			},
		}
	end,
}
