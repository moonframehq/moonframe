-- The mail, in two layouts the user switches between (View > Switch Layout,
-- or L):
--
--   split  the list as cards on the left, the open email on the right (or,
--          with none open, how many there are and the keys to use);
--   rows   one line per email across the whole card, grouped by day; an
--          email opens in its place, with a way back.
--
-- J and K move through the list (opening the next email when one is open),
-- Enter opens, U goes back, E marks done, C writes a new message. The tabs
-- over the list filter it. What is read, done or opened is this mod's
-- state; the mailbox itself is `app.mailbox` (SAMPLE DATA).

local key_hints = use "key-hints"
local avatar = use "avatar"
local icon_button = use "icon-button"
local menu = use "menu"
local dialog = use "dialog"

-- What update needs of each thread to move through the list and count
-- what is unread: kept from the provider's topic, never saved.
type Entry = { id: string, buckets: { string }, unread: boolean, text: string }

type State = {
	layout: string,
	filter: string,
	selected: string?,
	opened: string?,
	tabs: { string },
	read: { [string]: boolean },
	unread: { [string]: boolean },
	done: { [string]: boolean },
	expanded: { [string]: boolean },
	replies: { [string]: { string } },
	draft: string,
	query: string,
	searching: boolean,
	focus_search: boolean,
	menu: boolean,
	compose: boolean,
	index: { Entry },
}

local FILTERS = {
	{ id = "needs", label = "Needs you" },
	{ id = "vip", label = "VIP" },
	{ id = "sales", label = "Sales" },
	{ id = "investors", label = "Investors" },
	{ id = "fyi", label = "FYI" },
	{ id = "all", label = "All" },
}

local TAGS: { [string]: string } = {
	Finance = "#3b82f6",
	Sales = "#22c55e",
	Investors = "#6366f1",
	Team = "#14b8a6",
	Press = "#ec4899",
	Updates = "#a1a1aa",
	Newsletter = "#a78bfa",
	Receipts = "#f59e0b",
}

local MAX_TABS = 3

local function has(list: { string }, value: string): boolean
	for _, item in list do
		if item == value then
			return true
		end
	end
	return false
end

local function is_unread(state: State, id: string, unread: boolean): boolean
	return state.unread[id] == true or (unread and not state.read[id])
end

-- The threads the list shows, in order: in the filter, matching the query,
-- not done.
local function visible(state: State): { Entry }
	local query = string.lower(state.query)
	local out = {}
	for _, entry in state.index do
		if not state.done[entry.id]
			and (state.filter == "all" or has(entry.buckets, state.filter))
			and (query == "" or string.find(entry.text, query, 1, true) ~= nil)
		then
			table.insert(out, entry)
		end
	end
	return out
end

local function unread_in(state: State, bucket: string): number
	local count = 0
	for _, entry in state.index do
		if not state.done[entry.id] and (bucket == "all" or has(entry.buckets, bucket)) and is_unread(state, entry.id, entry.unread) then
			count += 1
		end
	end
	return count
end

-- What the sidebar and the title row hear: Mail's unread count, and the
-- emails open as tabs.
local function publish(state: State): { any }
	return {
		{ to = "topic:tidings/unread", msg = unread_in(state, "needs") },
		{ to = "topic:tidings/tabs", msg = { threads = state.tabs, current = state.opened or "" } },
	}
end

local function open(state: State, id: string)
	state.selected = id
	state.opened = id
	state.read[id] = true
	state.unread[id] = nil
	state.menu = false
	if not has(state.tabs, id) then
		table.insert(state.tabs, id)
		while #state.tabs > MAX_TABS do
			table.remove(state.tabs, 1)
		end
	end
end

-- Moves the selection by `by` through what the list shows; with an email
-- open, the next one opens in its place.
local function step(state: State, by: number)
	local list = visible(state)
	if #list == 0 then
		return
	end
	local at = 0
	for index, entry in list do
		if entry.id == state.selected then
			at = index
		end
	end
	local to = if at == 0 then 1 else math.clamp(at + by, 1, #list)
	if state.opened then
		open(state, list[to].id)
	else
		state.selected = list[to].id
	end
end

return mod {
	region = "main",
	title = "Mail",
	subscribe = { "provider/mailbox" },

	styles = {
		[".mail"] = { padding = { 0, 8, 8, 0 }, gap = 8 },
		[".mail-card"] = { padding = { 0, 0, 0, 0 } },
		[".mail-head"] = { padding = { 16, 14, 8, 20 }, gap = 6 },
		[".mail-title"] = { radius = 8, padding = { 2, 6, 2, 0 }, gap = 6 },
		[".mail-title:hover"] = { background = "list-hover" },
		[".mail-title-text"] = { size = "large", weight = "bold" },
		[".mail-search"] = { background = "secondary", radius = 9, padding = { 0, 10 }, gap = 8 },
		[".mail-search-field"] = { background = "#00000000", border = "none", shadow = "none", padding = { 5, 0 } },
		[".mail-filters-wrap"] = { padding = { 0, 18, 10, 18 } },
		[".mail-filters"] = { background = "secondary", radius = 18, padding = 3, gap = 0 },
		[".mail-filter"] = { radius = 15, padding = { 4, 9 }, gap = 5, color = "muted-foreground" },
		[".mail-filter:hover"] = { color = "foreground" },
		[".mail-filter:selected"] = {
			background = "background",
			color = "foreground",
			shadow = "0 1px 2px #00000014, 0 0 0 1px #0000000a",
		},
		[".mail-filter-count"] = { size = "small", color = "muted-foreground" },
		[".mail-list"] = { padding = { 0, 8, 12, 8 }, gap = 4 },

		-- Layout A: cards.
		[".mcard"] = { padding = { 12, 12, 14, 10 }, radius = 12, gap = 6, border = true, border_color = "#00000000" },
		[".mcard:hover"] = { background = "#f9f9fa" },
		[".mcard:selected"] = { background = "#f6f6f7", border_color = "border" },
		[".mcard-sender"] = { weight = "medium" },
		[".mcard-body"] = { padding = { 0, 0, 0, 44 }, gap = 6 },
		[".mcard-subject"] = { size = "large", weight = "medium" },
		[".mcard-preview"] = { color = "muted-foreground" },
		[".mcard-pills"] = { padding = { 4, 0, 0, 0 }, gap = 8 },

		-- Layout B: rows.
		[".mrow"] = { padding = { 0, 12, 0, 10 }, radius = 10, gap = 10, border = true, border_color = "#00000000" },
		[".mrow:hover"] = { background = "#f9f9fa" },
		[".mrow:selected"] = { background = "#f6f6f7", border_color = "border" },
		[".mrow-preview"] = { color = "muted-foreground" },
		[".mgroup"] = { padding = { 14, 0, 6, 34 }, size = "small", color = "muted-foreground" },

		-- Shared bits.
		[".dot"] = { background = "unread", radius = 4 },
		[".muted"] = { color = "muted-foreground" },
		[".small-muted"] = { color = "muted-foreground", size = "small" },
		[".pill"] = { background = "secondary", radius = 6, padding = { 3, 8 }, gap = 6, color = "ink" },
		[".pill:hover"] = { background = "#ececef" },
		[".pill-text"] = { size = "small", weight = "medium" },
		[".pill-dot"] = { radius = 3 },
		[".logo"] = { radius = 999, color = "#ffffff" },

		-- The open email.
		[".read"] = { padding = { 18, 24, 16, 24 }, gap = 12 },
		[".read-subject"] = { size = "title", weight = "bold" },
		[".msg"] = { padding = { 12, 0 }, gap = 8 },
		[".msg-folded"] = { padding = { 10, 8 }, radius = 10, gap = 10 },
		[".msg-folded:hover"] = { background = "list-hover" },
		[".msg-body"] = { padding = { 0, 0, 0, 34 }, gap = 10 },
		[".reply"] = { border = true, border_color = "border", radius = 12, padding = 10, gap = 8 },
		[".reply-field"] = { background = "#00000000", border = "none", shadow = "none", padding = { 2, 2 } },
		[".send"] = { background = "foreground", color = "background", radius = 8, padding = { 5, 12 }, gap = 6 },
		[".empty-icon"] = { color = "#d4d4d8" },
		[".empty-text"] = { size = "large", color = "muted-foreground" },
		[".compose-field"] = { border = "bottom", border_color = "border", radius = 0, background = "#00000000", padding = { 8, 2 } },
		[".compose-body"] = { background = "#00000000", border = "none", padding = { 8, 2 } },
	},

	init = function(previous: State?): (State, { any })
		local state: State = {
			layout = "split",
			filter = "needs",
			selected = "iris-thursday",
			opened = nil,
			tabs = { "felix-seats" },
			read = {},
			unread = {},
			done = {},
			expanded = {},
			replies = {},
			draft = "",
			query = "",
			searching = false,
			focus_search = false,
			menu = false,
			compose = false,
			index = {},
		}
		if previous then
			-- Keep what the user chose and did; drop what is passing.
			for _, key in { "layout", "filter", "selected", "opened", "tabs", "read", "unread", "done", "replies" } do
				if (previous :: any)[key] ~= nil then
					(state :: any)[key] = (previous :: any)[key]
				end
			end
		end
		return state, publish(state)
	end,

	update = function(state: State, msg: any)
		state.focus_search = false
		if type(msg) == "table" and msg.topic == "provider/mailbox" then
			local index: { Entry } = {}
			for _, thread in (msg.value and msg.value.threads) or {} do
				table.insert(index, {
					id = thread.id,
					buckets = thread.buckets,
					unread = thread.unread == true,
					text = string.lower(`{thread.sender} {thread.subject} {thread.preview} {thread.tag}`),
				})
			end
			state.index = index
		elseif msg == "next" then
			step(state, 1)
		elseif msg == "previous" then
			step(state, -1)
		elseif msg == "open" then
			if state.selected then
				open(state, state.selected)
			end
		elseif msg == "back" then
			state.opened = nil
		elseif msg == "done" then
			local id = state.opened or state.selected
			if id then
				local was_open = state.opened ~= nil
				step(state, 1)
				if state.selected == id then
					step(state, -1)
				end
				state.done[id] = true
				if state.selected == id then
					state.selected = nil
					state.opened = nil
				elseif not was_open then
					state.opened = nil
				end
			end
		elseif msg == "unread" then
			local id = state.opened or state.selected
			if id then
				state.unread[id] = true
				state.read[id] = nil
				state.opened = nil
			end
		elseif msg == "compose" then
			state.compose = true
		elseif msg == "close-compose" or msg == "send-compose" then
			state.compose = false
		elseif msg == "search" then
			state.searching = true
			state.focus_search = true
		elseif type(msg) == "table" and msg.msg == "query" then
			state.query = msg.value
			state.searching = true
		elseif type(msg) == "table" and msg.msg == "clear-search" then
			state.query = ""
			state.searching = false
		elseif type(msg) == "table" and msg.msg == "draft" then
			state.draft = msg.value
		elseif msg == "send" or (type(msg) == "table" and msg.msg == "send") then
			local text = if type(msg) == "table" then msg.value else state.draft
			if state.opened and text and string.find(text, "%S") then
				local list = state.replies[state.opened] or {}
				table.insert(list, text)
				state.replies[state.opened] = list
				state.draft = ""
			end
		elseif msg == "toggle-menu" then
			state.menu = not state.menu
		elseif msg == "close-menu" then
			state.menu = false
		elseif msg == "switch-layout" then
			state.layout = if state.layout == "split" then "rows" else "split"
			state.menu = false
		elseif type(msg) == "table" and msg.layout then
			state.layout = msg.layout
			state.menu = false
		elseif type(msg) == "table" and msg.pick then
			-- From the title's menu.
			if msg.pick == "split" or msg.pick == "rows" then
				state.layout = msg.pick
			elseif msg.pick == "read-all" then
				for _, entry in visible(state) do
					state.read[entry.id] = true
					state.unread[entry.id] = nil
				end
			elseif msg.pick == "restore" then
				state.done = {}
			end
			state.menu = false
		elseif type(msg) == "table" and msg.filter then
			state.filter = msg.filter
			local list = visible(state)
			if state.layout == "rows" then
				state.opened = nil
			end
			local kept = false
			for _, entry in list do
				kept = kept or entry.id == state.selected
			end
			if #list > 0 and not kept then
				state.selected = list[1].id
			end
		elseif type(msg) == "table" and msg.pick_thread then
			open(state, msg.pick_thread)
		elseif type(msg) == "table" and msg.close_tab then
			for index, id in state.tabs do
				if id == msg.close_tab then
					table.remove(state.tabs, index)
					break
				end
			end
			if state.opened == msg.close_tab then
				state.opened = nil
			end
		elseif type(msg) == "table" and msg.home then
			state.opened = nil
		elseif type(msg) == "table" and msg.expand then
			state.expanded[msg.expand] = not state.expanded[msg.expand]
		else
			return state
		end
		return state, publish(state)
	end,

	view = function(state: State, app)
		local mailbox = app.mailbox
		local threads: { [string]: any } = {}
		for _, thread in (mailbox and mailbox.threads) or {} do
			threads[thread.id] = thread
		end
		local list = visible(state)
		local me = mailbox and mailbox.account or { name = "Robin Hale", email = "robin@fernhill.studio" }

		local function pill(label: string, color: string, msg: any?): any
			return ui.row {
				class = "pill",
				hug = true,
				align = "center",
				msg = msg,
				ui.row { class = "pill-dot", width = 6, height = 6, style = { background = color } },
				ui.text { label, class = "pill-text" },
			}
		end

		local function face(thread: any, size: string?): any
			if thread.logo then
				local side = if size == "normal" then 24 else 18
				return ui.row {
					class = "logo",
					width = side,
					height = side,
					shrink = false,
					align = "center",
					justify = "center",
					style = { background = thread.color },
					ui.text { (string.match(thread.sender, "^" .. utf8.charpattern) or ""), size = "small", weight = "bold" },
				}
			end
			return avatar { name = thread.sender, shape = "circle", size = size or "small", color = thread.color }
		end

		local function dot(on: boolean): any
			return ui.row {
				width = 8,
				shrink = false,
				justify = "center",
				on and ui.row { class = "dot", width = 6, height = 6 },
			}
		end

		local function count_of(thread: any): number
			return #thread.messages + #(state.replies[thread.id] or {})
		end

		-- Layout A: one card per email.
		local function card(thread: any): any
			local unread = is_unread(state, thread.id, thread.unread)
			local count = count_of(thread)
			return ui.column {
				class = "mcard",
				key = thread.id,
				selected = state.selected == thread.id,
				msg = { pick_thread = thread.id },
				ui.row {
					align = "center",
					gap = "normal",
					dot(unread),
					face(thread),
					ui.text { thread.sender, class = "mcard-sender" },
					count > 1 and ui.text { `({count})`, class = "muted" },
					ui.text { "·", class = "muted" },
					ui.text { thread.domain, class = "muted", truncate = true },
					ui.spacer(),
					ui.text { thread.time, class = "small-muted" },
				},
				ui.column {
					class = "mcard-body",
					ui.text { thread.subject, class = "mcard-subject", truncate = true },
					ui.text { thread.preview, class = "mcard-preview", lines = 2 },
					ui.row {
						class = "mcard-pills",
						align = "center",
						pill(thread.tag, TAGS[thread.tag] or "#a1a1aa"),
						thread.action and pill(thread.action, "#f59e0b"),
					},
				},
			}
		end

		-- Layout B: one line per email.
		local function row(thread: any): any
			local unread = is_unread(state, thread.id, thread.unread)
			local count = count_of(thread)
			return ui.row {
				class = "mrow",
				key = thread.id,
				height = 40,
				align = "center",
				selected = state.selected == thread.id,
				msg = { pick_thread = thread.id },
				dot(unread),
				face(thread),
				ui.row {
					width = 186,
					shrink = false,
					align = "center",
					gap = "normal",
					ui.text { thread.sender, weight = if unread then "bold" else "normal", truncate = true },
					count > 1 and ui.text { `({count})`, class = "small-muted" },
				},
				ui.row { shrink = false, pill(thread.tag, TAGS[thread.tag] or "#a1a1aa") },
				ui.row {
					grow = true,
					align = "center",
					gap = "normal",
					ui.row { shrink = false, ui.text { thread.subject, weight = if unread then "bold" else "medium" } },
					ui.text { thread.preview, class = "mrow-preview", truncate = true },
				},
				thread.action and ui.row { shrink = false, pill(thread.action, "#f59e0b") },
				ui.row { width = 40, shrink = false, justify = "end", ui.text { thread.time, class = "small-muted" } },
			}
		end

		-- The open email: subject, labels, messages (all but the last folded
		-- until clicked) and a reply box.
		local function reading(thread: any, back: boolean): any
			local messages: { any } = {}
			for _, message in thread.messages do
				table.insert(messages, message)
			end
			local replies: { string } = state.replies[thread.id] or {}
			for _, text in replies do
				table.insert(messages, { from = "me", time = "Now", body = text })
			end
			local items: { any } = {}
			for index, message in messages do
				local key = `{thread.id}:{index}`
				local mine = message.from == "me"
				local name = if mine then me.name else message.from
				local who = if mine then { sender = name, color = "#3f3f46" } else thread
				local open_one = index == #messages or state.expanded[key]
				if open_one then
					local paragraphs: { any } = {}
					for paragraph in string.gmatch(message.body .. "\n\n", "(.-)\n\n") do
						if paragraph ~= "" then
							table.insert(paragraphs, ui.text { paragraph, wrap = true })
						end
					end
					table.insert(items, ui.column {
						class = "msg",
						ui.row {
							align = "center",
							gap = "normal",
							msg = if index < #messages then { expand = key } else nil,
							face(who, "normal"),
							ui.text { name, weight = "medium" },
							ui.text { if mine then me.email else thread.domain, class = "small-muted" },
							ui.spacer(),
							ui.text { message.time, class = "small-muted" },
						},
						ui.column { class = "msg-body", table.unpack(paragraphs) },
					})
				else
					table.insert(items, ui.row {
						class = "msg-folded",
						align = "center",
						msg = { expand = key },
						face(who, "normal"),
						ui.text { name, weight = "medium" },
						ui.row { grow = true, ui.text { (string.gsub(message.body, "\n+", " ")), class = "muted", truncate = true } },
						ui.text { message.time, class = "small-muted" },
					})
				end
				if index < #messages then
					table.insert(items, ui.divider())
				end
			end
			local first = string.match(thread.sender, "^%S+") or thread.sender
			return ui.column {
				class = "read",
				grow = true,
				ui.row {
					align = "center",
					gap = "normal",
					back and icon_button { icon = "arrow-left", tooltip = "Back to the list (U)", msg = "back" },
					ui.text { thread.subject, class = "read-subject", truncate = true },
					ui.spacer(),
					icon_button { icon = "archive", tooltip = "Mark as done (E)", msg = "done" },
					icon_button { icon = "mail", tooltip = "Mark as unread (⇧U)", msg = "unread" },
					not back and icon_button { icon = "close", tooltip = "Close (U)", msg = "back" },
				},
				ui.row {
					align = "center",
					gap = "normal",
					pill(thread.tag, TAGS[thread.tag] or "#a1a1aa"),
					thread.action and pill(thread.action, "#f59e0b"),
					ui.text { `{#messages} {if #messages == 1 then "message" else "messages"}`, class = "small-muted" },
				},
				ui.divider(),
				ui.column {
					grow = true,
					scroll = true,
					gap = "none",
					ui.column { gap = "none", shrink = false, table.unpack(items) },
				},
				ui.column {
					class = "reply",
					ui.input {
						id = "reply",
						value = state.draft,
						placeholder = `Reply to {first}…`,
						multiline = true,
						rows = 3,
						change = "draft",
						submit = "send",
						class = "reply-field",
					},
					ui.row {
						align = "center",
						gap = "normal",
						icon_button { icon = "paperclip", tooltip = "Attach (not in this example)", size = "small", msg = "noop" },
						ui.text { "⌘↩ to send", class = "small-muted" },
						ui.spacer(),
						ui.row {
							class = "send",
							align = "center",
							msg = "send",
							ui.icon { name = "send", size = "small" },
							ui.text { "Send", weight = "medium" },
						},
					},
				},
			}
		end

		-- The card's head: the title with its menu, compose, search, then the
		-- filter tabs with their unread counts.
		local tabs = {}
		for _, filter in FILTERS do
			local count = if filter.id == "all" then 0 else unread_in(state, filter.id)
			table.insert(tabs, ui.row {
				class = "mail-filter",
				align = "center",
				selected = state.filter == filter.id,
				msg = { filter = filter.id },
				ui.text { filter.label },
				count > 0 and ui.text { tostring(count), class = "mail-filter-count" },
			})
		end
		local layout_items: { any } = {
			{ heading = "Layout" },
			{ id = "split", label = "List beside the email", icon = "columns-2", checked = state.layout == "split", shortcut = "cmd-1" },
			{ id = "rows", label = "One line per email", icon = "rows-2", checked = state.layout == "rows", shortcut = "cmd-2" },
			"separator",
			{ id = "read-all", label = "Mark all as read", icon = "check", checked = false },
			{ id = "restore", label = "Bring back done", icon = "undo", checked = false },
		}
		local head = ui.column {
			gap = "none",
			shrink = false,
			ui.row {
				class = "mail-head",
				align = "center",
				ui.popover {
					open = state.menu,
					dismiss = "close-menu",
					side = "bottom",
					align = "start",
					width = 240,
					content = menu {
						items = layout_items,
					},
					ui.row {
						class = "mail-title",
						align = "center",
						msg = "toggle-menu",
						tooltip = "Layout and more",
						ui.text { "Mail", class = "mail-title-text" },
						ui.icon { name = "chevron-down", size = "small", tone = "muted" },
					},
				},
				ui.spacer(),
				(state.searching or state.query ~= "") and ui.row {
					class = "mail-search",
					width = 220,
					height = 30,
					align = "center",
					ui.icon { name = "search", size = "small", tone = "muted" },
					ui.input {
						id = "search",
						value = state.query,
						placeholder = "Search mail",
						change = "query",
						keys = { escape = "clear-search" },
						focus = state.focus_search,
						grow = true,
						class = "mail-search-field",
					},
				},
				icon_button {
					icon = if state.layout == "split" then "rows-2" else "columns-2",
					tooltip = if state.layout == "split" then "One line per email (L)" else "List beside the email (L)",
					msg = "switch-layout",
				},
				icon_button { icon = "square-pen", tooltip = "New message (C)", msg = "compose" },
				not (state.searching or state.query ~= "") and icon_button { icon = "search", tooltip = "Search (/)", msg = "search" },
			},
			ui.row {
				class = "mail-filters-wrap",
				ui.row { class = "mail-filters", hug = true, align = "center", table.unpack(tabs) },
			},
		}

		local compose = dialog {
			open = state.compose,
			dismiss = "close-compose",
			submit = "send-compose",
			title = "New message",
			icon = "square-pen",
			size = "large",
			buttons = {
				{ label = "Discard", msg = "close-compose", variant = "ghost" },
				{ label = "Send", msg = "send-compose", variant = "primary" },
			},
			ui.input { id = "compose-to", placeholder = "To", class = "compose-field", focus = state.compose },
			ui.input { id = "compose-subject", placeholder = "Subject", class = "compose-field" },
			ui.input { id = "compose-body", placeholder = "Write your message…", multiline = true, rows = 8, class = "compose-body" },
		}

		local opened = state.opened and threads[state.opened]

		if state.layout == "rows" then
			local body
			if opened then
				body = reading(opened, true)
			else
				local rows: { any } = {}
				local group = nil
				for _, entry in list do
					local thread = threads[entry.id]
					if thread then
						if thread.group ~= group then
							group = thread.group
							local label = group
							for _, known in (mailbox and mailbox.groups) or {} do
								if known.id == group then
									label = known.label
								end
							end
							table.insert(rows, ui.text { label, class = "mgroup" })
						end
						table.insert(rows, row(thread))
					end
				end
				body = ui.column {
					grow = true,
					scroll = true,
					gap = "none",
					ui.column { class = "mail-list", gap = "none", shrink = false, table.unpack(rows) },
				}
			end
			return ui.column {
				class = "mail",
				grow = true,
				ui.column { class = { "card", "mail-card" }, grow = true, gap = "none", head, body },
				compose,
			}
		end

		local cards: { any } = {}
		for _, entry in list do
			local thread = threads[entry.id]
			if thread then
				table.insert(cards, card(thread))
			end
		end
		local unread = 0
		for _, entry in list do
			if is_unread(state, entry.id, entry.unread) then
				unread += 1
			end
		end
		return ui.row {
			class = "mail",
			grow = true,
			ui.column {
				class = { "card", "mail-card" },
				width = 400,
				shrink = false,
				gap = "none",
				head,
				ui.column {
					grow = true,
					scroll = true,
					gap = "none",
					ui.column { class = "mail-list", shrink = false, table.unpack(cards) },
				},
			},
			ui.column {
				class = "card",
				grow = true,
				if opened
					then reading(opened, false)
					else ui.column {
						grow = true,
						align = "center",
						justify = "center",
						gap = "large",
						ui.icon { name = "mail", size = "title", class = "empty-icon" },
						ui.text { `{#list} {if #list == 1 then "thread" else "threads"}, {unread} unread`, class = "empty-text" },
						key_hints {
							caps = true,
							separator = "",
							hints = {
								{ keys = { "J", "K" } :: any, label = "to move" },
								{ keys = "enter", label = "to open" },
								{ keys = { "C" }, label = "to compose" },
							},
						},
					},
			},
			compose,
		}
	end,
}
