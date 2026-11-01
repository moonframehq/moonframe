-- Emberline's look: near black, cards one step lighter with hairline
-- borders, figures in a monospace face, orange for what matters. Dark in
-- both modes: the report is designed for the dark.
return style {
	font_size = 14,
	radius = 10,
	mono = { family = "GeistMono Nerd Font Mono", size = 13 },

	colors = {
		background = "#171717",
		foreground = "#f4f4f4",
		sidebar = "#171717",
		card = "#1b1b1b",
		muted = "#222222",
		secondary = "#262626",
		border = "#262626",
		accent = "#ff8e16",
		["accent-foreground"] = "#171717",
		["muted-foreground"] = "#9a9a9a",
		["list-hover"] = "#ffffff0a",
		["list-active"] = "#ffffff12",
		success = "#00e26e",
		danger = "#fe6367",
		warning = "#ffb020",
		faint = "#6e6e6e",
		chip = "#232323",
	},

	rules = {
		pane = { transition = "slide-fade normal ease-out" },
		tooltip = { background = "#262626", color = "foreground", border = true, border_color = "#333333", radius = 8 },
		[".frame"] = { background = "background" },
		[".main-pane"] = { border = "left", border_color = "border" },
		[".topbar"] = { border = "bottom", border_color = "border" },
		[".card"] = { background = "card", border = true, border_color = "border", radius = 16 },
		[".mono"] = { font = "mono" },
		[".up"] = { color = "success", font = "mono" },
		[".down"] = { color = "danger", font = "mono" },
		[".note"] = { color = "muted-foreground", font = "mono" },
	},
}
