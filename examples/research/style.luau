-- Sorrel's look: a pale grey frame, one white card with soft edges, a light
-- bay behind the sheet, blue for what the agent is doing. Dark mode follows.
return style {
	font_size = 13,
	radius = 8,

	colors = {
		frame = { light = "#f3f3f4", dark = "#141416" },
		bay = { light = "#fafafa", dark = "#19191c" },
		background = { light = "#ffffff", dark = "#1e1e21" },
		secondary = { light = "#f4f4f5", dark = "#27272b" },
		border = { light = "#e8e8eb", dark = "#313136" },
		accent = { light = "#1a7cf5", dark = "#3d93fa" },
		success = { light = "#22b45b", dark = "#34c76c" },
		["muted-foreground"] = { light = "#7a7a82", dark = "#8e8e96" },
		["list-hover"] = { light = "#00000007", dark = "#ffffff0d" },
		["list-active"] = { light = "#e9e9ec", dark = "#2c2c31" },
	},

	rules = {
		pane = { transition = "slide-fade normal ease-out" },
		[".frame"] = { background = "frame" },
		[".topline"] = { border = "bottom", border_color = "border" },
		[".bay"] = { background = "bay" },
		[".card"] = {
			background = "background",
			radius = 14,
			border = true,
			border_color = "border",
			margin = { 8, 8, 8, 0 },
			shadow = "0 1px 2px #0000000a",
		},
		[".bare-input"] = { background = "#00000000", border = "none", shadow = "none", padding = { 5, 0 } },
		[".quiet"] = { color = "muted-foreground" },
		[".small-muted"] = { size = "small", color = "muted-foreground" },
	},
}
