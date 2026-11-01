-- The workspace: a title row of tabs, the grid of panes for the chosen tab,
-- and an icon rail at the right edge. Each pane is framed by `pane-frame`
-- (icon, title, a menu of what it shows, zoom, close) and shows a live
-- terminal, the project's files, the processes or the changes. A click in a
-- pane gives it the focus, drawn as an ember frame; the rail and the Pane
-- menu act on the focused pane. Tabs, panes and their sizes are kept.

local pane_frame = use "pane-frame"
local icon_button = use "icon-button"
local menu = use "menu"

type Tab = { id: string, columns: { { string } }, sizes: { number } }
type Pane = { kind: string }

type State = {
	tabs: { Tab },
	selected: string,
	panes: { [string]: Pane },
	focus: string?,
	zoomed: string?,
	menu: string?,
	made: number,
	-- Every terminal id ever started: the host keeps 8 shells a mod.
	shells: { string },
	rail: boolean,
	sidebar: boolean,
}

local KINDS: { [string]: { icon: string, label: string } } = {
	terminal = { icon = "square-terminal", label = "Terminal" },
	files = { icon = "list-tree", label = "Files" },
	processes = { icon = "activity", label = "Processes" },
	changes = { icon = "git-branch", label = "Changes" },
}
local ORDER = { "terminal", "files", "changes", "processes" }
local MOST_SHELLS = 8

local function fresh(): State
	return {
		tabs = {
			{ id = "t1", columns = { { "a", "b" }, { "c" }, { "d", "e" } }, sizes = { 0, 222, 340 } },
			{ id = "t2", columns = { { "f" }, { "g" } }, sizes = { 0, 420 } },
			{ id = "t3", columns = { { "h", "i" } }, sizes = { 0 } },
			{ id = "t4", columns = { { "j" } }, sizes = { 0 } },
		},
		selected = "t1",
		panes = {
			a = { kind = "terminal" },
			b = { kind = "terminal" },
			c = { kind = "files" },
			d = { kind = "processes" },
			e = { kind = "changes" },
			f = { kind = "terminal" },
			g = { kind = "processes" },
			h = { kind = "terminal" },
			i = { kind = "changes" },
			j = { kind = "terminal" },
		},
		focus = "b",
		zoomed = nil,
		menu = nil,
		made = 0,
		shells = {},
		rail = true,
		sidebar = true,
	}
end

local function tab_of(state: State): Tab
	for _, tab in state.tabs do
		if tab.id == state.selected then
			return tab
		end
	end
	return state.tabs[1]
end

-- Where a pane sits in the tab: its column and its place there.
local function locate(tab: Tab, id: string): (number?, number?)
	for c, column in tab.columns do
		for r, pane in column do
			if pane == id then
				return c, r
			end
		end
	end
	return nil, nil
end

local function first_pane(tab: Tab): string?
	return tab.columns[1] and tab.columns[1][1]
end

local function has_shell(state: State, id: string): boolean
	return table.find(state.shells, id) ~= nil
end

-- Whether pane `id` may show a terminal: it has one already, or there is
-- room for another shell.
local function shell_room(state: State, id: string): boolean
	return has_shell(state, id) or #state.shells < MOST_SHELLS
end

local function set_kind(state: State, id: string, kind: string)
	if kind == "terminal" then
		if not shell_room(state, id) then
			return
		end
		if not has_shell(state, id) then
			table.insert(state.shells, id)
		end
	end
	state.panes[id] = { kind = kind }
end

local function new_pane(state: State, kind: string): string
	state.made += 1
	local id = `p{state.made}`
	state.panes[id] = { kind = "files" }
	set_kind(state, id, kind)
	return id
end

local function split(state: State, way: string)
	local tab = tab_of(state)
	local id = state.focus or first_pane(tab)
	local c, r = nil, nil
	if id then
		c, r = locate(tab, id)
	end
	local pane = new_pane(state, "terminal")
	if not c or not r then
		table.insert(tab.columns, { pane })
		table.insert(tab.sizes, 0)
	elseif way == "right" then
		table.insert(tab.columns, c + 1, { pane })
		table.insert(tab.sizes, c + 1, 0)
	else
		table.insert(tab.columns[c], r + 1, pane)
	end
	state.focus = pane
	state.zoomed = nil
end

local function close(state: State, id: string)
	local tab = tab_of(state)
	local c, r = locate(tab, id)
	if not c or not r then
		return
	end
	table.remove(tab.columns[c], r)
	if #tab.columns[c] == 0 then
		table.remove(tab.columns, c)
		table.remove(tab.sizes, c)
	end
	if state.zoomed == id then
		state.zoomed = nil
	end
	if #tab.columns == 0 then
		-- The last pane of a tab closes the tab, unless it is the only one.
		if #state.tabs > 1 then
			local index = table.find(state.tabs, tab) or 1
			table.remove(state.tabs, index)
			state.selected = state.tabs[math.min(index, #state.tabs)].id
		else
			tab.columns = { { new_pane(state, "files") } }
			tab.sizes = { 0 }
		end
	end
	state.focus = first_pane(tab_of(state))
end

local function step(state: State, by: number)
	for index, tab in state.tabs do
		if tab.id == state.selected then
			local next = state.tabs[(index - 1 + by) % #state.tabs + 1]
			state.selected = next.id
			state.focus = first_pane(next)
			state.zoomed = nil
			return
		end
	end
end

local function count_shells(tab: Tab, state: State): number
	local count = 0
	for _, column in tab.columns do
		for _, id in column do
			if state.panes[id] and state.panes[id].kind == "terminal" then
				count += 1
			end
		end
	end
	return count
end

return mod {
	region = "main",
	title = "Workspace",

	styles = {
		[".ws-title"] = { background = "night", border = "bottom", border_color = "border", padding = { 0, 8 }, gap = 4 },
		[".ws-rule"] = { background = "border" },
		[".ws-tab"] = { padding = { 0, 10 }, radius = 7, gap = 7, color = "muted-foreground" },
		[".ws-tab:hover"] = { background = "list-hover", color = "foreground" },
		[".ws-tab:selected"] = { background = "card", color = "foreground", border = true, border_color = "border" },
		[".ws-tab-pill"] = { background = "secondary", radius = 8, padding = { 1, 6 }, gap = 5 },
		[".ws-tab-dot"] = { background = "success", radius = 3 },
		[".ws-tab-count"] = { size = "small", color = "muted-foreground" },
		[".ws-grid"] = { padding = 3 },
		[".ws-cell"] = { padding = 3 },
		[".ws-kind"] = { padding = { 3, 5 }, radius = 5, gap = 2, color = "muted-foreground" },
		[".ws-kind:hover"] = { background = "list-hover", color = "foreground" },
		[".ws-kind:selected"] = { background = "row-selected", color = "foreground" },
		[".ws-live"] = { background = "success", radius = 4 },
		[".ws-rail"] = { background = "sidebar", border = "left", border_color = "border", padding = { 6, 0 }, gap = 4 },
		[".ws-rail-item"] = { radius = 6, color = "muted-foreground" },
		[".ws-rail-item:hover"] = { background = "list-hover", color = "foreground" },
		[".ws-rail-item:selected"] = { background = "row-selected", color = "pumpkin" },
		[".ws-menu"] = { padding = 0 },
	},

	init = function(previous: State?): State
		local state = previous or fresh()
		state.menu = nil
		-- Terminals the first tabs start with.
		for id, pane in state.panes do
			if pane.kind == "terminal" and not has_shell(state, id) and #state.shells < MOST_SHELLS then
				table.insert(state.shells, id)
			end
		end
		return state
	end,

	update = function(state: State, msg)
		local tab = tab_of(state)
		if type(msg) == "table" then
			if msg.focus then
				state.focus = msg.focus
			elseif msg.tab then
				state.selected = msg.tab
				state.focus = first_pane(tab_of(state))
				state.zoomed = nil
			elseif msg.close_tab then
				for index, t in state.tabs do
					if t.id == msg.close_tab and #state.tabs > 1 then
						table.remove(state.tabs, index)
						if state.selected == msg.close_tab then
							state.selected = state.tabs[math.min(index, #state.tabs)].id
							state.focus = first_pane(tab_of(state))
						end
						break
					end
				end
			elseif msg.menu then
				state.menu = if state.menu == msg.menu then nil else msg.menu
				state.focus = msg.menu
			elseif msg.pick and state.menu then
				local id = state.menu
				state.menu = nil
				state.focus = id
				local what, value = string.match(msg.pick, "^(%a+):(%a+)$")
				if what == "kind" and value then
					set_kind(state, id, value)
				elseif what == "split" and value then
					split(state, value)
				elseif what == "pane" and value == "zoom" then
					state.zoomed = if state.zoomed == id then nil else id
				elseif what == "pane" and value == "close" then
					close(state, id)
				end
			elseif msg.kind then
				-- From the rail or the Pane menu: the focused pane.
				local id = state.focus or first_pane(tab)
				if id then
					set_kind(state, id, msg.kind)
				end
			elseif msg.split then
				split(state, msg.split)
			elseif msg.zoom then
				state.focus = msg.zoom
				state.zoomed = if state.zoomed == msg.zoom then nil else msg.zoom
			elseif msg.close then
				close(state, msg.close)
			elseif msg.sidebar ~= nil then
				state.sidebar = msg.sidebar
			elseif msg.project then
				-- Nothing to keep: the panes read the open project.
				return
			end
		elseif msg == "dismiss" then
			state.menu = nil
		elseif msg == "zoom" then
			if state.focus then
				state.zoomed = if state.zoomed == state.focus then nil else state.focus
			end
		elseif msg == "close" then
			if state.focus then
				close(state, state.focus)
			end
		elseif msg == "new-tab" then
			state.made += 1
			local id = `t{state.made}x`
			local pane = new_pane(state, if #state.shells < MOST_SHELLS then "terminal" else "files")
			table.insert(state.tabs, { id = id, columns = { { pane } }, sizes = { 0 } })
			state.selected = id
			state.focus = pane
			state.zoomed = nil
		elseif msg == "previous" then
			step(state, -1)
		elseif msg == "next" then
			step(state, 1)
		elseif msg == "toggle-rail" then
			state.rail = not state.rail
		end
	end,

	view = function(state: State, app)
		local tab = tab_of(state)
		local window = app.window
		local inset = window.platform == "macos" and not window.fullscreen and not state.sidebar
		local projects = app.projects
		local current = projects and projects.current
		local cwd = if current then current.path else "~"

		-- Agents running in the open project light the first terminal's dot.
		local agents = 0
		local running: { any } = (app.agents and app.agents.agents) or {}
		for _, agent in running do
			if agent.cwd and current and string.sub(agent.cwd, 1, #current.path) == current.path then
				agents += 1
			end
		end

		-- The title row: sidebar, history, tabs, add, rail.
		local tab_rows: { any } = {}
		for index, t in state.tabs do
			local selected = t.id == state.selected
			local shells = count_shells(t, state)
			table.insert(tab_rows, ui.row {
				class = "ws-tab",
				key = t.id,
				selected = selected,
				height = 28,
				align = "center",
				shrink = false,
				msg = { tab = t.id },
				selected and shells > 0 and ui.row {
					class = "ws-tab-pill",
					align = "center",
					tooltip = `{shells} {if shells == 1 then "terminal" else "terminals"}`,
					ui.row { class = "ws-tab-dot", width = 6, height = 6 },
					ui.text { tostring(shells), class = "ws-tab-count" },
				},
				ui.text { `tab {index}` },
				selected and #state.tabs > 1 and ui.row {
					msg = { close_tab = t.id },
					tooltip = "Close tab",
					ui.icon { name = "close", size = "small" },
				},
			})
		end
		local title = ui.row {
			class = "ws-title",
			drag = true,
			height = 46,
			align = "center",
			shrink = false,
			inset and ui.row { width = 72 },
			icon_button { icon = "panel-left", tooltip = "Show or hide pages", msg = "toggle-left", to = "shell", size = "small" },
			icon_button { icon = "chevron-left", tooltip = "Previous tab", msg = "previous", size = "small" },
			icon_button { icon = "chevron-right", tooltip = "Next tab", msg = "next", size = "small" },
			ui.row { class = "ws-rule", width = 1, height = 18 },
			ui.row { gap = "small", align = "center", shrink = false, table.unpack(tab_rows) },
			icon_button { icon = "plus", tooltip = "New tab", msg = "new-tab", size = "small" },
			ui.spacer(),
			icon_button { icon = "panel-right", tooltip = "Show or hide the rail", msg = "toggle-rail", size = "small", active = not state.rail },
		}

		-- One pane's frame and content.
		local number = 0
		local function frame(id: string)
			local pane = state.panes[id] or { kind = "files" }
			local kind = KINDS[pane.kind] or KINDS.files
			number += 1
			local focused = state.focus == id
			local terminal = pane.kind == "terminal"

			local items: { any } = { { heading = "Show" } }
			for _, name in ORDER do
				table.insert(items, {
					id = `kind:{name}`,
					label = KINDS[name].label,
					icon = KINDS[name].icon,
					checked = pane.kind == name,
					disabled = name == "terminal" and not shell_room(state, id),
				})
			end
			table.insert(items, "separator")
			table.insert(items, { id = "split:right", label = "Split right", icon = "square-split-horizontal", shortcut = "cmd-d" })
			table.insert(items, { id = "split:down", label = "Split down", icon = "square-split-vertical", shortcut = "cmd-shift-d" })
			table.insert(items, { id = "pane:zoom", label = if state.zoomed == id then "Restore" else "Zoom", icon = "maximize-2" })
			table.insert(items, { id = "pane:close", label = "Close pane", icon = "close", danger = true })

			local kind_button = ui.popover {
				open = state.menu == id,
				dismiss = "dismiss",
				align = "end",
				width = 210,
				content = ui.column { class = "ws-menu", menu { items = items } },
				ui.row {
					class = "ws-kind",
					align = "center",
					selected = state.menu == id,
					msg = { menu = id },
					tooltip = `Showing {string.lower(kind.label)}: change or split`,
					ui.icon { name = kind.icon :: any, size = "small" },
					ui.icon { name = "chevron-down", size = "small" },
				},
			}

			local body
			if terminal and not projects then
				-- Wait for the open project, so the shell starts in it.
				body = ui.column { grow = true, align = "center", justify = "center", ui.text { "Opening…", tone = "muted" } }
			elseif terminal then
				body = ui.terminal { id = id, cwd = cwd }
			elseif pane.kind == "files" then
				body = ui.mount { "explorer", id = id }
			elseif pane.kind == "processes" then
				body = ui.mount { "monitor", id = id }
			else
				body = ui.mount { "changes", id = id }
			end

			return pane_frame {
				title = if terminal then "Terminal" else `pane {number}`,
				icon = if terminal then "square-terminal" else nil,
				leading = if terminal and number == 1 and agents > 0
					then { ui.row { class = "ws-live", width = 8, height = 8, tooltip = `{agents} agent running` } }
					else nil,
				menu = if terminal then { menu = id } else nil,
				focused = focused,
				focus = { focus = id },
				click_focus = true,
				zoomed = state.zoomed == id,
				zoom = { zoom = id },
				close = { close = id },
				actions = { kind_button },
				ui.column { grow = true, body },
			}
		end

		local grid
		if state.zoomed and locate(tab, state.zoomed) then
			grid = ui.column { class = "ws-cell", grow = true, frame(state.zoomed) }
		else
			local columns: { any } = {}
			for c, column in tab.columns do
				local cells: { any } = {}
				if #column > 1 then
					for _, id in column do
						table.insert(cells, ui.pane { class = "ws-cell", frame(id) })
					end
				else
					table.insert(cells, false)
				end
				local size = tab.sizes[c] or 0
				table.insert(columns, ui.pane {
					size = if size > 0 then size else nil,
					min = 160,
					if #cells == 1 then ui.column { class = "ws-cell", grow = true, frame(column[1]) } else ui.vsplit { id = `{tab.id}-col-{c}-{#cells}`, divider = false, table.unpack(cells) },
				})
			end
			grid = ui.hsplit { id = `{tab.id}-cols-{#columns}`, divider = false, table.unpack(columns) }
		end

		-- The rail: what the focused pane shows, and the themes.
		local focused_kind = state.focus and state.panes[state.focus] and state.panes[state.focus].kind
		local rail_items: { any } = {}
		for _, name in ORDER do
			table.insert(rail_items, ui.row {
				class = "ws-rail-item",
				width = 30,
				height = 30,
				align = "center",
				justify = "center",
				selected = focused_kind == name,
				msg = { kind = name },
				tooltip = `Show {string.lower(KINDS[name].label)} in the focused pane`,
				ui.icon { name = KINDS[name].icon :: any },
			})
		end
		table.insert(rail_items, ui.spacer())
		table.insert(rail_items, ui.row {
			class = "ws-rail-item",
			width = 30,
			height = 30,
			align = "center",
			justify = "center",
			command = "show-themes",
			tooltip = "Themes: try Hallows",
			ui.icon "settings",
		})

		return ui.column {
			grow = true,
			gap = "none",
			title,
			ui.row {
				grow = true,
				gap = "none",
				ui.column { class = "ws-grid", grow = true, grid },
				state.rail and ui.column {
					class = "ws-rail",
					width = 40,
					align = "center",
					shrink = false,
					table.unpack(rail_items),
				},
			},
		}
	end,
}
