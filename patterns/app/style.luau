return style {
	colors = {
		accent = { light = "#557345", dark = "#b1ca9d" },
		sidebar = { light = "#f5f5f2", dark = "#191c19" },
		card = { light = "#ffffff", dark = "#222622" },
	},
	rules = {
		[".patterns-sidebar"] = { background = "sidebar", padding = 20, gap = 18, border = "right" },
		[".patterns-page"] = { padding = 28, gap = 20 },
		[".patterns-tile"] = { border = "all", border_color = "border", radius = 10, padding = 16, gap = 12 },
		[".patterns-tile:hover"] = { border_color = "accent" },
		[".patterns-preview"] = { background = "sidebar", padding = 20, radius = 8, border = "all", border_color = "border" },
		[".patterns-heading"] = { size = "title", weight = "semibold" },
		[".patterns-nav"] = { padding = { 8, 10 }, radius = 6, color = "muted-foreground" },
		[".patterns-nav:selected"] = { background = "card", color = "foreground" },
		[".patterns-nav:hover"] = { background = "list-hover" },
	},
}
