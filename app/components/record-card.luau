-- A small card that stands for a record (a deal, a ticket, a file) inside
-- a message or a note: its title, and a line with an icon and its kind.

type Props = {
	title: string,
	detail: string?,
	icon: string?,
	tone: string?,
	msg: any,
	to: string?,
}

return component {
	title = "Record Card",
	category = "Data",
	icon = "file-text",
	description = "A compact card for a linked record: a bold title, then an icon and a muted line saying what it is.",
	when = "A record an agent or a person points at inside a message or a note, which a click opens. For a full row in a list use `list` or `pr-row`; for a dated item use `date-tile` beside text.",
	props = {
		["title"] = "the record's name",
		["detail"] = "what it is, muted (\"Deal · Negotiation · $98,000\")",
		["icon"] = "icon before the detail",
		["tone"] = "the icon's tone (default warning)",
		["msg"] = "message a click sends",
		["to"] = "mod to send it to, if not the drawing one",
	},
	styles = {
		[".record-card"] = { background = "background", border = true, border_color = "border", radius = 10, padding = { 10, 14 }, gap = 4, shadow = "0 1px 2px #0000000a" },
		[".record-card:hover"] = { border_color = "muted-foreground/40" },
		[".record-card-title"] = { weight = "medium" },
		[".record-card-detail"] = { size = "small", color = "muted-foreground", gap = 6 },
	},
	example = { title = "Lumen Retail · Platform renewal", detail = "Deal · Negotiation · $98,000", icon = "building-2" },

	build = function(props: Props)
		return ui.column {
			class = "record-card",
			hug = true,
			msg = props.msg,
			to = props.to,
			ui.text { props.title, class = "record-card-title" },
			props.detail and ui.row {
				class = "record-card-detail",
				align = "center",
				props.icon and ui.icon { name = props.icon :: any, size = "small", tone = (props.tone or "warning") :: any },
				ui.text { props.detail, size = "small" },
			},
		}
	end,
}
