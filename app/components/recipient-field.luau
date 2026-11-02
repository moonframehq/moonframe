-- An address line of a message being written: a label (To, Cc), the
-- recipients as chips with a × each, and a field to type more, with the
-- matching contacts in a menu under it. The mod keeps the list: Enter sends
-- `submit` with the text, a suggestion sends { pick = email }, a × sends
-- { remove = index }.

local menu = use "menu"

type Recipient = { name: string?, email: string, invalid: boolean? }

type Props = {
	label: string?,
	recipients: { any }?,
	id: string?,
	value: string?,
	placeholder: string?,
	change: any,
	submit: any,
	focus: boolean?,
	keys: { [string]: any }?,
	suggestions: { any }?,
	highlighted: number?,
	pick: string?,
	remove: string?,
	dismiss: any,
	trailing: { any }?,
	width: number?,
	to: string?,
}

-- A recipient given as a bare address ("sam@studio.example") or a table.
local function recipient(value: any): Recipient
	if type(value) == "string" then
		return { email = value }
	end
	return value
end

-- The preview's recipients, typed loosely: one has a name, one is invalid.
local EXAMPLE_TO: { any } = {
	{ name = "Sam Lee", email = "sam@studio.example" },
	{ email = "priya@", invalid = true },
}

return component {
	title = "Recipient Field",
	category = "Forms",
	icon = "at-sign",
	description = "An address line: a label, recipients as removable chips (red when invalid), a field to type more, and matching contacts in a menu under it.",
	when = "The To, Cc and Bcc lines of a message being written, or inviting people by address. For tags the user only reads use `tags`; for filters use `chips`; for a single free-text value use `ui.input`; for a search box use `search-field`.",
	props = {
		["label"] = "text before the chips (\"To\", \"Cc\")",
		["recipients"] = "the chips: addresses, or { name?, email, invalid? } (shown by name, the address under the pointer; `invalid` in the danger colour)",
		["id"] = "keeps the typed text across renders (default \"recipients\")",
		["value"] = "the typed text; set it to \"\" after adding a recipient",
		["placeholder"] = "grey text while empty and there are no chips",
		["change"] = "sent on each edit, with `value` set to the text",
		["submit"] = "sent on Enter, with `value`: add the highlighted suggestion or the typed address",
		["focus"] = "take focus when it appears",
		["keys"] = "more keys while typing, such as { up = \"up\", down = \"down\", tab = \"add\", escape = \"close\" }",
		["suggestions"] = "contacts matching the text: { name?, email }; a menu under the field while there are some",
		["highlighted"] = "index of the keyboard-highlighted suggestion",
		["pick"] = "key of the message a suggestion sends, default \"pick\": { pick = email }",
		["remove"] = "key of the message a chip's × sends, default \"remove\": { remove = index }",
		["dismiss"] = "message a click outside the suggestions sends (to clear them)",
		["trailing"] = "elements at the line's right end (Cc and Bcc buttons)",
		["width"] = "the suggestions menu's width in pixels (default 300)",
		["to"] = "mod to send its messages to, if not the drawing one",
	},
	styles = {
		[".recipient-field"] = { padding = { 6, 0 }, gap = 6, border = "bottom", border_color = "border" },
		[".recipient-field-label"] = { color = "muted-foreground", padding = { 0, 4, 0, 0 } },
		[".recipient-field-chip"] = { background = "accent/12", color = "accent", radius = 999, padding = { 2, 4, 2, 10 }, gap = 4 },
		[".recipient-field-invalid"] = { background = "danger/12", color = "danger" },
		[".recipient-field-remove"] = { radius = 999, padding = 2, opacity = 0.7 },
		[".recipient-field-remove:hover"] = { background = "foreground/10", opacity = 1 },
		[".recipient-field-input"] = { background = "#00000000", border = "none", shadow = "none", padding = { 2, 0 } },
		[".recipient-field-popover"] = { padding = 0, radius = 10 },
	},
	example = {
		label = "To",
		recipients = EXAMPLE_TO,
		value = "da",
		suggestions = {
			{ name = "Daniel Reyes", email = "daniel@studio.example" },
			{ name = "Dana Holt", email = "dana@freelance.example" },
		},
		highlighted = 1,
	},
	usage = [[local recipient_field = use "recipient-field"

recipient_field {
	label = "To", recipients = state.to, value = state.typed,
	change = "type", submit = "add", dismiss = "close",
	keys = { up = "up", down = "down", tab = "add", escape = "close" },
	suggestions = matches(state.typed), highlighted = state.highlighted,
	trailing = { ui.button { "Cc", small = true, variant = "ghost", msg = "show-cc" } },
}
-- update: msg.msg == "type" keeps msg.value; "add" appends the highlighted
-- suggestion (or the typed address) and sets state.typed = ""; msg.pick
-- appends that address; msg.remove drops the recipient at that index.]],

	build = function(props: Props)
		local pick = props.pick or "pick"
		local remove = props.remove or "remove"
		local chips = {}
		local recipients: { any } = props.recipients or {}
		for index, value in recipients do
			local person = recipient(value)
			chips[#chips + 1] = ui.row {
				class = if person.invalid
					then { "recipient-field-chip", "recipient-field-invalid" }
					else "recipient-field-chip",
				align = "center",
				hug = true,
				shrink = false,
				tooltip = if person.invalid then `Not a valid address: {person.email}` else person.email,
				ui.text { person.name or person.email, size = "small", weight = "medium" },
				ui.row {
					class = "recipient-field-remove",
					msg = { [remove] = index },
					to = props.to,
					tooltip = "Remove",
					ui.icon { name = "close", size = "small" },
				},
			}
		end

		local items = {}
		local suggestions: { any } = props.suggestions or {}
		for _, value in suggestions do
			local person = recipient(value)
			items[#items + 1] = {
				id = person.email,
				label = person.name or person.email,
				detail = if person.name then person.email else nil,
				icon = "user",
			}
		end

		-- The field joins the chips' row, so it wraps after them.
		chips[#chips + 1] = ui.input {
			id = props.id or "recipients",
			value = props.value,
			placeholder = if #chips == 0 then props.placeholder else nil,
			change = props.change,
			submit = props.submit,
			keys = props.keys,
			focus = props.focus,
			to = props.to,
			width = 140,
			grow = true,
			class = "recipient-field-input",
		}
		local trailing: { any } = props.trailing or {}

		return ui.popover {
			class = "recipient-field-popover",
			open = #items > 0,
			dismiss = props.dismiss,
			-- The field, the chips' × and the trailing buttons keep working
			-- while the suggestions show.
			anchor_clicks = true,
			to = props.to,
			side = "bottom",
			align = "start",
			gap = 4,
			width = props.width or 300,
			content = menu { items = items, highlighted = props.highlighted, event = pick, to = props.to },
			ui.row {
				class = "recipient-field",
				align = "center",
				grow = true,
				props.label and ui.text { props.label, class = "recipient-field-label" },
				ui.row {
					align = "center",
					gap = "small",
					wrap = true,
					grow = true,
					table.unpack(chips),
				},
				table.unpack(trailing),
			},
		}
	end,
}
