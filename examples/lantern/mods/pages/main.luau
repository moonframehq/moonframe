-- The pages: the workspace switcher under the traffic lights, ACTIVE (the
-- open project and those with an agent running), and PAGES, every project
-- under its project folder, with its changed files and live agents. A
-- night scene (string lights, stars, a jack-o'-lantern) closes the column.

local menu = use "menu"

type State = {
	folded: { [string]: boolean? },
	active_folded: boolean,
	pages_folded: boolean,
	picking: boolean,
}

-- Agents running in each project, by its path.
local function live(agents: any, path: string): number
	local count = 0
	for _, agent in (agents and agents.agents) or {} do
		local cwd = agent.cwd
		if cwd and (cwd == path or string.sub(cwd, 1, #path + 1) == path .. "/") then
			count += 1
		end
	end
	return count
end

local function pill(count: number)
	return ui.row {
		class = "pages-live",
		align = "center",
		shrink = false,
		tooltip = `{count} {if count == 1 then "agent" else "agents"} running`,
		ui.row { class = "pages-dot", width = 6, height = 6 },
		ui.text { tostring(count), class = "pages-live-count" },
	}
end

local function heading(title: string, count: number?, folded: boolean, msg: any)
	return ui.row {
		class = "pages-heading",
		align = "center",
		msg = msg,
		ui.icon { name = if folded then "chevron-right" else "chevron-down", class = "pages-chevron", size = "small" },
		ui.text { string.upper(title), class = "pages-heading-text" },
		count ~= nil and ui.text { tostring(count), class = "pages-count" },
	}
end

return mod {
	region = "left",
	title = "Pages",

	styles = {
		[".pages-head"] = { padding = { 0, 8, 0, 0 }, gap = 8 },
		[".pages-tile"] = { background = "pumpkin", radius = 6, color = "#2a0e02" },
		[".pages-switch"] = { padding = { 4, 6 }, radius = 6, gap = 8 },
		[".pages-switch:hover"] = { background = "list-hover" },
		[".pages-name"] = { weight = "medium", size = "large" },
		[".pages-body"] = { padding = { 4, 8, 8, 8 }, gap = 2 },
		[".pages-heading"] = { padding = { 8, 4, 6, 4 }, gap = 6, color = "muted-foreground" },
		[".pages-heading-text"] = { weight = "bold", size = "small" },
		[".pages-chevron"] = { color = "muted-foreground" },
		[".pages-count"] = { background = "secondary", radius = 8, padding = { 0, 6 }, size = "small", color = "muted-foreground" },
		[".pages-row"] = { padding = { 0, 8 }, radius = 6, gap = 8 },
		[".pages-row:hover"] = { background = "list-hover" },
		[".pages-row:selected"] = { background = "row-selected" },
		[".pages-row-icon"] = { color = "muted-foreground" },
		[".pages-row-detail"] = { color = "muted-foreground", size = "small" },
		[".pages-changed"] = { color = "muted-foreground", size = "small" },
		[".pages-live"] = { background = "secondary", radius = 9, padding = { 1, 7 }, gap = 6 },
		[".pages-dot"] = { background = "success", radius = 3 },
		[".pages-live-count"] = { size = "small", color = "muted-foreground" },
		[".pages-art"] = { padding = 0 },
		[".pages-menu"] = { padding = 0 },
	},

	init = function(previous: State?): State
		local state: State = previous or { folded = {}, active_folded = false, pages_folded = false, picking = false }
		state.picking = false
		return state
	end,

	update = function(state: State, msg)
		if msg == "pick" then
			state.picking = not state.picking
		elseif msg == "dismiss" then
			state.picking = false
		elseif msg == "fold-active" then
			state.active_folded = not state.active_folded
		elseif msg == "fold-pages" then
			state.pages_folded = not state.pages_folded
		elseif type(msg) == "table" and msg.fold then
			state.folded[msg.fold] = if state.folded[msg.fold] then nil else true
		elseif type(msg) == "table" and (msg.open or msg.pick) then
			state.picking = false
			local path = msg.open or msg.pick
			return state, {
				{ to = "provider:projects", msg = { select = path } },
				{ to = "workspace", msg = { project = path } },
			}
		end
	end,

	view = function(state: State, app)
		local projects = app.projects
		local agents = app.agents
		local current = projects and projects.current
		local window = app.window
		local inset = window.platform == "macos" and not window.fullscreen

		-- The switcher: the open project's name, a menu of every project.
		local items: { any } = {}
		for index, project in (projects and projects.projects) or {} do
			if index > 16 then
				break
			end
			table.insert(items, {
				id = project.path,
				label = project.name,
				detail = project.branch,
				icon = "file",
				checked = current ~= nil and project.path == current.path,
			})
		end
		local name = if current then current.name else "hollow"
		local head = ui.row {
			class = "pages-head",
			drag = true,
			height = 46,
			align = "center",
			shrink = false,
			inset and ui.row { width = 72 },
			ui.popover {
				open = state.picking,
				dismiss = "dismiss",
				width = 280,
				content = ui.column { class = "pages-menu", menu { items = items } },
				ui.row {
					class = "pages-switch",
					width = if inset then 162 else 234,
					align = "center",
					msg = "pick",
					tooltip = "Switch project",
					ui.row {
						class = "pages-tile",
						width = 22,
						height = 22,
						align = "center",
						justify = "center",
						ui.text { string.upper(string.sub(name, 1, 1)), weight = "bold" },
					},
					ui.text { name, class = "pages-name", truncate = true },
					ui.spacer(),
					ui.icon { name = "chevron-down", class = "pages-chevron", size = "small" },
				},
			},
		}

		local function row(project: any, depth: number, detail: string?, changed: boolean)
			local count = live(agents, project.path)
			return ui.row {
				class = "pages-row",
				height = 30,
				align = "center",
				shrink = false,
				selected = current ~= nil and project.path == current.path,
				msg = { open = project.path },
				tooltip = `{project.home}/{project.name} · {project.branch}`,
				depth > 0 and ui.row { width = 12 * depth },
				ui.icon { name = "file", class = "pages-row-icon" },
				ui.text { project.name, truncate = true },
				ui.spacer(),
				detail ~= nil and ui.text { detail, class = "pages-row-detail", truncate = true },
				changed and project.files > 0 and ui.text { tostring(project.files), class = "pages-changed", tooltip = "Files changed" },
				count > 0 and pill(count),
			}
		end

		local body: { any } = {}
		if not projects then
			table.insert(body, ui.text { "Looking for projects…", tone = "muted", style = { padding = { 8, 10 } } })
		else
			-- ACTIVE: the open project, then any other with an agent in it.
			local active: { any } = {}
			for _, project in projects.projects do
				if (current and project.path == current.path) or live(agents, project.path) > 0 then
					table.insert(active, project)
				end
			end
			table.insert(body, heading("Active", #active, state.active_folded, "fold-active"))
			if not state.active_folded then
				for _, project in active do
					local home = string.match(project.home, "([^/]+)$") or project.home
					table.insert(body, row(project, 0, home, false))
				end
			end

			-- PAGES: each project folder, its projects nested under it.
			table.insert(body, heading("Pages", nil, state.pages_folded, "fold-pages"))
			if not state.pages_folded then
				local homes: { string } = {}
				local by: { [string]: { any } } = {}
				for _, project in projects.projects do
					if not by[project.home] then
						by[project.home] = {}
						table.insert(homes, project.home)
					end
					table.insert(by[project.home], project)
				end
				for _, home in homes do
					local folded = state.folded[home] == true
					local changed = 0
					for _, project in by[home] do
						changed += project.files
					end
					table.insert(body, ui.row {
						class = "pages-row",
						height = 30,
						align = "center",
						shrink = false,
						msg = { fold = home },
						tooltip = home,
						ui.icon { name = if folded then "folder" else "folder-open", class = "pages-row-icon" },
						ui.text { string.match(home, "([^/]+)$") or home, truncate = true },
						ui.spacer(),
						ui.text { tostring(#by[home]), class = "pages-changed" },
					})
					if not folded then
						for _, project in by[home] do
							table.insert(body, row(project, 1, nil, true))
						end
					end
				end
			end
		end

		return ui.column {
			grow = true,
			gap = "none",
			head,
			ui.column { class = "pages-body", grow = true, scroll = true, ui.column { gap = "none", shrink = false, table.unpack(body) } },
			ui.row {
				class = "pages-art",
				shrink = false,
				height = 260,
				tooltip = "Happy Halloween",
				ui.image { "../../art/night.png", width = 250, height = 260, fit = "fill" },
			},
		}
	end,
}
