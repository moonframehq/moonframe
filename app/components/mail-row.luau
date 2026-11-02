-- One message in a mailbox: the sender's avatar (with a dot while unread),
-- the sender, the thread's size, a star and the time on the first line, a
-- status pill and the subject on the second, a line of the preview, and the
-- account it came to. The chosen row shows an accent bar on its left edge.

local avatar = use "avatar"
local pill = use "pill"

type Tag = { label: string, icon: string?, tone: string?, color: string? }

type Props = {
	from: string,
	subject: string?,
	preview: string?,
	time: string?,
	account: string?,
	unread: boolean?,
	count: number?,
	starred: boolean?,
	star: any,
	attachment: boolean?,
	tag: Tag?,
	avatar: string?,
	color: string?,
	selected: boolean?,
	msg: any,
	context: any,
	to: string?,
}

return component {
	title = "Mail Row",
	category = "Lists",
	icon = "mail",
	description = "A message in a mailbox: avatar with an unread dot, sender, thread count, star and time, a status pill before the subject, a preview line and the account; the chosen one has an accent bar.",
	when = "Each row of an inbox or a mailbox's message list; stack them in a `ui.list` (rows of one height) or a scrolling column, under `segmented` category tabs. For tasks use `task-row`; for pull requests use `pr-row`; for a plain selectable row use `list`; for the open message's header use `message-header`.",
	props = {
		["from"] = "the sender's name; the avatar's initials and colour come from it",
		["subject"] = "the subject, cut to one line",
		["preview"] = "the start of the message, muted, cut to one line",
		["time"] = "when it came, as shown (\"9:41\", \"Friday\")",
		["account"] = "the account or mailbox it came to, small and muted under the preview",
		["unread"] = "a dot on the avatar and the sender and subject in bold",
		["count"] = "messages in the thread, after the sender (hidden under 2)",
		["starred"] = "a star before the time, in the warning colour",
		["star"] = "message a click on the star sends; with it, an unstarred row shows a muted star to click",
		["attachment"] = "a paperclip before the time",
		["tag"] = "a status pill before the subject: { label, icon?, tone? (accent, warning, danger, success), color? }, such as \"Needs Reply\" or \"No reply · 3 days\"",
		["avatar"] = "an image file for the avatar instead of the initials",
		["color"] = "the avatar's colour instead of the one picked from the name",
		["selected"] = "the open message: tinted, with an accent bar",
		["msg"] = "message a click sends (open it)",
		["context"] = "message a right-click sends (a `context-menu` of actions)",
		["to"] = "mod to send its messages to, if not the drawing one",
	},
	styles = {
		[".mail-row"] = { padding = { 12, 16, 12, 0 }, gap = 12, border = "bottom", border_color = "border" },
		[".mail-row:hover"] = { background = "list-hover" },
		[".mail-row:selected"] = { background = "accent/8" },
		[".mail-row-bar"] = { radius = 2 },
		[".mail-row-bar-on"] = { background = "accent" },
		[".mail-row-body"] = { gap = 2 },
		[".mail-row-line"] = { gap = 6 },
		[".mail-row-from"] = { weight = "medium" },
		[".mail-row-from-unread"] = { weight = "bold" },
		[".mail-row-count"] = { color = "muted-foreground", size = "small" },
		[".mail-row-time"] = { color = "muted-foreground", size = "small" },
		[".mail-row-subject-unread"] = { weight = "semibold" },
		[".mail-row-preview"] = { color = "muted-foreground" },
		[".mail-row-account"] = { color = "muted-foreground", size = "small" },
		[".mail-row-dot"] = { background = "accent", radius = 999, border = "all", border_color = "background" },
		[".mail-row-star"] = { color = "muted-foreground" },
		[".mail-row-starred"] = { color = "warning" },
		[".mail-row-clip"] = { color = "muted-foreground" },
	},
	example = {
		from = "Lucía Romero",
		subject = "Cena del sábado",
		preview = "¡Hola! Hemos reservado una mesa para ocho a las nueve.",
		time = "9:58",
		account = "Personal",
		unread = true,
		count = 2,
		starred = true,
		tag = { label = "Needs Reply", icon = "reply", tone = "accent" },
		selected = true,
	},
	usage = [[local mail_row = use "mail-row"

ui.list { count = #state.messages, row_height = 104, row = function(i)
	local m = state.messages[i]
	return mail_row {
		from = m.from, subject = m.subject, preview = m.preview, time = m.time,
		account = m.account, unread = m.unread, count = m.count, starred = m.starred,
		tag = m.waiting and { label = "No reply · 3 days", icon = "clock", tone = "warning" },
		selected = state.open == m.id, msg = { open = m.id }, context = { menu = m.id },
		star = { star = m.id },
	}
end }
-- update: msg.open opens a message; msg.star flips its star; msg.menu opens
-- a context menu for it.]],

	build = function(props: Props)
		local unread = props.unread == true
		local face = ui.row {
			gap = "none",
			shrink = false,
			avatar {
				name = props.from,
				letters = 2,
				src = props.avatar,
				color = props.color,
				shape = "circle",
				size = "large",
			},
			unread and ui.row {
				class = "mail-row-dot",
				width = 10,
				height = 10,
				float = { top = -2, left = -2 },
			},
		}

		-- The star: in colour when starred; a muted one to click when the
		-- row can be starred.
		local star = if props.starred or props.star ~= nil
			then ui.row {
			shrink = false,
			msg = props.star,
			to = props.to,
			tooltip = if props.starred then "Starred" else "Star",
			ui.icon {
				name = "star",
				size = "small",
				class = if props.starred then "mail-row-starred" else "mail-row-star",
			},
		}
			else nil

		local first = ui.row {
			class = "mail-row-line",
			align = "center",
			ui.text {
				props.from,
				class = if unread then { "mail-row-from", "mail-row-from-unread" } else "mail-row-from",
				truncate = true,
			},
			props.count and props.count > 1 and ui.text { tostring(props.count), class = "mail-row-count" },
			ui.spacer(),
			props.attachment and ui.icon { name = "paperclip", size = "small", class = "mail-row-clip" },
			star,
			props.time and ui.text { props.time, class = "mail-row-time" },
		}

		local tag = props.tag
		local second = (props.subject or tag) and ui.row {
			class = "mail-row-line",
			align = "center",
			tag and pill { label = tag.label, icon = tag.icon, tone = tag.tone or "accent", color = tag.color },
			props.subject and ui.text {
				props.subject,
				class = if unread then "mail-row-subject-unread" else nil,
				truncate = true,
			},
		}

		return ui.row {
			class = "mail-row",
			align = "start",
			selected = props.selected,
			msg = props.msg,
			context = props.context,
			to = props.to,
			ui.row {
				class = if props.selected then { "mail-row-bar", "mail-row-bar-on" } else "mail-row-bar",
				width = 3,
				self_align = "stretch",
				shrink = false,
			},
			face,
			ui.column {
				class = "mail-row-body",
				grow = true,
				first,
				second,
				props.preview and ui.text { props.preview, class = "mail-row-preview", truncate = true },
				props.account and ui.text { props.account, class = "mail-row-account", truncate = true },
			},
		}
	end,
}
