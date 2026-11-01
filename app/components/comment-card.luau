-- One authored message: an avatar, the author in bold, "commented 3d ago",
-- an optional file:line chip, and the body. For pull request
-- conversations, review threads and notes.

type Props = {
	author: string,
	action: string?,
	age: string?,
	seconds: number?,
	body: string?,
	location: string?,
	locate: any,
	badge: string?,
	avatar: string?,
	to: string?,
	actions: { any }?,
	[number]: any,
}

local avatar = use "avatar"

local function ago(seconds: number): string
	if seconds < 60 then
		return "just now"
	elseif seconds < 3600 then
		return `{math.floor(seconds / 60)}m ago`
	elseif seconds < 86400 then
		return `{math.floor(seconds / 3600)}h ago`
	end
	return `{math.floor(seconds / 86400)}d ago`
end

return component {
	title = "Comment Card",
	category = "Data",
	icon = "message-square",
	description = "An authored message: avatar, bold author, \"commented 3d ago\", an optional file:line chip, a role badge, and the body.",
	when = "Comments in a pull request conversation, review threads, notes left by agents or people. Stack them in a column. For a numbered review note with a remove button use `note-card`; for events in order use `timeline`.",
	props = {
		["author"] = "author name (the avatar's initial and colour come from it)",
		["action"] = "verb after the name (default \"commented\"), e.g. \"approved\", \"requested changes\"",
		["age"] = "age text, e.g. \"3d ago\"",
		["seconds"] = "age in seconds instead of `age`",
		["body"] = "the text; blank lines split paragraphs",
		["location"] = "a file:line chip, e.g. \"src/ledger.rs:26\"",
		["locate"] = "message the chip sends on a click (e.g. { open = \"src/ledger.rs:26\" })",
		["badge"] = "a small role tag after the name: \"author\", \"bot\"",
		["avatar"] = "an image file for the avatar",
		["to"] = "the mod to send `locate` to",
		["actions"] = "elements at the header's right",
		["[children]"] = "anything after the body: a code-view, a diff-view, buttons",
	},
	styles = {
		[".comment-card"] = { gap = 10, padding = 12, radius = 8, border = "all", border_color = "border", background = "background" },
		[".comment-card-author"] = { weight = "semibold", size = "small" },
		[".comment-card-meta"] = { size = "small", color = "muted-foreground" },
		[".comment-card-badge"] = { radius = 999, padding = { 0, 6 }, border = "all", border_color = "border" },
		[".comment-card-badge-text"] = { size = "small", color = "muted-foreground" },
		[".comment-card-location"] = { gap = 4, radius = 6, padding = { 1, 6 }, background = "accent/10", color = "accent" },
		[".comment-card-location:hover"] = { background = "accent/18" },
		[".comment-card-location-text"] = { font = "mono", size = "small" },
		[".comment-card-body"] = { gap = 6 },
		[".comment-card-text"] = { size = "small" },
	},
	example = {
		author = "alice",
		action = "requested changes",
		age = "3d ago",
		badge = "reviewer",
		location = "src/ledger.rs:26",
		body = "Reject mixing currencies here: return a Result instead of silently grouping.\n\nA builder would also keep the call sites short.",
	},

	build = function(props: Props)
		local paragraphs = {}
		if props.body then
			for _, paragraph in string.split(props.body, "\n\n") do
				if paragraph ~= "" then
					table.insert(paragraphs, ui.text { paragraph, class = "comment-card-text" })
				end
			end
		end
		for _, child in ipairs(props) do
			table.insert(paragraphs, child)
		end
		local age = props.age
		if age == nil and props.seconds then
			age = ago(props.seconds)
		end
		local meta = props.action or "commented"
		if age then
			meta ..= " " .. age
		end

		return ui.row {
			class = "comment-card",
			shrink = false,
			align = "start",
			avatar { name = props.author, src = props.avatar, size = "normal", shape = "circle" },
			ui.column {
				grow = true,
				gap = "small",
				ui.row {
					align = "center",
					gap = "small",
					wrap = true,
					ui.text { props.author, class = "comment-card-author" },
					props.badge and ui.row {
						class = "comment-card-badge",
						ui.text { props.badge, class = "comment-card-badge-text" },
					},
					ui.text { meta, class = "comment-card-meta" },
					ui.spacer(),
					table.unpack(props.actions or {}),
				},
				props.location and ui.row {
					ui.row {
						class = "comment-card-location",
						align = "center",
						msg = props.locate,
						to = props.to,
						ui.icon { name = "file", size = "small" },
						ui.text { props.location, class = "comment-card-location-text" },
					},
				},
				#paragraphs > 0 and ui.column { class = "comment-card-body", table.unpack(paragraphs) },
			},
		}
	end,
}
