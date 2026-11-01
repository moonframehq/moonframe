-- Duly's look: a cool grey window, cards one step lighter with white
-- insides, quiet borders, small mono capitals for section heads, and a
-- muted steel blue for every bar, with red, amber and green for state.
return style {
	font_size = 14,
	radius = 8,

	colors = {
		background = { light = "#ffffff", dark = "#1c1d20" },
		foreground = { light = "#1b1c1f", dark = "#e8e9ec" },
		frame = { light = "#e9eaec", dark = "#121315" },
		side = { light = "#efeff1", dark = "#17181a" },
		secondary = { light = "#f4f4f5", dark = "#202124" },
		border = { light = "#dcdde1", dark = "#2e3035" },
		hairline = { light = "#e9eaed", dark = "#26282c" },
		accent = { light = "#3f6fb5", dark = "#7aa3dc" },
		["muted-foreground"] = { light = "#6d7077", dark = "#8f939b" },
		["list-hover"] = { light = "#0000000a", dark = "#ffffff0d" },
		danger = { light = "#d6462c", dark = "#ff6a52" },
		success = { light = "#2f8a50", dark = "#4cc17a" },
		warning = { light = "#c49a1f", dark = "#e0b84a" },
		-- The chart palette the provider's data names.
		blue = { light = "#6f9bd1", dark = "#6f9bd1" },
		ink = { light = "#3f73b5", dark = "#7aa3dc" },
		red = { light = "#d6462c", dark = "#ff6a52" },
		amber = { light = "#c49a1f", dark = "#e0b84a" },
		green = { light = "#2f8a50", dark = "#4cc17a" },
		grey = { light = "#8e9096", dark = "#80838a" },
		track = { light = "#dcdde1", dark = "#33353a" },
		insight = { light = "#e7eef8", dark = "#1d2633" },
		["insight-line"] = { light = "#c7d6ec", dark = "#2c3a4f" },
		["insight-text"] = { light = "#3d4350", dark = "#c3c9d4" },
	},

	rules = {
		pane = { transition = "slide-fade normal ease-out" },
		tooltip = { background = "foreground", color = "background", radius = 6, padding = { 4, 8 } },
		[".frame"] = { background = "frame" },
		[".side-pane"] = { background = "side", border = "right", border_color = "border" },
		-- The shared dashboard card, in this app's colours with mono heads.
		[".dashboard-card"] = { background = "secondary", radius = 12, padding = { 0, 5, 5, 5 } },
		[".dashboard-card-head"] = { padding = { 9, 10, 8, 12 } },
		[".dashboard-card-title"] = { font = "mono", weight = "normal", size = "small", color = "muted-foreground" },
		[".dashboard-card-body"] = { radius = 9, padding = 16, gap = 12 },
		[".insight-note"] = { background = "insight", border_color = "insight-line", radius = 8 },
		[".insight-note-text"] = { color = "insight-text" },
	},
}
