-- A one-line search box. Each edit sends `change` with `value` set to the
-- text; Enter sends `submit`; Esc sends `cancel` (to clear it, pass
-- `value = ""` back). Up and down send `previous` and `next`, for a list of
-- results under it.

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
	},
	example = { placeholder = "Search files…" },
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
		return ui.input {
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
			width = props.width,
		}
	end,
}
