-- A text field that completes `@names`: while the word before the caret
-- starts with `@`, the people whose names match show under it, and picking
-- one puts `@Name ` in place of what was typed (through `app:edit`, so the
-- caret, undo and focus stay right). The mod keeps the text and the caret
-- from `change`, as for any field.

local avatar = use "avatar"

type Person = { name: string, detail: string?, src: string? }

type Props = {
	id: string,
	value: string?,
	caret: number?,
	people: { Person }?,
	change: any,
	submit: any,
	blur: any,
	placeholder: string?,
	multiline: boolean?,
	rows: number?,
	max_rows: number?,
	highlighted: number?,
	event: string?,
	dismiss: any,
	most: number?,
	width: number?,
	grow: boolean?,
	focus: boolean?,
	to: string?,
}

-- The `@query` just before the caret, and where its `@` is; nil when the
-- caret isn't in one. The `@` starts the text or follows a space, so an
-- address (sam@studio) does not count.
local function query_at(value: string, caret: number?): string?
	local before = string.sub(value, 1, caret or #value)
	local at, query = string.match(before, "()@([%w%.%-_]*)$")
	if not at then
		return nil
	end
	if at > 1 and not string.match(string.sub(before, at - 1, at - 1), "%s") then
		return nil
	end
	return query
end

-- The people whose name (or one of its words) starts with `query`.
local function matching(people: { Person }, query: string, most: number): { Person }
	local found = {}
	local wanted = string.lower(query)
	for _, person in people do
		local name = string.lower(person.name)
		local hit = string.sub(name, 1, #wanted) == wanted or string.find(name, " " .. wanted, 1, true) ~= nil
		if hit then
			table.insert(found, person)
			if #found >= most then
				break
			end
		end
	end
	return found
end

local EXAMPLE_PEOPLE: { any } = {
	{ name = "Sarah Chen", detail = "Design" },
	{ name = "Sam Okafor", detail = "Engineering" },
	{ name = "Priya Patel", detail = "Marketing" },
}

return component {
	title = "Mention Field",
	category = "Forms",
	icon = "at-sign",
	description = "A text field that completes @names: people matching the word after an @ show under it, and picking one puts @Name in its place.",
	when = "A comment box, a task's description or a chat message where people are mentioned. For addresses on a message use `recipient-field`; for a chat with an agent use `composer`; for a plain field use `ui.input`.",
	props = {
		["id"] = "the field's id: the suggestions edit it through `app:edit`",
		["value"] = "the text, kept from `change` (`msg.value`)",
		["caret"] = "where the caret is, kept from `change` (`msg.caret`)",
		["people"] = "who can be mentioned: { name, detail?, src? (an avatar image) }",
		["change"] = "sent on each edit, with `value` and `caret`",
		["submit"] = "sent on Enter (⌘↩ when multiline) while no suggestion shows",
		["blur"] = "sent with `value` when the field loses focus",
		["placeholder"] = "grey text while empty",
		["multiline"] = "a text area; `rows` and `max_rows` size it (default 2 to 8)",
		["rows"] = "visible lines of a multiline field",
		["max_rows"] = "the most lines a multiline field grows to",
		["highlighted"] = "index of the keyboard-highlighted suggestion (default 1)",
		["event"] = "key of the message ↑ ↓ ↩ and Tab send while suggestions show, default \"mention\": { mention = \"up\" }, { mention = \"down\" }, or { mention = \"choose\", edit = <msg> }, which the mod sends on: `return state, { { to = \"app:edit\", msg = msg.edit } }`",
		["dismiss"] = "sent by Escape or a click outside the suggestions",
		["most"] = "the most suggestions shown (default 6)",
		["width"] = "the suggestions' width in pixels (default 260)",
		["grow"] = "fill the width",
		["focus"] = "take focus when it appears",
		["to"] = "mod its messages go to, if not the drawing one",
	},
	styles = {
		[".mention-field-popover"] = { padding = 4, radius = 10 },
		[".mention-field-row"] = { padding = { 5, 8 }, radius = 6, gap = 8 },
		[".mention-field-row:hover"] = { background = "list-hover" },
		[".mention-field-row:selected"] = { background = "accent/12" },
		[".mention-field-name"] = { weight = "medium" },
		[".mention-field-detail"] = { size = "small", color = "muted-foreground" },
		[".mention-field-head"] = { size = "small", color = "muted-foreground", padding = { 4, 8, 2, 8 } },
	},
	example = { id = "comment", value = "Thanks @sa", people = EXAMPLE_PEOPLE, multiline = true, grow = true },
	usage = [[local mention_field = use "mention-field"

mention_field {
	id = "comment", value = state.text, caret = state.caret, people = MEMBERS,
	change = "type", submit = "post", highlighted = state.highlighted,
	multiline = true, placeholder = "Leave a comment… type @ to mention",
}
-- update: msg.msg == "type" keeps state.text = msg.value and state.caret =
-- msg.caret (and sets state.highlighted = 1); msg.mention == "up"/"down"
-- moves state.highlighted; "choose" sends on its edit:
-- return state, { { to = "app:edit", msg = msg.edit } }]],

	build = function(props: Props)
		local value = props.value or ""
		local query = query_at(value, props.caret)
		local people: { Person } = props.people or {}
		local found: { Person } = if query then matching(people, query, props.most or 6) else {}
		local event = props.event or "mention"
		local highlighted = math.clamp(props.highlighted or 1, 1, math.max(#found, 1))

		-- What picking someone does: replace the `@query` with `@Name `.
		local function edit(person: Person)
			return { field = props.id, insert = `@{person.name} `, back = #(query or "") + 1 }
		end

		local rows = {}
		for index, person in found do
			table.insert(rows, ui.row {
				class = "mention-field-row",
				selected = index == highlighted,
				align = "center",
				msg = edit(person),
				to = "app:edit",
				avatar { name = person.name, src = person.src, size = "small", shape = "circle", letters = 2 },
				ui.text { person.name, class = "mention-field-name" },
				person.detail and ui.text { person.detail, class = "mention-field-detail" },
			})
		end

		local open = #found > 0
		local keys = nil
		if open then
			local choose = { [event] = "choose", edit = edit(found[highlighted]) }
			keys = {
				up = { [event] = "up" },
				down = { [event] = "down" },
				enter = choose,
				tab = choose,
				escape = props.dismiss,
			}
		end
		local multiline = props.multiline == true
		return ui.popover {
			class = "mention-field-popover",
			open = open,
			dismiss = props.dismiss,
			anchor_clicks = true,
			to = props.to,
			side = "bottom",
			align = "start",
			gap = 4,
			width = props.width or 260,
			content = ui.column {
				gap = "none",
				ui.text { "People", class = "mention-field-head" },
				table.unpack(rows),
			},
			ui.input {
				id = props.id,
				value = props.value,
				placeholder = props.placeholder,
				change = props.change,
				-- Enter picks while suggestions show.
				submit = if open then nil else props.submit,
				blur = props.blur,
				keys = keys,
				to = props.to,
				multiline = multiline,
				rows = if multiline then props.rows or 2 else nil,
				max_rows = if multiline then props.max_rows or 8 else nil,
				focus = props.focus,
				grow = props.grow,
			},
		}
	end,
}
