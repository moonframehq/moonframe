return style {
	colors = {
		accent = { light = "#4f46e5", dark = "#818cf8" },
		brand = "#ff3366",
	},
	rules = {
		panel = { background = "card", radius = 12, padding = "normal", border = true },
		[".stat"] = { background = "accent/8", border_color = "border" },
		[".stat-label"] = { color = "muted-foreground" },
		["item:selected"] = { background = "accent/15", color = "accent" },
		["item:hover"] = { background = "accent/8" },
		["button:disabled"] = { opacity = 0.5 },
	},
}
