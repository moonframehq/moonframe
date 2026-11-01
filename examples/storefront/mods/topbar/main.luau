-- The title bar: room for the traffic lights, the sidebar toggle and a
-- search button, back and forward through the pages visited, and the open
-- tabs as a browser shows them. It keeps the tabs and publishes the one
-- in front on `store/tab`, which the sidebar and the page follow.

local tabs_strip = use "tabs"
local icon_button = use "icon-button"

type Tab = { id: string, page: string, label: string, icon: string }
type Visit = { tab: string, page: string, label: string, icon: string }
type State = { tabs: { Tab }, selected: string, next: number, history: { Visit }, at: number }

-- What the sidebar can open, by page id: its tab's label and icon.
local PAGES: { [string]: { label: string, icon: string } } = {
	home = { label = "Home", icon = "house" },
	audit = { label = "Audit log", icon = "history" },
}

local function find(state: State, id: string): (Tab?, number)
	for index, tab in state.tabs do
		if tab.id == id then
			return tab, index
		end
	end
	return nil, 0
end

-- Remember where we are, dropping anything forward of it, as a browser does.
local function visit(state: State, tab: Tab)
	while #state.history > state.at do
		table.remove(state.history)
	end
	local last = state.history[state.at]
	if last and last.tab == tab.id and last.page == tab.page then
		return
	end
	table.insert(state.history, { tab = tab.id, page = tab.page, label = tab.label, icon = tab.icon })
	if #state.history > 50 then
		table.remove(state.history, 1)
	end
	state.at = #state.history
end

local function publish(state: State)
	local tab = find(state, state.selected)
	return { { to = "topic:store/tab", msg = tab or { id = "", page = "home", label = "Home", icon = "house" } } }
end

local function close(state: State, id: string)
	local _, index = find(state, id)
	if index > 0 then
		table.remove(state.tabs, index)
		if state.selected == id then
			local next = state.tabs[index] or state.tabs[index - 1]
			state.selected = if next then next.id else ""
		end
	end
end

local function open_tab(state: State, tab: Tab)
	table.insert(state.tabs, tab)
	state.selected = tab.id
	visit(state, tab)
end

return mod {
	region = "top",
	title = "Title bar",

	styles = {
		[".topbar"] = { padding = { 0, 10, 0, 92 }, gap = 6 },
		[".topbar-tools"] = { gap = 4, padding = { right = 14 } },
	},

	init = function(previous: State?): State
		if previous and previous.tabs then
			return previous, publish(previous)
		end
		local state: State = {
			tabs = {
				{ id = "t1", page = "audit", label = "Audit log", icon = "history" },
				{ id = "t2", page = "order", label = "Order #3241", icon = "calendar" },
				{ id = "t3", page = "order", label = "Order #2905", icon = "calendar" },
				{ id = "t4", page = "product", label = "Summer Tee", icon = "tag" },
			},
			selected = "t1",
			next = 5,
			history = { { tab = "t1", page = "audit", label = "Audit log", icon = "history" } },
			at = 1,
		}
		return state, publish(state)
	end,

	update = function(state: State, msg)
		if msg == "new-tab" then
			open_tab(state, { id = `t{state.next}`, page = "home", label = "Home", icon = "house" })
			state.next += 1
		elseif msg == "close-current" then
			close(state, state.selected)
		elseif msg == "back" or msg == "forward" then
			local at = state.at + (if msg == "back" then -1 else 1)
			local entry = state.history[at]
			if not entry then
				return
			end
			state.at = at
			local open: Tab = { id = entry.tab, page = entry.page, label = entry.label, icon = entry.icon }
			local tab, index = find(state, entry.tab)
			if tab then
				state.tabs[index] = open
			else
				-- The tab was closed since: open it again.
				table.insert(state.tabs, open)
			end
			state.selected = open.id
		elseif type(msg) ~= "table" then
			return
		elseif msg.select then
			local tab = find(state, msg.select)
			if tab then
				state.selected = tab.id
				visit(state, tab)
			end
		elseif msg.close then
			close(state, msg.close)
		elseif msg.open then
			-- From the sidebar: show the page in the tab in front (a new tab
			-- when none is open), unless a tab already shows it.
			local page = PAGES[msg.open] or { label = msg.label or msg.open, icon = msg.icon or "file" }
			for _, tab in state.tabs do
				if tab.page == msg.open then
					state.selected = tab.id
					visit(state, tab)
					return state, publish(state)
				end
			end
			local tab = find(state, state.selected)
			if tab then
				tab.page, tab.label, tab.icon = msg.open, page.label, page.icon
				visit(state, tab)
			else
				open_tab(state, { id = `t{state.next}`, page = msg.open, label = page.label, icon = page.icon })
				state.next += 1
			end
		end
		return state, publish(state)
	end,

	view = function(state: State)
		return ui.row {
			class = "topbar",
			drag = true,
			grow = true,
			align = "center",
			ui.row {
				class = "topbar-tools",
				align = "center",
				icon_button { icon = "panel-left", tooltip = "Show or hide the sidebar", msg = "toggle-left", to = "shell" },
				icon_button { icon = "search", tooltip = "Find in the audit log", msg = "find", to = "audit" },
				ui.row { width = 4 },
				icon_button { icon = "arrow-left", tooltip = "Back", msg = "back", disabled = state.at <= 1 },
				icon_button { icon = "arrow-right", tooltip = "Forward", msg = "forward", disabled = state.at >= #state.history },
			},
			tabs_strip {
				variant = "browser",
				width = 200,
				selected = state.selected,
				add = "new-tab",
				tabs = state.tabs,
			},
		}
	end,
}
