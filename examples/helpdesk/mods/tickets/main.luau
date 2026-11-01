-- The ticket list: a header with the view's name, the sort and a + for a
-- new ticket; the week in a sentence and a bar split three ways; then one
-- row per ticket, its priority as the left edge. Follows `desk/filter` (the
-- sidebar's view and query) and publishes the picked ticket on
-- `desk/selected`.

local avatar = use "avatar"
local share_bar = use "share-bar"
local menu = use "menu"

type State = {
	selected: string?,
	sort: string,
	sort_open: boolean,
	view: string,
	query: string,
}

local TITLES = {
	all = "All tickets",
	open = "All open",
	mine = "Assigned to me",
	waiting = "Waiting on employee",
}

local SORTS = {
	{ id = "newest", label = "Newest" },
	{ id = "oldest", label = "Oldest" },
	{ id = "priority", label = "Priority" },
}

local RANK = { High = 1, Medium = 2, Low = 3 }

-- Where each ticket came from, as an icon in that app's colour.
local SOURCES = {
	slack = { icon = "hash", color = "#e2457a", name = "Slack" },
	teams = { icon = "message-square", color = "#6264a7", name = "Teams" },
	web = { icon = "globe", color = "#7d7d8a", name = "Web" },
}

local function matches(ticket, view: string, query: string): boolean
	if view == "open" and ticket.status == "resolved" then
		return false
	elseif view == "mine" and (not ticket.mine or ticket.status == "resolved") then
		return false
	elseif view == "waiting" and ticket.status ~= "waiting" then
		return false
	end
	if query == "" then
		return true
	end
	local needle = string.lower(query)
	for _, hay in { ticket.id, ticket.title, ticket.requester.name, ticket.kind } do
		if string.find(string.lower(hay), needle, 1, true) then
			return true
		end
	end
	return false
end

local function visible(desk, state: State): { any }
	local list = {}
	for _, ticket in desk.tickets do
		if matches(ticket, state.view, state.query) then
			table.insert(list, ticket)
		end
	end
	table.sort(list, function(a, b)
		if state.sort == "oldest" then
			return a.minutes > b.minutes
		elseif state.sort == "priority" and RANK[a.priority] ~= RANK[b.priority] then
			return (RANK[a.priority] or 9) < (RANK[b.priority] or 9)
		end
		return a.minutes < b.minutes
	end)
	return list
end

-- The provider's latest data, for stepping through the list from the menu.
local latest: any = nil

local function pick(state: State, id: string?)
	state.selected = id
	return state, { { to = "topic:desk/selected", msg = id or "" } }
end

return mod {
	region = "list",
	title = "Tickets",
	subscribe = { "desk/filter", "provider/desk" },

	styles = {
		[".list-head"] = { padding = { 0, 22 }, height = 66, gap = 10, border = "bottom", border_color = "border" },
		[".list-title"] = { size = "large", weight = "medium" },
		[".list-sort"] = { padding = { 6, 8 }, radius = 8, gap = 6, color = "muted-foreground" },
		[".list-sort:hover"] = { background = "list-hover" },
		[".list-add"] = { border = true, border_color = "border", radius = 9, background = "background" },
		[".list-add:hover"] = { background = "secondary" },
		[".week"] = { padding = { 20, 22, 22, 22 }, gap = 0, border = "bottom", border_color = "border" },
		[".week-label"] = { size = "small", color = "muted-foreground", margin = { bottom = 6 } },
		[".week-line"] = { gap = 0 },
		[".week-legend"] = { gap = 18, margin = { top = 12 } },
		[".week-key"] = { gap = 6 },
		[".week-swatch"] = { radius = 2 },
		[".ticket"] = { padding = { 15, 22, 15, 13 }, gap = 0, border = "bottom", border_color = "border" },
		[".ticket:hover"] = { background = "list-hover" },
		[".ticket:selected"] = { background = "secondary" },
		[".ticket-edge"] = { radius = 2, margin = { right = 13 } },
		[".edge-high"] = { background = "high" },
		[".edge-medium"] = { background = "medium" },
		[".edge-low"] = { background = "low" },
		[".ticket-body"] = { gap = 6 },
		[".ticket-id"] = { size = "small", color = "muted-foreground" },
		[".ticket-title"] = { weight = "medium" },
		[".ticket-meta"] = { gap = 6, margin = { top = 3 } },
		[".ticket-sub"] = { color = "muted-foreground" },
		[".ticket-dot"] = { radius = 3, background = "low" },
		[".ticket-status"] = { color = "muted-foreground", gap = 7 },
		[".resolved"] = { background = "lime-soft", color = "lime-ink", radius = 7, padding = { 3, 8, 3, 4 }, gap = 6 },
		[".resolved-tile"] = { background = "accent", color = "#ffffff", radius = 4 },
		[".list-empty"] = { padding = 40, color = "muted-foreground" },
	},

	init = function(previous: State?): (State, any)
		local state: State = previous or { selected = "INC-1206", sort = "newest", sort_open = false, view = "all", query = "" }
		state.sort_open = false
		return pick(state, state.selected)
	end,

	update = function(state: State, msg)
		if type(msg) ~= "table" then
			if msg == "toggle-sort" then
				state.sort_open = not state.sort_open
			elseif msg == "close-sort" then
				state.sort_open = false
			elseif msg == "new" then
				return state, { { to = "provider:desk", msg = { new = true } } }
			end
			return state
		end
		if msg.topic == "provider/desk" then
			latest = msg.value
		elseif msg.topic == "desk/filter" and type(msg.value) == "table" then
			state.view = msg.value.view or "all"
			state.query = msg.value.query or ""
		elseif msg.pick then
			state.sort = msg.pick
			state.sort_open = false
		elseif msg.select then
			return pick(state, msg.select)
		elseif msg.step and latest and latest.tickets then
			local list = visible(latest, state)
			local at = 0
			for index, ticket in list do
				if ticket.id == state.selected then
					at = index
				end
			end
			local next = list[math.clamp(at + msg.step, 1, math.max(#list, 1))]
			if next then
				return pick(state, next.id)
			end
		end
		return state
	end,

	view = function(state: State, app)
		local desk = app.desk
		local list: { any } = if desk then visible(desk, state) else {}
		local sort_label = "Newest"
		for _, sort in SORTS do
			if sort.id == state.sort then
				sort_label = sort.label
			end
		end

		local head = ui.row {
			class = "list-head",
			align = "center",
			shrink = false,
			drag = true,
			ui.text { TITLES[state.view] or "All tickets", class = "list-title" },
			ui.spacer(),
			ui.popover {
				open = state.sort_open,
				dismiss = "close-sort",
				align = "end",
				width = 180,
				content = menu { items = SORTS, highlighted = state.sort },
				ui.row {
					class = "list-sort",
					align = "center",
					msg = "toggle-sort",
					ui.text { sort_label },
					ui.icon { name = "chevron-down", size = "small" },
				},
			},
			ui.row {
				class = "list-add",
				width = 32,
				height = 32,
				align = "center",
				justify = "center",
				msg = "new",
				tooltip = "New ticket",
				ui.icon { name = "plus" },
			},
		}

		if not desk then
			return ui.column { grow = true, gap = "none", head, ui.text { "Loading tickets…", class = "list-empty" } }
		end

		-- The week: a sentence with its figure in bold, a bar, a legend.
		local week = desk.week
		local words = {}
		local sentence = `{desk.assistant} resolved *{week.share}%* of {week.total} tickets with no human, first reply in {week.first_reply} on average.`
		for word in string.gmatch(sentence, "%S+") do
			local bold = string.match(word, "^%*(.*)%*$")
			table.insert(words, ui.text { (bold or word) .. " ", weight = if bold then "bold" else nil })
		end
		local shades = { assistant = "lime", team = "ink", open = "low" }
		local shares, keys = {}, {}
		for _, part in week.split do
			table.insert(shares, { share = part.count, color = shades[part.id], label = `{part.label} {part.count}` })
			table.insert(keys, ui.row {
				class = "week-key",
				align = "center",
				ui.row { class = "week-swatch", width = 8, height = 8, style = { background = shades[part.id] } },
				ui.text { part.label, tone = "muted" },
				ui.text { tostring(part.count), weight = "medium" },
			})
		end

		local rows = {}
		for _, ticket in list do
			local source = SOURCES[ticket.source] or SOURCES.web
			local status = if ticket.status == "resolved"
				then ui.row {
					class = "resolved",
					align = "center",
					hug = true,
					ui.row {
						class = "resolved-tile",
						width = 16,
						height = 16,
						align = "center",
						justify = "center",
						ui.icon { name = "sparkles", size = "small" },
					},
					ui.text { `Resolved by {desk.assistant}`, weight = "medium", size = "small" },
				}
				else ui.row {
					class = "ticket-status",
					align = "center",
					ui.row { class = "ticket-dot", width = 6, height = 6 },
					ui.text { ticket.status_label },
				}
			table.insert(rows, ui.row {
				class = "ticket",
				key = string.lower(ticket.id),
				selected = ticket.id == state.selected,
				msg = { select = ticket.id },
				ui.row { class = { "ticket-edge", `edge-{string.lower(ticket.priority)}` }, width = 3 },
				ui.column {
					class = "ticket-body",
					grow = true,
					ui.row {
						align = "center",
						ui.text { ticket.id, class = "ticket-id" },
						ui.spacer(),
						ui.text { ticket.age, class = "ticket-id" },
					},
					ui.text { ticket.title, class = "ticket-title", truncate = true },
					ui.row {
						class = "ticket-meta",
						align = "center",
						avatar { name = ticket.requester.name, shape = "circle", size = "small" },
						ui.text { ticket.requester.name },
						ui.text { "·", class = "ticket-sub" },
						ui.text { ticket.kind, class = "ticket-sub" },
						ui.text { "·", class = "ticket-sub" },
						ui.icon { name = source.icon :: any, size = "small", style = { color = source.color }, tooltip = `From {source.name}` },
						ui.spacer(),
						status,
					},
				},
			})
		end

		return ui.column {
			grow = true,
			gap = "none",
			head,
			ui.column {
				grow = true,
				scroll = true,
				gap = "none",
				ui.column {
					gap = "none",
					shrink = false,
					ui.column {
						class = "week",
						ui.text { "This week", class = "week-label" },
						ui.row { class = "week-line", wrap = true, table.unpack(words) },
						ui.row { height = 14 },
						share_bar { shares = shares, width = 384, height = 5 },
						ui.row { class = "week-legend", align = "center", table.unpack(keys) },
					},
					#rows == 0 and ui.text { "No tickets match.", class = "list-empty" },
					table.unpack(rows),
				},
			},
		}
	end,
}
