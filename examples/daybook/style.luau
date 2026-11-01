-- Daybook's looks. The frame (the window around the cards) and the cards
-- have a colour per look; in dark mode every look is the same dark one.
return style {
	font_size = 14,
	radius = 10,

	colors = {
		["frame-classic"] = { light = "#0f0f10", dark = "#0e0e0f" },
		["frame-sepia"] = { light = "#1d1813", dark = "#0e0e0f" },
		["frame-light"] = { light = "#f5f4f2", dark = "#141416" },
		["frame-ink"] = { light = "#ececec", dark = "#ececec" },
		["frame-ink-light"] = { light = "#1b1b1d", dark = "#ececec" },
		card = { light = "#ffffff", dark = "#202023" },
		["card-sepia"] = { light = "#f6efe2", dark = "#202023" },
		["card-ink-sepia"] = { light = "#2a2118", dark = "#ececec" },
		["list-hover"] = { light = "#0000000a", dark = "#ffffff0d" },
		["muted-foreground"] = { light = "#8a8a8e", dark = "#8e8e93" },
		border = { light = "#e7e5e2", dark = "#2e2e32" },
		danger = { light = "#e5484d", dark = "#ff6369" },
		warning = { light = "#f1a10d", dark = "#ffb224" },
	},

	rules = {
		pane = { transition = "slide-fade normal ease-out" },
		[".chrome-classic"] = { background = "frame-classic", color = "frame-ink" },
		[".chrome-sepia"] = { background = "frame-sepia", color = "frame-ink" },
		[".chrome-light"] = { background = "frame-light", color = "frame-ink-light" },
		[".card-pane"] = { padding = { 0, 8, 12, 0 } },
		[".card-pane-last"] = { padding = { 0, 12, 12, 0 } },
		[".card"] = { radius = 16, shadow = "0 1px 2px #00000010, 0 8px 24px -8px #00000022", color = "foreground" },
		[".card-classic"] = { background = "card" },
		[".card-light"] = { background = "card", border = true, border_color = "border" },
		[".card-sepia"] = { background = "card-sepia", color = "card-ink-sepia" },
	},
}
