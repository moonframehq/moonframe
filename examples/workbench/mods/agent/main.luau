-- The terminal card: tabs of shells, each started in the project open when
-- it was made, so an agent can run in one beside a plain shell in another.
-- The header names the tab, its folder (a menu of projects to switch to),
-- the branch, and buttons to split the shell right or below.

local icon_button = use "icon-button"

type Shell = { id: string, cwd: string, name: string }

type State = {
	shells: { Shell },
	selected: string?,
	-- A second shell beside the selected one: "right" or "below".
	split: string?,
	split_id: string?,
	picking: boolean,
	made: number,
	project: string?,
}

-- Moonframe keeps up to 8 shells a mod; one is the first project's.
local MOST = 7

local function tilde(path: string, home: string?): string
	if home then
		return `{home}/{string.match(path, "([^/]+)$")}`
	end
	return path
end

local function find(state: State, id: string?): Shell?
	for _, shell in state.shells do
		if shell.id == id then
			return shell
		end
	end
	return nil
end

-- A new shell in `cwd`, if there is room for one.
local function add(state: State, cwd: string): string?
	if state.made >= MOST then
		return nil
	end
	state.made += 1
	local id = `t{state.made}`
	local name = string.match(cwd, "([^/]+)$") or cwd
	table.insert(state.shells, { id = id, cwd = cwd, name = name })
	return id
end

return mod {
	region = "left",
	title = "Terminal",

	styles = {
		[".agent-tab"] = { padding = { 4, 10 }, radius = 8, gap = 6, color = "muted-foreground" },
		[".agent-tab:hover"] = { background = "list-hover" },
		[".agent-tab:selected"] = { background = "secondary", color = "foreground" },
		[".agent-folder"] = { color = "accent", padding = { 3, 6 }, radius = 6 },
		[".agent-folder:hover"] = { background = "accent/10" },
		[".agent-branch"] = { background = "secondary", radius = 7, padding = { 3, 8 }, gap = 5 },
		[".agent-body"] = { padding = { 0, 4, 4, 14 }, gap = 8 },
		[".agent-split"] = { border = "left", border_color = "border", padding = { 0, 0, 0, 10 } },
		[".agent-split-below"] = { border = "top", border_color = "border", padding = { 10, 0, 0, 0 } },
		[".agent-pick"] = { padding = { 6, 10 }, radius = 8, gap = 2 },
		[".agent-pick:hover"] = { background = "list-hover" },
		[".agent-pick:selected"] = { background = "accent/12" },
		[".agent-menu"] = { padding = 6, gap = 0 },
	},

	init = function(previous: State?): State
		return previous
			or { shells = {}, selected = nil, split = nil, split_id = nil, picking = false, made = 0, project = nil }
	end,

	update = function(state: State, msg)
		-- The shell shown before any was made joins the tabs.
		if type(msg) == "table" and msg.shown and #state.shells == 0 then
			table.insert(state.shells, { id = "t0", cwd = msg.shown, name = string.match(msg.shown, "([^/]+)$") or msg.shown })
			state.selected = "t0"
		end
		if type(msg) == "table" and msg.new then
			state.selected = add(state, msg.new) or state.selected
		elseif msg == "new" then
			-- From the menu: beside the shell shown.
			local shell = find(state, state.selected)
			local cwd = (shell and shell.cwd) or state.project
			if cwd then
				state.selected = add(state, cwd) or state.selected
			end
		elseif msg == "pick" then
			state.picking = not state.picking
		elseif msg == "dismiss" then
			state.picking = false
		elseif type(msg) == "table" and msg.split then
			if state.split == msg.split then
				state.split = nil
			else
				state.split = msg.split
				local shell = find(state, state.selected)
				if shell and not find(state, state.split_id) then
					state.split_id = add(state, shell.cwd)
				end
			end
		elseif type(msg) == "table" then
			if msg.tab then
				state.selected = msg.tab
			elseif msg.close then
				for index, shell in state.shells do
					if shell.id == msg.close then
						table.remove(state.shells, index)
					end
				end
				if state.selected == msg.close then
					state.selected = state.shells[1] and state.shells[1].id
				end
			elseif msg.project then
				-- A project chosen here: tell the provider and the others.
				state.picking = false
				state.project = msg.project
				state.selected = add(state, msg.project) or state.selected
				return state, {
					{ to = "provider:projects", msg = { select = msg.project } },
					{ to = "editor", msg = { project = msg.project } },
					{ to = "files", msg = { project = msg.project } },
				}
			end
		end
	end,

	view = function(state: State, app)
		local projects = app.projects
		local current = projects and projects.current
		local window = app.window
		local inset = window.platform == "macos" and not window.fullscreen

		-- The first project the provider opens gets a shell of its own.
		local shells = state.shells
		local shown = find(state, state.selected)
		local implicit = false
		if not shown and current then
			shown = { id = "t0", cwd = current.path, name = current.name }
			implicit = true
		end
		local summary
		for _, project in (projects and projects.projects) or {} do
			if shown and project.path == shown.cwd then
				summary = project
			end
		end

		local tab_rows = {}
		if implicit and shown then
			table.insert(tab_rows, ui.row {
				class = "agent-tab",
				selected = true,
				align = "center",
				shrink = false,
				max_width = 150,
				ui.icon { name = "square-terminal", size = "small" },
				ui.text { shown.name, weight = "medium", truncate = true },
			})
		end
		for _, shell in shells do
			table.insert(tab_rows, ui.row {
				class = "agent-tab",
				selected = shell.id == state.selected,
				align = "center",
				shrink = false,
				max_width = 150,
				msg = { tab = shell.id },
				tooltip = shell.cwd,
				ui.icon { name = "square-terminal", size = "small" },
				ui.text { shell.name, weight = "medium", truncate = true },
				#shells > 1 and ui.button { icon = "close", variant = "ghost", small = true, msg = { close = shell.id } },
			})
		end

		local picks = {}
		for index, project in (projects and projects.projects) or {} do
			if index > 14 then
				break
			end
			table.insert(picks, ui.column {
				class = "agent-pick",
				selected = shown ~= nil and project.path == shown.cwd,
				msg = { project = project.path, shown = implicit and shown and shown.cwd or nil },
				ui.text { project.name, weight = "medium" },
				ui.text { `{project.home} · {project.branch}`, tone = "muted", size = "small", truncate = true },
			})
		end

		local header = ui.row {
			class = "card-header",
			drag = true,
			height = 50,
			align = "center",
			inset and ui.row { width = 70 },
			ui.row {
				shrink = false,
				gap = "none",
				icon_button { icon = "panel-right", tooltip = "Show or hide the editor", msg = "toggle-editor", to = "shell" },
				icon_button {
					icon = "plus",
					tooltip = "New shell in this project",
					msg = { new = shown and shown.cwd, shown = implicit and shown and shown.cwd or nil },
					disabled = state.made >= MOST or not shown,
				},
			},
			ui.row { gap = "small", align = "center", shrink = false, table.unpack(tab_rows) },
			shown and ui.popover {
				open = state.picking,
				dismiss = "dismiss",
				width = 320,
				content = ui.column { class = "agent-menu", table.unpack(picks) },
				ui.row {
					class = "agent-folder",
					max_width = 160,
					msg = "pick",
					tooltip = "Open another project",
					ui.text { tilde(shown.cwd, summary and summary.home), truncate = true },
				},
			},
			summary and summary.branch ~= "" and ui.row {
				class = "agent-branch",
				align = "center",
				max_width = 130,
				tooltip = `{summary.branch}  +{summary.added} −{summary.deleted}`,
				ui.icon { name = "git-branch", size = "small", tone = "muted" },
				ui.text { summary.branch, size = "small", truncate = true },
			},
			ui.spacer(),
			ui.row { shrink = false, gap = "none", icon_button {
				icon = "columns-2",
				tooltip = "Split right",
				msg = { split = "right", shown = implicit and shown and shown.cwd or nil },
				active = state.split == "right",
			},
			icon_button {
				icon = "rows-2",
				tooltip = "Split below",
				msg = { split = "below", shown = implicit and shown and shown.cwd or nil },
				active = state.split == "below",
			} },
		}

		local body
		if not shown then
			body = ui.column {
				grow = true,
				align = "center",
				justify = "center",
				ui.text { if projects then "No project open" else "Looking for projects…", tone = "muted" },
			}
		else
			local main = ui.column { grow = true, ui.terminal { id = shown.id, cwd = shown.cwd } }
			local second = state.split and find(state, state.split_id)
			local split = second
				and ui.column {
					grow = true,
					class = if state.split == "right" then "agent-split" else "agent-split-below",
					ui.terminal { id = second.id, cwd = second.cwd },
				}
			if state.split == "right" then
				body = ui.row { class = "agent-body", grow = true, main, split }
			else
				body = ui.column { class = "agent-body", grow = true, main, split }
			end
		end

		return ui.column { class = "card", grow = true, gap = "none", header, body }
	end,
}
