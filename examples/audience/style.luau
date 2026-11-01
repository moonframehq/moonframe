-- Inkwell's look: a soft grey window, one white card for the page, quiet
-- borders, violet as the accent.
return style {
	font_size = 14,
	radius = 10,

	colors = {
		background = { light = "#ffffff", dark = "#1c1c1f" },
		frame = { light = "#f2f2f3", dark = "#121214" },
		ink = { light = "#26262b", dark = "#3a3a42" },
		accent = { light = "#7c5cff", dark = "#9b84ff" },
		border = { light = "#ececee", dark = "#2c2c31" },
		["list-hover"] = { light = "#0000000a", dark = "#ffffff0d" },
		["muted-foreground"] = { light = "#82828a", dark = "#8e8e96" },
		success = { light = "#16a35a", dark = "#3ccf7e" },
		danger = { light = "#e5484d", dark = "#ff6369" },
	},

	rules = {
		pane = { transition = "slide-fade normal ease-out" },
		[".frame"] = { background = "frame" },
		[".page-card"] = {
			background = "background",
			radius = 16,
			border = true,
			border_color = "border",
			margin = { 0, 18, 18, 0 },
			shadow = "0 1px 2px #0000000a",
		},
	},
}
