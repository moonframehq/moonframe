-- A grid of emoji to react with: an optional search field, the ones used
-- most, then about eighty common ones under Smileys, Gestures and Objects &
-- Symbols. A click sends `{ pick = "👍" }`. Meant for a `ui.popover`.

local search_field = use "search-field"

type Props = {
	recent: { string }?,
	event: string?,
	query: string?,
	search: any,
	id: string?,
	focus: boolean?,
	cancel: any,
	columns: number?,
	height: number?,
	to: string?,
}

-- Each category's emoji with the words a search matches.
local CATEGORIES = {
	{
		title = "Smileys",
		emoji = {
			{ "😀", "grinning smile happy" },
			{ "😃", "smiley happy joy" },
			{ "😄", "smile happy laugh" },
			{ "😁", "grin beaming" },
			{ "😆", "laughing satisfied" },
			{ "😅", "sweat smile relief" },
			{ "😂", "joy tears laugh lol" },
			{ "🙂", "slight smile" },
			{ "😉", "wink" },
			{ "😊", "blush happy" },
			{ "😍", "heart eyes love" },
			{ "🥰", "hearts love adore" },
			{ "😘", "kiss love" },
			{ "😋", "yum tasty" },
			{ "😎", "cool sunglasses" },
			{ "🤓", "nerd geek" },
			{ "🤔", "thinking hmm" },
			{ "🤨", "raised eyebrow skeptic" },
			{ "😐", "neutral meh" },
			{ "🙄", "eye roll" },
			{ "😏", "smirk" },
			{ "😬", "grimace awkward" },
			{ "😮", "open mouth wow surprised" },
			{ "😲", "astonished shocked" },
			{ "🥳", "party celebrate" },
			{ "🤩", "star struck excited" },
			{ "😢", "cry sad tear" },
			{ "😭", "sob crying sad" },
			{ "😤", "triumph huff" },
			{ "😡", "angry mad rage" },
			{ "😱", "scream fear shock" },
			{ "🥲", "smile tear" },
			{ "😴", "sleep tired zzz" },
			{ "🤯", "mind blown exploding" },
			{ "🫠", "melting" },
			{ "🤗", "hug" },
		},
	},
	{
		title = "Gestures",
		emoji = {
			{ "👍", "thumbs up yes like +1 approve" },
			{ "👎", "thumbs down no dislike -1" },
			{ "👏", "clap applause bravo" },
			{ "🙌", "raised hands hooray celebrate" },
			{ "🙏", "pray please thanks" },
			{ "👋", "wave hello bye" },
			{ "👌", "ok perfect" },
			{ "✌️", "victory peace" },
			{ "🤞", "fingers crossed luck" },
			{ "🤝", "handshake deal agree" },
			{ "💪", "muscle strong flex" },
			{ "👀", "eyes look see watching" },
			{ "🫡", "salute" },
			{ "🤷", "shrug dunno" },
			{ "🤦", "facepalm" },
			{ "☝️", "point up" },
			{ "👉", "point right" },
			{ "✋", "hand stop high five" },
		},
	},
	{
		title = "Objects & Symbols",
		emoji = {
			{ "❤️", "heart love red" },
			{ "🧡", "orange heart" },
			{ "💜", "purple heart" },
			{ "💔", "broken heart" },
			{ "🔥", "fire hot lit" },
			{ "✨", "sparkles shiny new" },
			{ "⭐", "star" },
			{ "🎉", "tada party celebrate hooray" },
			{ "🎯", "target bullseye goal" },
			{ "🚀", "rocket ship launch fast" },
			{ "💡", "bulb idea" },
			{ "📌", "pin pushpin" },
			{ "📎", "paperclip attach" },
			{ "📅", "calendar date" },
			{ "⏰", "alarm clock time" },
			{ "🐛", "bug" },
			{ "🔒", "lock secure" },
			{ "🏆", "trophy win award" },
			{ "☕", "coffee break" },
			{ "💯", "hundred perfect score" },
			{ "✅", "check done yes" },
			{ "❌", "cross no wrong" },
			{ "⚠️", "warning caution" },
			{ "❓", "question" },
			{ "❗", "exclamation important" },
			{ "➕", "plus add" },
			{ "👆", "up this" },
			{ "🚧", "construction wip" },
		},
	},
}

-- Emoji in rows of `columns`, each sending `{ [event] = emoji }`.
local function grid(list: { any }, columns: number, event: string, to: string?)
	local rows = {}
	local row = {}
	for _, entry in list do
		table.insert(row, ui.row {
			class = "emoji-picker-cell",
			width = 32,
			height = 32,
			shrink = false,
			align = "center",
			justify = "center",
			msg = { [event] = entry[1] },
			to = to,
			ui.text { entry[1], class = "emoji-picker-emoji" },
		})
		if #row == columns then
			table.insert(rows, ui.row { gap = "none", shrink = false, table.unpack(row) })
			row = {}
		end
	end
	if #row > 0 then
		table.insert(rows, ui.row { gap = "none", shrink = false, table.unpack(row) })
	end
	return ui.column { gap = "none", shrink = false, table.unpack(rows) }
end

local function heading(title: string)
	return ui.row { shrink = false, ui.text { title, class = "emoji-picker-heading" } }
end

return component {
	title = "Emoji Picker",
	category = "Overlays",
	icon = "heart",
	description = "A grid of about eighty common emoji under Smileys, Gestures and Objects & Symbols, with a search field and a Frequently used row; a click sends the emoji.",
	when = "Reacting to a comment or a message, or putting an emoji in a name: open it in a `ui.popover` from an add-reaction button (give the popover `class = \"emoji-picker-popover\"`). For a list of actions use `menu`; for colours use `swatches`.",
	props = {
		["recent"] = "emoji used most, shown first under Frequently used",
		["event"] = "the key a click sends the emoji under (default \"pick\": `{ pick = \"👍\" }`)",
		["search"] = "message each edit of the search field sends, with `value`; no field without it",
		["query"] = "the search text (the mod keeps it from `search`); shows only emoji whose names match",
		["id"] = "keeps the search field's text across renders",
		["focus"] = "focus the search field when it appears",
		["cancel"] = "message Escape sends from the search field (close the popover)",
		["columns"] = "emoji per row (default 8)",
		["height"] = "the most pixels it takes before it scrolls (default 260)",
		["to"] = "mod to send the messages to, if not the drawing one",
	},
	styles = {
		[".emoji-picker"] = { gap = 8, padding = 8 },
		[".emoji-picker-popover"] = { padding = 0, radius = 12 },
		[".emoji-picker-heading"] = { size = "small", weight = "medium", color = "muted-foreground", padding = { 4, 4, 2, 4 } },
		[".emoji-picker-cell"] = { radius = 6 },
		[".emoji-picker-cell:hover"] = { background = "list-hover" },
		[".emoji-picker-emoji"] = { size = "large" },
		[".emoji-picker-empty"] = { size = "small", color = "muted-foreground", padding = { 16, 4 } },
	},
	example = { recent = { "👍", "🎉", "❤️", "🚀", "👀" }, search = "search" },
	usage = [[local emoji_picker = use "emoji-picker"

ui.popover {
	class = "emoji-picker-popover",
	open = state.picking,
	dismiss = "close",
	content = emoji_picker { recent = state.recent, query = state.query, search = "query", cancel = "close", focus = true },
	ui.button { "React", small = true, msg = "open" },
}

-- in update: msg.pick is the emoji]],

	build = function(props: Props)
		local columns = props.columns or 8
		local event = props.event or "pick"
		local query = string.lower(props.query or "")
		local sections = {}
		if query == "" then
			if props.recent and #props.recent > 0 then
				local recent = {}
				for index, emoji in props.recent do
					if index > columns then
						break
					end
					table.insert(recent, { emoji })
				end
				table.insert(sections, heading("Frequently used"))
				table.insert(sections, grid(recent, columns, event, props.to))
			end
			for _, category in CATEGORIES do
				table.insert(sections, heading(category.title))
				table.insert(sections, grid(category.emoji, columns, event, props.to))
			end
		else
			local found = {}
			for _, category in CATEGORIES do
				for _, entry in category.emoji do
					if string.find(entry[2], query, 1, true) then
						table.insert(found, entry)
					end
				end
			end
			if #found > 0 then
				table.insert(sections, grid(found, columns, event, props.to))
			else
				table.insert(sections, ui.text { "No emoji match", class = "emoji-picker-empty" })
			end
		end
		return ui.column {
			class = "emoji-picker",
			width = columns * 32 + 16,
			props.search ~= nil and search_field {
				id = props.id or "emoji-picker-search",
				value = props.query,
				placeholder = "Search emoji",
				change = props.search,
				cancel = props.cancel,
				focus = props.focus,
				to = props.to,
				icon = true,
			},
			ui.column {
				style = { gap = 2 },
				scroll = true,
				max_height = props.height or 260,
				table.unpack(sections),
			},
		}
	end,
}
