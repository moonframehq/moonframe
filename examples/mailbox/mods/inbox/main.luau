-- The chosen folder's messages: a header with its name and count, then one
-- row per message (sender, subject over a preview, time). Sample data.

type State = { folder: string, open: string? }

local NAMES: { [string]: string } = {
	compose = "Compose",
	search = "Search",
	calendar = "Calendar",
	automation = "Automation",
	inbox = "Inbox",
	starred = "Starred",
	scheduled = "Scheduled",
	drafts = "Drafts",
	sent = "Sent",
	spam = "Spam",
	trash = "Trash",
	settings = "Settings",
	support = "Support & Feedback",
}

local MESSAGES: { any } = {
	{ id = "m1", from = "Maya Okafor", subject = "Offsite agenda, final draft", preview = "Moved the design review to Thursday morning so the eng folks can join after standup.", time = "9:41", unread = true },
	{ id = "m2", from = "Linear", subject = "3 issues assigned to you", preview = "MAIL-212 Sidebar collapse animation, MAIL-218 Labels lose their colour in dark mode…", time = "9:12", unread = true },
	{ id = "m3", from = "Theo Brandt", subject = "Re: icons that stay in place", preview = "Agreed, the rail should never shift. 16 / 8 / 16 and 32 px rows felt right in the prototype.", time = "8:57" },
	{ id = "m4", from = "Stripe", subject = "Your October invoice", preview = "Invoice #4471 for $1,284.00 is available. It will be charged to the card ending 4242.", time = "Yesterday" },
	{ id = "m5", from = "Priya Shah", subject = "Type scale for the settings page", preview = "Attached the 13/14/15 comparison. I lean towards 14 for rows with 13 for section headings.", time = "Yesterday" },
	{ id = "m6", from = "GitHub", subject = "[moonframe] PR #12 ready for review", preview = "Stagger the sidebar fade: delays per item, a clip on the column, two new icons.", time = "Tue" },
	{ id = "m7", from = "Jonas Weber", subject = "Lunch on Friday?", preview = "The new ramen place on 5th opened this week, I can book for four at 12:30.", time = "Mon" },
}

return mod {
	title = "Inbox",
	subscribe = { "mail/folder" },

	styles = {
		[".inbox-head"] = { padding = { 18, 24, 12, 24 }, gap = 10 },
		[".inbox-title"] = { size = "large", weight = "medium" },
		[".inbox-count"] = { color = "muted-foreground" },
		[".inbox-row"] = { padding = { 12, 24 }, gap = 14, border = "top", border_color = "border" },
		[".inbox-row:hover"] = { background = "list-hover" },
		[".inbox-row:selected"] = { background = "accent/8" },
		[".inbox-from"] = { weight = "medium" },
		[".inbox-read"] = { color = "muted-foreground" },
		[".inbox-subject"] = { weight = "medium" },
		[".inbox-preview"] = { color = "muted-foreground" },
		[".inbox-time"] = { color = "muted-foreground", size = "small" },
		[".inbox-unread"] = { background = "accent", radius = 999 },
	},

	init = function(previous: State?): State
		return previous or { folder = "inbox" }
	end,

	update = function(state: State, msg)
		if typeof(msg) == "table" and msg.topic == "mail/folder" then
			state.folder = msg.value
		elseif typeof(msg) == "table" and msg.open then
			state.open = msg.open
		end
		return state
	end,

	view = function(state: State)
		local rows = {}
		for _, message in MESSAGES do
			table.insert(rows, ui.row {
				class = "inbox-row",
				selected = state.open == message.id,
				align = "start",
				msg = { open = message.id },
				ui.row {
					width = 8,
					height = 18,
					align = "center",
					shrink = false,
					message.unread and ui.row { class = "inbox-unread", width = 7, height = 7 },
				},
				ui.row {
					width = 150,
					shrink = false,
					ui.text { message.from, class = if message.unread then "inbox-from" else "inbox-read", truncate = true },
				},
				ui.column {
					grow = true,
					gap = "none",
					ui.text { message.subject, class = "inbox-subject", truncate = true },
					ui.text { message.preview, class = "inbox-preview", truncate = true },
				},
				ui.row { width = 72, justify = "end", shrink = false, ui.text { message.time, class = "inbox-time" } },
			})
		end
		return ui.column {
			grow = true,
			gap = "none",
			ui.row {
				class = "inbox-head",
				align = "center",
				ui.text { NAMES[state.folder] or "Label", class = "inbox-title" },
				ui.text { `{#MESSAGES} conversations`, class = "inbox-count" },
			},
			ui.column { grow = true, scroll = true, gap = "none", table.unpack(rows) },
		}
	end,
}
