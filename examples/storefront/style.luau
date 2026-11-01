-- Storefront's look: a pale grey window and title bar, one white sheet,
-- hairline borders, and an orange accent kept for the current page.
return style {
	font_size = 14,
	radius = 8,

	colors = {
		background = { light = "#ffffff", dark = "#1c1c1e" },
		foreground = { light = "#1f1f22", dark = "#ececee" },
		frame = { light = "#f3f3f3", dark = "#121213" },
		accent = { light = "#e5683f", dark = "#f08a62" },
		border = { light = "#e7e7e9", dark = "#2d2d31" },
		["list-hover"] = { light = "#00000008", dark = "#ffffff0d" },
		["list-active"] = { light = "#e5683f12", dark = "#f08a621f" },
		["muted-foreground"] = { light = "#85858c", dark = "#8f8f97" },
	},

	rules = {
		pane = { transition = "slide-fade normal ease-out" },
		[".frame"] = { background = "frame" },
		[".sheet-pane"] = { padding = { 0, 10, 10, 2 } },
		[".sheet"] = { background = "background", radius = 12, border = true, border_color = "border", shadow = "0 1px 2px #00000008" },
	},
}
