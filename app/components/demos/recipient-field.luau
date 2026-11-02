-- Addressing a message: type a few letters to see matching contacts, move
-- through them with ↑ ↓, and press Enter or Tab (or click one) to add it.
-- Enter on an address that matches no one adds it as typed; one without an
-- @ shows in red. A chip's × removes it; Cc shows a second line.
local recipient_field = use "recipient-field"

local CONTACTS: { any } = {
	{ name = "Alex Kim", email = "alex@studio.example" },
	{ name = "Dana Holt", email = "dana@freelance.example" },
	{ name = "Daniel Reyes", email = "daniel@studio.example" },
	{ name = "Lucía Romero", email = "lucia@personal.example" },
	{ name = "Priya Shah", email = "priya@studio.example" },
	{ name = "Sam Lee", email = "sam@studio.example" },
}

-- Contacts whose name or address starts a word with the typed text.
local function matches(typed: string): { any }
	if typed == "" then
		return {}
	end
	local query = string.lower(typed)
	local found = {}
	for _, contact in CONTACTS do
		local haystack = string.lower(`{contact.name} {contact.email}`)
		if string.find(haystack, query, 1, true) then
			table.insert(found, contact)
		end
	end
	return found
end

return mod {
	init = function(previous)
		return previous
			or {
				lines = { to = { { name = "Sam Lee", email = "sam@studio.example" } }, cc = {} },
				typed = { to = "da", cc = "" },
				highlighted = 1,
				hidden = { to = false, cc = false },
				cc = false,
			}
	end,

	update = function(state, msg)
		if typeof(msg) ~= "table" then
			return
		end
		-- Which line a message is for: typing carries `line`; a pick or a ×
		-- comes as { to_pick = email } or { cc_remove = index }.
		local line = msg.line or "to"
		for _, name in { "to", "cc" } do
			if msg[`{name}_pick`] or msg[`{name}_remove`] then
				line = name
			end
		end
		local pick = msg[`{line}_pick`]
		local remove = msg[`{line}_remove`]
		local function add(contact)
			table.insert(state.lines[line], contact)
			state.typed[line] = ""
			state.hidden[line] = false
			state.highlighted = 1
		end
		if msg.msg == "type" then
			state.typed[line] = msg.value or ""
			state.hidden[line] = false
			state.highlighted = 1
		elseif msg.msg == "add" then
			local found = if state.hidden[line] then {} else matches(state.typed[line])
			local typed = state.typed[line]
			if found[state.highlighted] then
				add(found[state.highlighted])
			elseif typed ~= "" then
				add({ email = typed, invalid = not string.find(typed, "@", 1, true) })
			end
		elseif msg.msg == "up" or msg.msg == "down" then
			local count = #matches(state.typed[line])
			state.highlighted = math.clamp(state.highlighted + (if msg.msg == "up" then -1 else 1), 1, math.max(count, 1))
		elseif msg.msg == "close" then
			state.hidden[line] = true
		elseif pick then
			for _, contact in CONTACTS do
				if contact.email == pick then
					add(contact)
				end
			end
		elseif remove then
			table.remove(state.lines[line], remove)
		elseif msg.msg == "show-cc" then
			state.cc = true
		end
	end,

	view = function(state)
		local function field(line: string, label: string, trailing: { any }?)
			-- Messages carry which line they came from.
			local function tagged(name: string)
				return { msg = name, line = line }
			end
			return recipient_field {
				id = `demo-{line}`,
				focus = line == "to",
				label = label,
				recipients = state.lines[line],
				value = state.typed[line],
				placeholder = "Name or address",
				change = tagged("type"),
				submit = tagged("add"),
				dismiss = tagged("close"),
				keys = { up = tagged("up"), down = tagged("down"), tab = tagged("add"), escape = tagged("close") },
				suggestions = if state.hidden[line] then {} else matches(state.typed[line]),
				highlighted = state.highlighted,
				pick = `{line}_pick`,
				remove = `{line}_remove`,
				trailing = trailing,
			}
		end
		return ui.column {
			gap = "none",
			width = 460,
			field("to", "To", if state.cc then nil else { ui.button { "Cc", small = true, variant = "ghost", msg = { msg = "show-cc" } } }),
			state.cc and field("cc", "Cc"),
			ui.row { height = 200 },
		}
	end,
}
