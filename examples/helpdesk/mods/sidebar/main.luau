-- The sidebar: the brand, the workspace, a search field, the inbox's views
-- with their counts, the other sections (not in this example), and who is
-- signed in. The view and the query go out on the `desk/filter` topic, which
-- the ticket list follows.

local avatar = use "avatar"

type State = { view: string, query: string, focus: boolean }

local VIEWS: { { id: string, label: string, icon: string, count: string? } } = {
	{ id = "all", label = "All tickets", icon = "inbox", count = nil },
	{ id = "open", label = "All open", icon = "circle-dot", count = "open" },
	{ id = "mine", label = "Assigned to me", icon = "circle-user", count = "mine" },
	{ id = "waiting", label = "Waiting on employee", icon = "clock", count = "waiting" },
}

local SECTIONS = {
	{ id = "agents", label = "Agents", icon = "bot" },
	{ id = "knowledge", label = "Knowledge", icon = "book-open" },
	{ id = "integrations", label = "Integrations", icon = "layers" },
	{ id = "reports", label = "Reports", icon = "chart-column" },
}

local function initials(name: string): string
	local first, last = string.match(name, `^({utf8.charpattern})%S*%s+({utf8.charpattern})`)
	return if first and last then first .. last else (string.match(name, "^" .. utf8.charpattern) or "")
end

local function publish(state: State)
	return { { to = "topic:desk/filter", msg = { view = state.view, query = state.query } } }
end

return mod {
	region = "left",
	title = "Sidebar",

	styles = {
		[".side"] = { padding = { 0, 14, 14, 14 }, gap = 0 },
		[".side-brand"] = { padding = { 6, 8, 0, 8 }, gap = 6, height = 44 },
		[".side-logo"] = { background = "accent", color = "#ffffff", radius = 8 },
		[".side-name"] = { size = "title", weight = "bold" },
		[".side-workspace"] = { padding = { 10, 8 }, radius = 8, gap = 4, margin = { top = 22 } },
		[".side-workspace:hover"] = { background = "list-hover" },
		[".side-search"] = {
			background = "background",
			border = true,
			border_color = "border",
			radius = 10,
			padding = { 0, 12 },
			gap = 8,
			height = 36,
			margin = { top = 8 },
		},
		[".bare-input"] = { background = "#00000000", border = "none", shadow = "none", padding = { 6, 0 } },
		[".side-heading"] = { size = "small", color = "muted-foreground", padding = { 32, 10, 8, 10 } },
		[".side-row"] = { padding = { 0, 10 }, height = 36, radius = 10, gap = 12, margin = { bottom = 3 } },
		[".side-row:hover"] = { background = "list-hover" },
		[".side-row:selected"] = {
			background = "background",
			border = true,
			border_color = "border",
			shadow = "0 1px 2px #1c1c2210",
		},
		[".side-icon"] = { color = "muted-foreground" },
		[".side-count"] = { size = "small", color = "muted-foreground" },
		[".side-gap"] = { height = 34 },
		[".side-user"] = {
			background = "background",
			border = true,
			border_color = "border",
			radius = 12,
			padding = { 10, 12 },
			gap = 10,
		},
		[".side-user:hover"] = { border_color = "muted-foreground/40" },
		[".side-small"] = { size = "small", color = "muted-foreground" },
	},

	init = function(previous: State?): (State, any)
		local state: State = previous or { view = "all", query = "", focus = false }
		state.focus = false
		return state, publish(state)
	end,

	update = function(state: State, msg)
		state.focus = false
		if msg == "focus" then
			state.focus = true
		elseif type(msg) == "table" and msg.view then
			state.view = msg.view
			return state, publish(state)
		elseif type(msg) == "table" and msg.msg == "query" then
			state.query = msg.value
			return state, publish(state)
		elseif type(msg) == "table" and msg.msg == "clear" then
			state.query = ""
			return state, publish(state)
		end
		return state
	end,

	view = function(state: State, app)
		local desk = app.desk
		local window = app.window
		local counts = (desk and desk.counts) or {}
		local team = (desk and desk.workspace.team) or "IT Help"
		local company = (desk and desk.workspace.company) or ""
		local me = (desk and desk.me) or { name = "", email = "" }

		local function row(item, selected: boolean, count: string?, msg)
			return ui.row {
				class = "side-row",
				align = "center",
				selected = selected,
				msg = msg,
				ui.icon { name = item.icon :: any, class = if selected then nil else "side-icon" },
				ui.text { item.label, weight = if selected then "medium" else nil },
				ui.spacer(),
				count and ui.text { count, class = "side-count" },
			}
		end

		local views = {}
		for _, item in VIEWS do
			local count = if item.count then counts[item.count] else nil
			table.insert(views, row(item, state.view == item.id, if count then tostring(count) else nil, { view = item.id }))
		end
		local sections = {}
		for _, item in SECTIONS do
			table.insert(sections, ui.row {
				class = "side-row",
				align = "center",
				tooltip = "Not in this example",
				ui.icon { name = item.icon :: any, class = "side-icon" },
				ui.text { item.label },
			})
		end

		return ui.column {
			grow = true,
			gap = "none",
			ui.row { height = if window.platform == "macos" and not window.fullscreen then 34 else 10, drag = true, shrink = false },
			ui.column {
				class = "side",
				grow = true,
				ui.row {
					class = "side-brand",
					align = "center",
					ui.row {
						class = "side-logo",
						width = 28,
						height = 28,
						align = "center",
						justify = "center",
						ui.icon { name = "sparkles" },
					},
					ui.text { (desk and desk.assistant) or "Tandem", class = "side-name" },
				},
				ui.row {
					class = "side-workspace",
					align = "center",
					tooltip = "Only one workspace in this example",
					ui.text { team },
					ui.text { `at {company}`, tone = "muted" },
					ui.spacer(),
					ui.icon { name = "chevrons-up-down", size = "small", tone = "muted" },
				},
				ui.row {
					class = "side-search",
					align = "center",
					ui.icon { name = "search", tone = "muted", size = "small" },
					ui.input {
						id = "search",
						class = "bare-input",
						grow = true,
						value = state.query,
						placeholder = "Search",
						change = "query",
						focus = state.focus,
						keys = { escape = "clear" },
					},
				},
				ui.text { "Inbox", class = "side-heading" },
				ui.column { gap = "none", table.unpack(views) },
				ui.row { class = "side-gap" },
				ui.column { gap = "none", table.unpack(sections) },
				ui.spacer(),
				ui.row {
					class = "side-user",
					align = "center",
					tooltip = me.email,
					avatar { name = me.name, initial = initials(me.name), shape = "circle", size = "large" },
					ui.column {
						gap = "none",
						grow = true,
						ui.text { me.name, weight = "medium" },
						ui.text { me.email, class = "side-small", truncate = true },
					},
					ui.icon { name = "chevrons-up-down", size = "small", tone = "muted" },
				},
			},
		}
	end,
}
