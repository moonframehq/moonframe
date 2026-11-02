-- One authored message: an avatar, the author in bold, "commented 3d ago",
-- an optional file:line chip, the body, and reactions under it with a
-- button to add one. For pull request conversations, review threads and
-- notes.

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
	reactions: { Reaction }?,
	react: string?,
	add_reaction: any,
	picker: any,
	picker_open: boolean?,
	[number]: any,
}

type Reaction = {
	emoji: string,
	count: number?,
	mine: boolean?,
	tooltip: string?,
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

-- The children in order. `ipairs` and `#` stop at the first `nil`, which a
-- child such as `cond and ui.text "x"` leaves, and drop what follows it.
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

-- The reactions as small pills, then the button that adds one, with the
-- picker in a popover on it while open.
local function reactions(props: Props): any
	local key = props.react or "react"
	local pills = {}
	local list: { Reaction } = props.reactions or {}
	for _, reaction in list do
		table.insert(pills, ui.row {
			class = if reaction.mine then { "comment-card-reaction", "comment-card-reaction-mine" } else "comment-card-reaction",
			align = "center",
			hug = true,
			shrink = false,
			msg = { [key] = reaction.emoji },
			to = props.to,
			tooltip = reaction.tooltip,
			ui.text { reaction.emoji, class = "comment-card-reaction-emoji" },
			ui.text { tostring(reaction.count or 1), class = "comment-card-reaction-count" },
		})
	end
	if props.add_reaction ~= nil then
		local button = ui.row {
			class = "comment-card-add-reaction",
			height = 24,
			align = "center",
			hug = true,
			shrink = false,
			selected = props.picker_open == true,
			msg = props.add_reaction,
			to = props.to,
			tooltip = "Add reaction",
			ui.text { "\u{263A}\u{FE0E}", class = "comment-card-add-reaction-face" },
			ui.icon { name = "plus", class = "comment-card-add-reaction-plus" },
		}
		if props.picker then
			button = ui.popover {
				class = "emoji-picker-popover",
				open = props.picker_open == true,
				dismiss = props.add_reaction,
				to = props.to,
				side = "bottom",
				align = "start",
				gap = 4,
				content = props.picker,
				button,
			}
		end
		table.insert(pills, button)
	end
	if #pills == 0 then
		return nil
	end
	return ui.row { class = "comment-card-reactions", align = "center", wrap = true, table.unpack(pills) }
end

return component {
	title = "Comment Card",
	category = "Data",
	icon = "message-square",
	description = "An authored message: avatar, bold author, \"commented 3d ago\", an optional file:line chip, a role badge, the body, and reactions with a button to add one.",
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
		["reactions"] = "list of { emoji, count, mine?, tooltip? }: pills under the body, `mine` tinted (the reader's own)",
		["react"] = "the key a reaction's click sends its emoji under (default \"react\": `{ react = \"👍\" }`)",
		["add_reaction"] = "message the add-reaction button sends; no button without it. With `picker`, a click outside the open picker or Escape sends it again, so the mod toggles",
		["picker"] = "an element, usually `emoji-picker`, shown in a popover on the add-reaction button",
		["picker_open"] = "show `picker`",
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
		[".comment-card-reactions"] = { gap = 6, padding = { 2, 0, 0, 0 } },
		[".comment-card-reaction"] = { height = 24, radius = 999, padding = { 0, 8 }, gap = 5, border = "all", border_color = "border", background = "background" },
		[".comment-card-reaction:hover"] = { border_color = "muted-foreground/40" },
		[".comment-card-reaction-mine"] = { background = "accent/10", border_color = "accent/45", color = "accent" },
		[".comment-card-reaction-mine:hover"] = { border_color = "accent" },
		[".comment-card-reaction-emoji"] = { size = "small" },
		[".comment-card-reaction-count"] = { size = "small", weight = "medium" },
		[".comment-card-add-reaction"] = { radius = 999, padding = { 0, 7 }, gap = 1, border = "all", border_color = "border", color = "muted-foreground" },
		[".comment-card-add-reaction:hover"] = { color = "foreground", border_color = "muted-foreground/40" },
		[".comment-card-add-reaction:selected"] = { color = "accent", border_color = "accent/45", background = "accent/10" },
		[".comment-card-add-reaction-face"] = { size = "normal" },
		[".comment-card-add-reaction-plus"] = { size = "small" },
	},
	example = {
		author = "alice",
		action = "requested changes",
		age = "3d ago",
		badge = "reviewer",
		location = "src/ledger.rs:26",
		body = "Reject mixing currencies here: return a Result instead of silently grouping.\n\nA builder would also keep the call sites short.",
		reactions = { { emoji = "👍", count = 3, mine = true }, { emoji = "🎉", count = 1, mine = false } },
		add_reaction = "add-reaction",
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
		for _, child in positional(props) do
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
				reactions(props),
			},
		}
	end,
}
