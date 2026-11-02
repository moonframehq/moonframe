-- A one-line search box. Each edit sends `change` with `value` set to the
-- text; Enter sends `submit`; Esc sends `cancel` (to clear it, pass
-- `value = ""` back). Up and down send `previous` and `next`, for a list of
-- results under it. With an `icon` or a `shortcut` it draws its own box: a
-- magnifier, the field, and the key that focuses it (⌘K).

local kbd = use "kbd"

type Props = {
	id: string?,
	value: string?,
	placeholder: string?,
	change: any,
	submit: any,
	cancel: any,
	previous: any,
	next: any,
	to: string?,
	focus: boolean?,
	width: number?,
	icon: any,
	shortcut: any,
}

return component {
	title = "Search field",
	category = "Forms",
	icon = "search",
	description = "A one-line search box; edits send change with the text as `value`, Esc sends cancel, up and down move through results.",
	when = "A query box over a list or palette: filter files, branches or commands as the user types, with up and down to move through results. For a longer value use `ui.input` (multiline for a text area); for one-of-many choices use `segmented` or `dropdown`.",
	props = {
		["id"] = "keeps the text across renders",
		["value"] = "the text; applied when it changes, so `\"\"` clears it",
		["placeholder"] = "grey text while empty",
		["change"] = "sent on each edit, with `value` set to the text",
		["submit"] = "sent on Enter",
		["cancel"] = "sent on Esc",
		["previous"] = "sent on up",
		["next"] = "sent on down",
		["to"] = "the mod the messages go to",
		["focus"] = "take focus when it appears",
		["width"] = "pixels",
		["icon"] = "an icon before the text (`true` for a magnifier); draws the field in a rounded box of its own",
		["shortcut"] = "keys shown as a cap at the end, such as \"cmd-k\" (it only shows them: the mod binds the key); draws the boxed field",
	},
	styles = {
		[".search-field"] = { background = "background", border = "all", border_color = "border", radius = 10, padding = { 2, 10 }, gap = 8 },
		[".search-field-icon"] = { color = "muted-foreground" },
		[".search-field-input"] = { background = "#00000000", border = "none", shadow = "none", padding = { 4, 0 } },
	},
	example = { placeholder = "Search", icon = true, shortcut = "cmd-k", width = 320 },
	usage = [[
local search = use "search-field"

-- in view:
search {
	id = "files", value = state.query, placeholder = "Search files…",
	change = "query", cancel = "clear", previous = "up", next = "down",
}
-- in update: msg.msg == "query" sets state.query = msg.value;
-- "clear" sets state.query = "" (the field empties)]],

	build = function(props: Props)
		local boxed = props.icon ~= nil or props.shortcut ~= nil
		local field = ui.input {
			id = props.id,
			value = props.value,
			placeholder = props.placeholder or "Search…",
			change = props.change,
			submit = props.submit,
			keys = {
				escape = props.cancel,
				up = props.previous,
				down = props.next,
			},
			to = props.to,
			focus = props.focus,
			width = if boxed then nil else props.width,
			grow = if boxed then true else nil,
			class = if boxed then "search-field-input" else nil,
		}
		if not boxed then
			return field
		end
		local icon = if props.icon == true then "search" else props.icon
		return ui.row {
			class = "search-field",
			align = "center",
			width = props.width,
			icon and ui.icon { name = icon :: any, class = "search-field-icon" },
			field,
			props.shortcut and kbd { keys = props.shortcut, small = true },
		}
	end,
}
