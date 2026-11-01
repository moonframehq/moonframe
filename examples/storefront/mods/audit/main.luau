-- The page in front. For the audit log: a title and Export as CSV, filter
-- chips (kind of activity, author, period) and a search, then the
-- activities in a bordered table with a box per row, and the pages under
-- it. Other pages are stubs in this sample.

local table_view = use "table"
local filter_dropdown = use "filter-dropdown"
local pagination = use "pagination"
local rich_text = use "rich-text"
local avatar = use "avatar"
local empty_state = use "empty-state"

type Tab = { id: string, page: string, label: string, icon: string }

type State = {
	tab: Tab?,
	kinds: { string },
	author: string,
	period: string,
	query: string,
	page: number,
	per_page: number,
	checks: { [string]: boolean? },
	open: string?,
	newest: boolean,
	focus: boolean,
	exported: boolean,
}

local PERIODS: { any } = {
	{ id = "1", label = "Last 24 hours" },
	{ id = "7", label = "Last 7 days" },
	{ id = "30", label = "Last 30 days" },
	{ id = "all", label = "All time" },
}

-- The activities that pass the filters and the search, in order.
local function matching(state: State, store: any): { any }
	local kinds = {}
	for _, kind in state.kinds do
		kinds[kind] = true
	end
	local names = {}
	for _, author in store.authors do
		names[author.id] = author.name
	end
	local since = if state.period == "all" then -math.huge else store.now - (tonumber(state.period) or 7) * 86400
	local query = string.lower(state.query)
	local list = {}
	for _, activity in store.activities do
		if activity.time < since then
			continue
		end
		if #state.kinds > 0 and not kinds[activity.kind] then
			continue
		end
		if state.author ~= "" and activity.author ~= state.author then
			continue
		end
		if query ~= "" then
			local text = string.lower(`{activity.subject} {activity.verb} {activity.tag} {names[activity.author] or ""}`)
			if not string.find(text, query, 1, true) then
				continue
			end
		end
		table.insert(list, activity)
	end
	if state.newest then
		table.sort(list, function(a, b)
			return a.time > b.time
		end)
	end
	return list
end

local function page_of(list: { any }, page: number, per_page: number): { any }
	local first = (page - 1) * per_page + 1
	local rows = {}
	for index = first, math.min(first + per_page - 1, #list) do
		table.insert(rows, list[index])
	end
	return rows
end

-- Going back to the first page and closing menus after a filter changes.
local function refiltered(state: State)
	state.page = 1
	state.open = nil
end

local function stub(tab: Tab)
	return ui.column {
		class = "audit",
		grow = true,
		ui.column {
			gap = "none",
			ui.text { tab.label, class = "audit-title" },
			ui.text { "Only the audit log is built in this sample.", class = "audit-subtitle" },
		},
		ui.column {
			grow = true,
			justify = "center",
			empty_state {
				icon = tab.icon,
				title = tab.label,
				message = "Open System › Audit log in the sidebar to see the sample at work.",
				action = "Open the audit log",
				msg = "open-audit",
			},
		},
	}
end

return mod {
	region = "main",
	title = "Audit log",
	subscribe = { "store/tab" },

	styles = {
		[".audit"] = { padding = { 16, 18, 14, 18 }, gap = 0 },
		[".audit-title"] = { size = "title", weight = "medium" },
		[".audit-subtitle"] = { color = "muted-foreground", padding = { top = 4 } },
		[".audit-export"] = { radius = 7, border = true, border_color = "border", background = "background", padding = { 0, 10 }, gap = 6, weight = "medium", size = "small" },
		[".audit-export:hover"] = { background = "list-hover" },
		[".audit-saved"] = { size = "small", color = "muted-foreground" },
		[".audit-filters"] = { padding = { 16, 0, 20, 0 }, gap = 8 },
		[".audit-search"] = { radius = 7, border = true, border_color = "border", background = "background", padding = { 0, 10 }, gap = 8, color = "muted-foreground" },
		[".audit-search-input"] = { background = "#00000000", border = "none", shadow = "none", padding = { 5, 0 } },
		[".audit-foot"] = { padding = { top = 14 } },
		[".audit-author"] = { gap = 8 },
		[".audit-avatar"] = { radius = 5, color = "#1f1f22", weight = "medium" },
		[".audit-loading"] = { color = "muted-foreground" },
	},

	init = function(previous: State?): State
		local state: State = previous
			or {
				tab = nil,
				kinds = {},
				author = "",
				period = "7",
				query = "",
				page = 1,
				per_page = 15,
				checks = {},
				open = nil,
				newest = false,
				focus = false,
				exported = false,
			}
		-- Menus, focus and the last export's note don't outlive a launch.
		state.open = nil
		state.focus = false
		state.exported = false
		return state
	end,

	update = function(state: State, msg)
		state.focus = false
		if msg == "close" then
			state.open = nil
		elseif msg == "find" then
			state.focus = true
		elseif msg == "open-audit" then
			return state, { { to = "topbar", msg = { open = "audit" } } }
		elseif msg == "toggle-size" then
			state.open = if state.open == "size" then nil else "size"
		elseif type(msg) ~= "table" then
			return
		elseif msg.export then
			-- The view sends the rows ticked, else every row it shows.
			state.exported = true
			return state, { { to = "provider:store", msg = { export = msg.export } } }
		elseif msg.topic == "store/tab" then
			state.tab = msg.value
		elseif msg.open then
			state.open = if state.open == msg.open then nil else msg.open
		elseif msg.kind then
			-- Several kinds at once: a pick toggles one; "" is "Any".
			if msg.kind == "" then
				state.kinds = {}
				state.open = nil
			else
				local at = table.find(state.kinds, msg.kind)
				if at then
					table.remove(state.kinds, at)
				else
					table.insert(state.kinds, msg.kind)
				end
			end
			state.page = 1
		elseif msg.author then
			state.author = msg.author
			refiltered(state)
		elseif msg.period then
			state.period = msg.period
			refiltered(state)
		elseif msg.clear then
			if msg.clear == "kinds" then
				state.kinds = {}
			else
				state.author = ""
			end
			refiltered(state)
		elseif msg.msg == "query" then
			state.query = msg.value
			state.page = 1
		elseif msg.msg == "clear-query" then
			state.query = ""
			state.page = 1
		elseif msg.page then
			state.page = msg.page
		elseif msg.per_page then
			state.per_page = msg.per_page
			state.page = 1
			state.open = nil
		elseif msg.sort then
			state.newest = not state.newest
			state.page = 1
		elseif msg.check then
			state.checks[msg.check] = if state.checks[msg.check] then nil else true
		elseif msg.check_all then
			-- The page's boxes (their ids come with the message): all on,
			-- unless they all are already.
			local every = #msg.check_all > 0
			for _, id in msg.check_all do
				every = every and state.checks[id] == true
			end
			for _, id in msg.check_all do
				state.checks[id] = if every then nil else true
			end
		end
	end,

	view = function(state: State, app)
		local tab = state.tab
		if tab and tab.page ~= "audit" then
			return stub(tab)
		end
		local store = app.store
		if not store or not store.activities then
			return ui.column {
				class = "audit",
				grow = true,
				justify = "center",
				align = "center",
				ui.text { if store and store.reason then store.reason else "Loading the audit log…", class = "audit-loading" },
			}
		end

		local list = matching(state, store)
		local pages = math.max(math.ceil(#list / state.per_page), 1)
		local page = math.clamp(state.page, 1, pages)

		local authors = {}
		local author_options = {}
		for _, author in store.authors do
			authors[author.id] = author
			table.insert(author_options, { id = author.id, label = author.name })
		end
		local kind_options = {}
		for _, kind in store.kinds do
			table.insert(kind_options, { id = kind.id, label = kind.label, icon = kind.icon })
		end

		local rows = {}
		local ids = {}
		for _, activity in page_of(list, page, state.per_page) do
			local author = authors[activity.author] or { name = activity.author, color = "#cccccc" }
			table.insert(ids, activity.id)
			local sentence: { any } = {
				{ text = activity.subject, strong = true },
				{ text = activity.verb, muted = true },
				{ tag = activity.tag },
			}
			local stamp: { any } = { activity.date, { text = activity.clock, muted = true } }
			table.insert(rows, {
				id = activity.id,
				activity = rich_text { parts = sentence },
				author = ui.row {
					class = "audit-author",
					align = "center",
					avatar { name = author.name, size = "small", color = author.color, initial = string.sub(author.name, 1, 1) },
					ui.text { author.name, truncate = true },
				},
				time = rich_text { parts = stamp },
			})
		end

		-- Export the ticked rows, else every row the filters let through.
		local export_ids = {}
		for _, activity in list do
			if state.checks[activity.id] then
				table.insert(export_ids, activity.id)
			end
		end
		local ticked = #export_ids
		if ticked == 0 then
			for _, activity in list do
				table.insert(export_ids, activity.id)
			end
		end
		local export = store.export
		local saved = state.exported
			and export
			and ui.text {
				if export.ok then `Saved {export.count} to {export.path}` else export.error,
				class = "audit-saved",
				truncate = true,
			}

		return ui.column {
			class = "audit",
			grow = true,
			ui.row {
				align = "start",
				gap = "normal",
				ui.column {
					gap = "none",
					grow = true,
					ui.text { "Audit log", class = "audit-title" },
					ui.text { "A timeline of all your store activities.", class = "audit-subtitle" },
				},
				ui.row {
					gap = "normal",
					align = "center",
					saved,
					ui.row {
						class = "audit-export",
						height = 32,
						align = "center",
						msg = { export = export_ids },
						tooltip = if ticked > 0 then "Export the ticked rows" else "Export every row the filters show",
						ui.icon { name = "download" },
						ui.text { if ticked > 0 then `Export {ticked} as CSV` else "Export as CSV" },
					},
				},
			},
			ui.row {
				class = "audit-filters",
				align = "center",
				shrink = false,
				filter_dropdown {
					icon = "square-check",
					placeholder = "Any activities",
					any = "Any activities",
					multiple = true,
					options = kind_options,
					selected = state.kinds,
					event = "kind",
					open = state.open == "kinds",
					toggle = { open = "kinds" },
					dismiss = "close",
					clear = { clear = "kinds" },
					width = 200,
				},
				filter_dropdown {
					icon = "user",
					placeholder = "Any author",
					any = "Any author",
					options = author_options,
					selected = state.author,
					event = "author",
					open = state.open == "author",
					toggle = { open = "author" },
					dismiss = "close",
					clear = { clear = "author" },
					width = 200,
				},
				filter_dropdown {
					icon = "calendar",
					options = PERIODS,
					selected = state.period,
					plain = true,
					event = "period",
					open = state.open == "period",
					toggle = { open = "period" },
					dismiss = "close",
					width = 180,
				},
				ui.spacer(),
				ui.row {
					class = "audit-search",
					width = 290,
					height = 32,
					align = "center",
					ui.icon { name = "search" },
					ui.input {
						id = "audit-search",
						class = "audit-search-input",
						value = state.query,
						placeholder = "Search by activity name, author…",
						change = "query",
						keys = { escape = "clear-query" },
						focus = state.focus,
						grow = true,
					},
				},
			},
			table_view {
				bordered = true,
				checkable = true,
				checks = state.checks,
				check_all = { check_all = ids },
				event = "check",
				grow = true,
				sort = "time",
				descending = state.newest,
				empty = "No activity matches these filters.",
				columns = {
					{ key = "activity", title = "Activity", sortable = false },
					{ key = "author", title = "Author", width = 250, sortable = false },
					{ key = "time", title = "Timestamp", width = 250, sortable = true },
				},
				rows = rows,
			},
			ui.row {
				class = "audit-foot",
				shrink = false,
				pagination {
					page = page,
					total = #list,
					per_page = state.per_page,
					noun = if #list == 1 then "activity" else "activities",
					window = 5,
					ellipsis = false,
					sizes = { 10, 15, 25, 50 },
					size_open = state.open == "size",
					size_toggle = "toggle-size",
					size_dismiss = "close",
				},
			},
		}
	end,
}
