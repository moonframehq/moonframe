-- A comment box: type @ and a few letters to mention someone; ↑ ↓ choose,
-- ↩ or Tab picks, Esc closes. ⌘↩ posts the comment.
local mention_field = use "mention-field"

local MEMBERS = {
	{ name = "Sarah Chen", detail = "Design" },
	{ name = "John Carter", detail = "Engineering" },
	{ name = "Emma Wilson", detail = "Design" },
	{ name = "Priya Patel", detail = "Marketing" },
	{ name = "Marcus Lee", detail = "Engineering" },
	{ name = "Lena Fischer", detail = "Product" },
}

return mod {
	init = function(previous)
		return previous or { text = "", caret = 0, highlighted = 1, posted = {}, closed = false }
	end,

	update = function(state, msg)
		if typeof(msg) ~= "table" then
			if msg == "close" then
				state.closed = true
			end
			return
		end
		if msg.msg == "type" then
			state.text = msg.value
			state.caret = msg.caret
			state.highlighted = 1
			state.closed = false
		elseif msg.mention == "up" then
			state.highlighted = math.max(1, state.highlighted - 1)
		elseif msg.mention == "down" then
			state.highlighted += 1
		elseif msg.mention == "choose" then
			return state, { { to = "app:edit", msg = msg.edit } }
		elseif msg.msg == "post" and msg.value ~= "" then
			table.insert(state.posted, msg.value)
			state.text = ""
			state.caret = 0
		end
	end,

	view = function(state)
		local children: { any } = {}
		for _, text in state.posted do
			table.insert(children, ui.text { tostring(text) })
		end
		table.insert(children, mention_field {
			id = "comment",
			value = state.text,
			-- Closed with Esc until the next edit.
			caret = if state.closed then 0 else state.caret,
			people = MEMBERS,
			change = "type",
			submit = "post",
			dismiss = "close",
			highlighted = state.highlighted,
			multiline = true,
			grow = true,
			placeholder = "Leave a comment… type @ to mention",
		})
		return ui.column { gap = "normal", width = 480, table.unpack(children) }
	end,
}
