-- Tally's look: a pale grey frame, one white sheet with soft edges, quiet
-- controls, dark primary buttons. Dark mode follows.
return style {
	font_size = 14,
	radius = 8,

	colors = {
		frame = { light = "#f4f4f5", dark = "#141416" },
		background = { light = "#ffffff", dark = "#1d1d20" },
		secondary = { light = "#f6f6f7", dark = "#26262a" },
		border = { light = "#e7e7ea", dark = "#303035" },
		accent = { light = "#1d9bf0", dark = "#3aa9f5" },
		["muted-foreground"] = { light = "#7c7c84", dark = "#8e8e96" },
		["list-hover"] = { light = "#00000008", dark = "#ffffff0d" },
	},

	rules = {
		pane = { transition = "slide-fade normal ease-out" },
		[".frame"] = { background = "frame" },
		[".sheet"] = {
			background = "background",
			radius = 14,
			border = true,
			border_color = "border",
			margin = { 0, 10, 10, 0 },
			shadow = "0 1px 2px #00000008",
		},
		[".dark-button"] = { background = "foreground", color = "background", radius = 8 },
	},
}
