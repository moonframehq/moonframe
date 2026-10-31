-- A numbered review note: a yellow circle with its number, the file:line
-- it points at, its text, and a × that sends `{ remove = id }`. Clicking
-- the note sends `{ select = id }` (to jump to the line).

type Props = {
	id: any,
	number: number?,
	location: string?,
	text: string,
	selected: boolean?,
	event: string?,
	remove: string?,
	to: string?,
}

return component {
	title = "Note Card",
	category = "Data",
	icon = "file-text",
	description = "A numbered review note: yellow numbered circle, file:line, text and a × remove button; selectable.",
	when = "Notes a person leaves on lines while reviewing, to send to an agent; list them in a column beside a `diff-view`. For an authored comment use `comment-card`.",
	props = {
		["id"] = "the note's id, sent in messages (default its number)",
		["number"] = "shown in the circle",
		["location"] = "file:line it points at",
		["text"] = "the note",
		["selected"] = "highlight it",
		["event"] = "click message key instead of \"select\": sends { [event] = id }",
		["remove"] = "× message key instead of \"remove\": sends { [remove] = id }",
		["to"] = "the mod to send them to",
	},
	styles = {
		[".note-card"] = { gap = 10, padding = { 10, 12 }, border = "bottom", border_color = "border" },
		[".note-card:hover"] = { background = "list-hover" },
		[".note-card:selected"] = { background = "list-active" },
		[".note-card-number"] = { radius = 999, background = "warning", color = "#1e1e2e" },
		[".note-card-number-text"] = { size = "small", weight = "bold" },
		[".note-card-location"] = { font = "mono", size = "small", color = "muted-foreground" },
		[".note-card-text"] = { size = "small" },
		[".note-card-remove"] = { radius = 4, padding = 2, color = "muted-foreground" },
		[".note-card-remove:hover"] = { background = "muted-foreground/15", color = "foreground" },
		[".note-card-remove-icon"] = { size = "small" },
	},
	example = {
		number = 1,
		location = "src/ledger.rs:26",
		text = "Reject mixing currencies here: return a Result instead of silently grouping.",
	},

	build = function(props: Props)
		local id = if props.id == nil then props.number else props.id
		return ui.row {
			class = "note-card",
			shrink = false,
			align = "start",
			selected = props.selected,
			msg = { [props.event or "select"] = id },
			to = props.to,
			ui.row {
				class = "note-card-number",
				width = 20,
				height = 20,
				align = "center",
				justify = "center",
				ui.text { tostring(props.number or ""), class = "note-card-number-text" },
			},
			ui.column {
				grow = true,
				gap = "small",
				props.location and ui.text { props.location, class = "note-card-location", truncate = true },
				ui.text { props.text, class = "note-card-text" },
			},
			ui.row {
				class = "note-card-remove",
				msg = { [props.remove or "remove"] = id },
				to = props.to,
				tooltip = "Remove note",
				ui.icon { name = "close", class = "note-card-remove-icon" },
			},
		}
	end,
}
