-- Cadence's look: dark whatever the system's mode, a soft charcoal frame,
-- a darker sheet, and raised charcoal cards; green for healthy, red for
-- what breaches, orange for the one action that matters.
return style {
	font_size = 15,
	radius = 10,

	colors = {
		background = "#222222",
		foreground = "#f4f4f4",
		card = "#2e2e2e",
		frame = "#2b2b2b",
		sidebar = "#2b2b2b",
		muted = "#333333",
		["muted-foreground"] = "#9a9a9a",
		secondary = "#333333",
		border = "#3a3a3a",
		["list-hover"] = "#ffffff0d",
		["list-active"] = "#ffffff14",
		accent = "#ff7a45",
		["accent-foreground"] = "#ffffff",
		success = "#5fd35a",
		danger = "#f2605d",
		warning = "#f5a524",
		chip = "#363636",
		["chip-hover"] = "#3e3e3e",
		label = "#c9c9c9",
	},

	rules = {
		pane = { transition = "slide-fade normal ease-out" },
		tooltip = { background = "#3a3a3a", color = "foreground", radius = 8 },
		[".frame"] = { background = "frame" },
		[".sheet"] = {
			background = "background",
			radius = 18,
			margin = { 10, 10, 10, 0 },
		},
	},
}
