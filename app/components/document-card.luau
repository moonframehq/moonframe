-- A document as a tile in a grid: its title, the first lines of it rendered
-- as markdown, and when it changed. For a notes app's home screen.

local icon_button = use "icon-button"

type Props = {
	title: string,
	text: string?,
	date: string?,
	lines: number?,
	favorite: boolean?,
	star: any?,
	selected: boolean?,
	msg: any?,
	context: any?,
	width: number?,
	height: number?,
	to: string?,
}

return component {
	title = "Document Card",
	category = "Data",
	icon = "file-text",
	description = "A document as a grid tile: its title, a preview of its first lines rendered as markdown, the date it changed and a star.",
	when = "A home screen or a folder of notes shown as a grid. For a list beside an editor use `document-row`; for attached files use `file-card`; for reading material with progress use `guide-card`.",
	props = {
		["title"] = "the document's title",
		["text"] = "its markdown, cut to `lines` lines",
		["date"] = "when it last changed, as shown (\"Today, 14:02\")",
		["lines"] = "lines of preview shown (default 5)",
		["favorite"] = "draw the star filled",
		["star"] = "message the star sends; no star without it",
		["selected"] = "highlight it",
		["msg"] = "message a click sends (open it)",
		["context"] = "message a right-click sends (a menu)",
		["width"] = "pixels (default 240)",
		["height"] = "pixels (default 200)",
		["to"] = "mod to send them to, if not the drawing one",
	},
	styles = {
		[".document-card"] = { background = "card", border = "all", border_color = "border", radius = 12, padding = 14, gap = 8, clip = true },
		[".document-card:hover"] = { border_color = "accent/50" },
		[".document-card:selected"] = { border_color = "accent" },
		[".document-card-title"] = { weight = "semibold" },
		[".document-card-preview"] = { padding = 0 },
		[".document-card-date"] = { size = "small", color = "muted-foreground" },
	},
	example = {
		title = "On short sentences",
		text = "A short sentence is a **door**. It opens, you walk through, and you are somewhere new.\n\n- Say one thing\n- Stop",
		date = "Today, 09:41",
		favorite = true,
		star = "star",
	},
	usage = [[local document_card = use "document-card"

ui.row { wrap = true, gap = "large",
	document_card { title = note.title, text = note.body, date = note.changed,
		favorite = note.starred, star = { star = note.id }, msg = { open = note.id } },
}]],

	build = function(props: Props)
		return ui.column {
			class = "document-card",
			width = props.width or 240,
			height = props.height or 200,
			selected = props.selected,
			msg = props.msg,
			context = props.context,
			to = props.to,
			ui.row {
				align = "center",
				gap = "small",
				ui.row { grow = true, ui.text { props.title, class = "document-card-title", truncate = true } },
				props.star and icon_button {
					icon = "star",
					tooltip = "Favourite",
					size = "small",
					active = props.favorite,
					msg = props.star,
					to = props.to,
				},
			},
			ui.markdown { text = props.text or "", class = "document-card-preview", lines = props.lines or 5, grow = true },
			props.date and ui.text { props.date, class = "document-card-date" },
		}
	end,
}
