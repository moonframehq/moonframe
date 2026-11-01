-- Workbench's look: white cards with round corners on a soft window, quiet
-- controls. Light first, with dark colours for each.
return style {
	font_size = 14,
	radius = 10,

	colors = {
		background = { light = "#eef1ee", dark = "#17191c" },
		card = { light = "#ffffff", dark = "#212327" },
		sidebar = { light = "#eef1ee", dark = "#17191c" },
		["list-hover"] = { light = "#00000009", dark = "#ffffff0d" },
		["muted-foreground"] = { light = "#7a7f87", dark = "#8b9099" },
		border = { light = "#e2e5e2", dark = "#2c2f34" },
	},

	rules = {
		pane = { transition = "slide-fade normal ease-out" },
		[".side"] = { background = "background" },
		[".card-pane"] = { background = "background", padding = { 10, 0, 10, 10 } },
		[".card"] = {
			background = "card",
			radius = 14,
			border = true,
			border_color = "border",
			shadow = "0 1px 2px #0000000d, 0 4px 16px -4px #00000014",
		},
		[".card-header"] = { padding = { 0, 12 }, gap = 6 },
		terminal = { background = "card" },
	},
}
