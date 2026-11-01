-- Tandem's look: white panes with hairlines between them, a pale lavender
-- sidebar, a violet accent for the assistant, and priorities as coloured
-- edges. Dark mode follows.
return style {
	font_size = 14,
	radius = 8,

	colors = {
		background = { light = "#ffffff", dark = "#1b1b20" },
		foreground = { light = "#1c1c22", dark = "#ececf1" },
		lavender = { light = "#f7f7fc", dark = "#202027" },
		secondary = { light = "#f3f3f7", dark = "#26262e" },
		border = { light = "#e9e9f0", dark = "#30303a" },
		accent = { light = "#5b4cf0", dark = "#8b80ff" },
		["muted-foreground"] = { light = "#7d7d8a", dark = "#9393a0" },
		["list-hover"] = { light = "#00000006", dark = "#ffffff0a" },
		-- The assistant's tint: its check card and its messages.
		tint = { light = "#f5f4fe", dark = "#25233a" },
		["tint-border"] = { light = "#e2dffa", dark = "#383458" },
		-- Priorities, as the list's left edges and the chip's bar.
		high = { light = "#ef5350", dark = "#f26b68" },
		medium = { light = "#f5a524", dark = "#f5b041" },
		low = { light = "#cbc7f4", dark = "#5a5490" },
		-- The week's split: the assistant, the IT team, still open.
		lime = { light = "#c4ef5a", dark = "#a6d63c" },
		["lime-soft"] = { light = "#e2fba6", dark = "#33421a" },
		["lime-ink"] = { light = "#24400a", dark = "#d4f59a" },
		ink = { light = "#1c1c22", dark = "#ececf1" },
		danger = { light = "#e5484d", dark = "#f2555a" },
	},

	rules = {
		pane = { transition = "slide-fade normal ease-out" },
		[".side-pane"] = { background = "lavender" },
		[".list-pane"] = { background = "background" },
		[".detail-pane"] = { background = "background" },
	},
}
