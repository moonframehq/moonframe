-- An inbox: category tabs with counts, then one row per message. Click a row
-- to open it (it is read from then on), click a star to flip it, and
-- right-click a row for its actions; "Move Sender To" opens a submenu that
-- moves the message to another category.
local mail_row = use "mail-row"
local segmented = use "segmented"
local context_menu = use "context-menu"

local CATEGORIES = { "primary", "updates", "promotions", "social" }
local TITLES: { [string]: string } = {
	primary = "Primary",
	updates = "Updates",
	promotions = "Promotions",
	social = "Social",
}

local function messages(): { any }
	return {
		{ id = "m1", category = "primary", from = "Alex Kim", subject = "Spring campaign proposal", preview = "Hi Sam, here's the proposal for the spring campaign we talked about.", time = "Friday", account = "Studio", waiting = true },
		{ id = "m2", category = "primary", from = "Lucía Romero", subject = "Cena del sábado", preview = "¡Hola! Hemos reservado una mesa para ocho a las nueve.", time = "9:58", account = "Personal", unread = true, reply = true },
		{ id = "m3", category = "primary", from = "Daniel Reyes", subject = "Q4 roadmap draft: comments by Friday", preview = "Hi Alex, the first draft of the Q4 roadmap is in the shared folder.", time = "9:41", account = "Studio", count = 2, starred = true, reply = true, attachment = true },
		{ id = "m4", category = "updates", from = "Northwind Air", subject = "Your flight to Lisbon is confirmed", preview = "Booking reference K7Q2LM. Outbound Friday 18:05.", time = "9:24", account = "Personal", unread = true },
		{ id = "m5", category = "promotions", from = "Pixel Kitchen", subject = "Your weekly recipes are here", preview = "Five dinners in thirty minutes or less, and a cake.", time = "9:12", account = "Personal", unread = true },
		{ id = "m6", category = "social", from = "Priya Shah", subject = "Photos from the weekend", preview = "Uploaded the album, there's one of you on the boat.", time = "Tue", account = "Family" },
	}
end

local function actions(category: string): { any }
	local moves = {}
	for _, id in CATEGORIES do
		table.insert(moves, { id = `move:{id}`, label = TITLES[id], checked = id == category })
	end
	return {
		{ id = "reply", label = "Reply", icon = "reply" },
		{ id = "star", label = "Star", icon = "star" },
		"separator",
		{ id = "move", label = "Move Sender To", icon = "inbox", items = moves },
		{ id = "needs-reply", label = "Mark as Needs Reply" },
		"separator",
		{ id = "archive", label = "Archive", icon = "archive" },
		{ id = "delete", label = "Delete", icon = "trash-2", danger = true },
	}
end

local function find(list: { any }, id: string)
	for _, message in list do
		if message.id == id then
			return message
		end
	end
	return nil
end

return mod {
	init = function(previous)
		return previous or { messages = messages(), category = "primary", open = "m2", menu = nil, expanded = nil }
	end,

	update = function(state, msg)
		if msg == "close-menu" then
			state.menu = nil
			state.expanded = nil
		elseif msg.select then
			state.category = msg.select
		elseif msg.open then
			state.open = msg.open
			local message = find(state.messages, msg.open)
			if message then
				message.unread = false
			end
		elseif msg.star then
			local message = find(state.messages, msg.star)
			if message then
				message.starred = not message.starred
			end
		elseif msg.menu then
			state.menu = msg.menu
			state.expanded = nil
		elseif msg.expand then
			state.expanded = if state.expanded == msg.expand then nil else msg.expand
		elseif msg.pick then
			local message = find(state.messages, state.menu or "")
			if message then
				local target = string.match(msg.pick, "^move:(.+)$")
				if target then
					message.category = target
				elseif msg.pick == "star" then
					message.starred = not message.starred
				elseif msg.pick == "needs-reply" then
					message.reply = true
				elseif msg.pick == "archive" or msg.pick == "delete" then
					table.remove(state.messages, table.find(state.messages, message) :: number)
				end
			end
			state.menu = nil
			state.expanded = nil
		end
	end,

	view = function(state)
		local counts = {}
		for _, message in state.messages do
			counts[message.category] = (counts[message.category] or 0) + 1
		end
		local options = {}
		for _, id in CATEGORIES do
			table.insert(options, { id = id, label = TITLES[id], count = counts[id] or 0 })
		end

		local rows = {}
		for _, m in state.messages do
			if m.category ~= state.category then
				continue
			end
			local tag
			if m.waiting then
				tag = { label = "No reply · 3 days", icon = "clock", tone = "warning" }
			elseif m.reply then
				tag = { label = "Needs Reply", icon = "reply", tone = "accent" }
			end
			table.insert(
				rows,
				context_menu {
					open = state.menu == m.id,
					dismiss = "close-menu",
					expanded = state.expanded,
					items = actions(m.category),
					width = 220,
					mail_row {
						from = m.from,
						subject = m.subject,
						preview = m.preview,
						time = m.time,
						account = m.account,
						unread = m.unread,
						count = m.count,
						starred = m.starred,
						attachment = m.attachment,
						tag = tag,
						selected = state.open == m.id,
						msg = { open = m.id },
						context = { menu = m.id },
						star = { star = m.id },
					},
				}
			)
		end

		return ui.column {
			gap = "normal",
			width = 460,
			segmented { options = options, selected = state.category, grow = true },
			ui.column { gap = "none", table.unpack(rows) },
			ui.row { height = 200 },
		}
	end,
}
