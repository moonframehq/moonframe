-- Ledger's look: a grey window, a white sheet, mono figures, a blue accent.
return style {
	font_size = 14,
	radius = 10,

	colors = {
		accent = { light = "#2f6bf0", dark = "#5b8cff" },
		background = { light = "#ffffff", dark = "#1b1c1f" },
		secondary = { light = "#f2f3f5", dark = "#232428" },
		frame = { light = "#ebecee", dark = "#121315" },
		border = { light = "#e4e5e8", dark = "#2e3035" },
		["muted-foreground"] = { light = "#7b7f87", dark = "#8d9199" },
		danger = { light = "#d92d3a", dark = "#ff6369" },
		success = { light = "#1f8a4c", dark = "#3fbf74" },
	},

	rules = {
		pane = { transition = "slide-fade normal ease-out" },
		[".frame"] = { background = "frame" },
		[".sheet-pane"] = { padding = { 8, 8, 8, 0 } },
		[".sheet"] = { background = "background", radius = 18, border = true, border_color = "border", shadow = "0 1px 2px #0000000a" },
	},
}
