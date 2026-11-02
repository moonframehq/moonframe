-- The top of an open message: its subject large, then the sender's avatar,
-- name and address (a chip that copies it), who it went to, and the date
-- with actions on the right. A line under it parts it from the body.

local avatar = use "avatar"

type Props = {
	subject: string?,
	from: string,
	email: string?,
	recipients: any,
	cc: any,
	date: string?,
	avatar: string?,
	color: string?,
	actions: { any }?,
	sender: any,
	to: string?,
	[number]: any,
}

-- A list of names as one line ("Sam Lee, Priya Shah"); a string as it is.
local function names(value: any): string?
	if type(value) == "table" then
		return if #value > 0 then table.concat(value, ", ") else nil
	end
	return value
end

-- The children in order, skipping the `nil` and `false` a condition leaves.
local function positional(props: any): { any }
	local keys = {}
	for key, child in props do
		if type(key) == "number" and child then
			table.insert(keys, key)
		end
	end
	table.sort(keys)
	local list = table.create(#keys)
	for _, key in keys do
		table.insert(list, props[key])
	end
	return list
end

return component {
	title = "Message Header",
	category = "Data",
	icon = "mail-open",
	description = "An open message's header: the subject large, the sender's avatar, name and a copyable address chip, To and Cc lines, and the date with actions.",
	when = "Above the body of an email or a thread's message in a reading pane. For a row in the message list use `mail-row`; for a comment in a review use `comment-card`; for a chat turn use `chat-message`.",
	props = {
		["[children]"] = "anything under the header, before its line: a follow-up `banner`, a Translate button",
		["subject"] = "the subject, as the pane's title; none for a later message in a thread",
		["from"] = "the sender's name; the avatar's initials and colour come from it",
		["email"] = "the sender's address, in a chip that copies it on a click",
		["recipients"] = "who it went to: a name, or a list of names (\"To: Sam Lee\")",
		["cc"] = "copied to: a name or a list of names",
		["date"] = "when it was sent, as shown (\"Oct 16, 2026 at 4:12 PM\")",
		["avatar"] = "an image file for the avatar instead of the initials",
		["color"] = "the avatar's colour instead of the one picked from the name",
		["actions"] = "elements after the date (icon buttons: show images, more)",
		["sender"] = "message a click on the sender's name sends (open the contact)",
		["to"] = "mod to send `sender` to, if not the drawing one",
	},
	styles = {
		[".message-header"] = { gap = 14, padding = { 0, 0, 14, 0 }, border = "bottom", border_color = "border" },
		[".message-header-subject"] = { size = "title", weight = "bold" },
		[".message-header-from"] = { gap = 12 },
		[".message-header-name"] = { weight = "semibold" },
		[".message-header-name:hover"] = { color = "accent" },
		[".message-header-email"] = { background = "muted", radius = 999, padding = { 2, 10 }, color = "muted-foreground", size = "small" },
		[".message-header-email:hover"] = { color = "foreground" },
		[".message-header-to"] = { color = "muted-foreground", size = "small" },
		[".message-header-date"] = { color = "muted-foreground", size = "small" },
	},
	example = {
		subject = "Spring campaign proposal",
		from = "Alex Kim",
		email = "alex@studio.example",
		recipients = "Sam Lee",
		date = "Oct 16, 2026 at 4:12 PM",
	},
	usage = [[local message_header = use "message-header"
local icon_button = use "icon-button"

message_header {
	subject = m.subject, from = m.from, email = m.email,
	recipients = m.to, cc = m.cc, date = m.date,
	actions = { icon_button { icon = "image", tooltip = "Show images", msg = "images", size = "small" } },
	ui.row { justify = "end", ui.button { "Translate", icon = "sparkles", small = true, variant = "outline", msg = "translate" } },
}]],

	build = function(props: Props)
		local to = names(props.recipients)
		local cc = names(props.cc)
		local actions: { any } = props.actions or {}
		local children = positional(props)
		return ui.column {
			class = "message-header",
			props.subject and ui.text { props.subject, class = "message-header-subject", lines = 3 },
			ui.row {
				class = "message-header-from",
				align = "start",
				avatar {
					name = props.from,
					letters = 2,
					src = props.avatar,
					color = props.color,
					shape = "circle",
					size = "xlarge",
					tooltip = props.email or props.from,
				},
				ui.column {
					grow = true,
					gap = "small",
					ui.row {
						align = "center",
						gap = "small",
						wrap = true,
						ui.row {
							class = "message-header-name",
							hug = true,
							msg = props.sender,
							to = props.to,
							ui.text { props.from, truncate = true },
						},
						props.email and ui.row {
							class = "message-header-email",
							hug = true,
							copy = props.email,
							tooltip = "Copy address",
							ui.text { props.email, truncate = true },
						},
					},
					to and ui.text { `To: {to}`, class = "message-header-to", truncate = true },
					cc and ui.text { `Cc: {cc}`, class = "message-header-to", truncate = true },
				},
				(props.date or #actions > 0) and ui.row {
					align = "center",
					gap = "small",
					shrink = false,
					props.date and ui.text { props.date, class = "message-header-date" },
					table.unpack(actions),
				},
			},
			table.unpack(children),
		}
	end,
}
