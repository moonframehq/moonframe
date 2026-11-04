-- Lightbox's look: instruments in cards on the window's own background.
return style {
	rules = {
		[".panel"] = { background = "card", radius = 10, border = "all", border_color = "border", padding = 12, gap = 8 },
		[".panel-title"] = { size = "small", weight = "medium", color = "muted-foreground" },
		[".closing"] = { background = "warning/15", radius = 10, padding = 12, gap = 8 },
		[".top-bar"] = { padding = { 8, 14 }, gap = 10, border = "bottom", border_color = "border" },
	},
}
