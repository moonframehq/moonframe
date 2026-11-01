-- A message in a chat with an agent or a person. From the user it is a
-- tinted bubble on the right; from the assistant, plain text under a small
-- header (an icon chip and a name), holding whatever else it shows: a
-- `thinking` fold, a `record-card`, a `numbered-list`.

type Props = {
	from: string?,
	author: string?,
	icon: string?,
	text: string?,
	[number]: any,
}

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

return component {
	title = "Chat Message",
	category = "Data",
	icon = "message-square",
	description = "A chat message: the user's as a bubble on the right, the assistant's as text under an icon and a name, with anything else inside.",
	when = "Each turn of a conversation with an agent or a person, stacked in a scrolling column over a `composer`. For a comment on a pull request use `comment-card`; for a notification use `toast`.",
	props = {
		["from"] = "\"user\" (a bubble on the right) or \"assistant\" (default)",
		["author"] = "the assistant's name in its header; no header without it",
		["icon"] = "the icon in the header's chip (default sparkles)",
		["text"] = "the message's text; paragraphs split on blank lines",
		["[children]"] = "more after the text: a `thinking` fold, cards, lists",
	},
	styles = {
		[".chat-message"] = { gap = 10 },
		[".chat-message-head"] = { gap = 8 },
		[".chat-message-chip"] = { background = "accent", color = "accent-foreground", radius = 6 },
		[".chat-message-author"] = { weight = "medium" },
		[".chat-message-bubble"] = { background = "secondary", radius = 12, padding = { 10, 14 } },
	},
	example = { from = "user", text = "Which deal should I look at first?" },
	usage = [[local chat_message = use "chat-message"
local thinking = use "thinking"

chat_message { from = "user", text = "Which deal should I look at first?" }
chat_message {
	author = "Sales agent", text = "Start with Lumen Retail: the biggest deal at risk.",
	thinking { seconds = 4, open = state.thought, msg = "toggle-thought" },
}]],

	build = function(props: Props)
		local paragraphs = {}
		for paragraph in string.gmatch((props.text or "") .. "\n\n", "(.-)\n\n") do
			if string.match(paragraph, "%S") then
				table.insert(paragraphs, ui.text { paragraph, wrap = true })
			end
		end
		if props.from == "user" then
			return ui.row {
				justify = "end",
				ui.column { class = "chat-message-bubble", max_width = 300, hug = true, table.unpack(paragraphs) },
			}
		end
		local children = positional(props)
		return ui.column {
			class = "chat-message",
			props.author and ui.row {
				class = "chat-message-head",
				align = "center",
				ui.row {
					class = "chat-message-chip",
					width = 22,
					height = 22,
					align = "center",
					justify = "center",
					ui.icon { name = (props.icon or "sparkles") :: any, size = "small" },
				},
				ui.text { props.author, class = "chat-message-author" },
			},
			ui.column { gap = "normal", table.unpack(paragraphs) },
			table.unpack(children),
		}
	end,
}
