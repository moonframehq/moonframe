-- Mailbox's look: a soft green-grey frame, a white sheet, a teal accent;
-- charcoal in the dark.
return style {
	font_size = 14,
	radius = 8,

	colors = {
		accent = { light = "#0f9488", dark = "#2dd4bf" },
		foreground = { light = "#262b29", dark = "#ececec" },
		["muted-foreground"] = { light = "#717875", dark = "#9b9b9b" },
		background = { light = "#ffffff", dark = "#1b1b1b" },
		sidebar = { light = "#f4f6f5", dark = "#232323" },
		card = { light = "#ffffff", dark = "#303030" },
		muted = { light = "#e8ecea", dark = "#363636" },
		border = { light = "#e3e7e5", dark = "#3a3a3a" },
		["list-hover"] = { light = "#e9edeb", dark = "#2c2c2c" },
	},

	rules = {
		[".frame"] = { background = "sidebar" },
		-- 2 at the top puts the toggle's centre on the traffic lights' (y 14, 14 high).
		[".titlebar"] = { padding = { 2, 10, 0, 10 }, gap = 6 },
		[".titlebar-button"] = { radius = 6, padding = 5, color = "muted-foreground" },
		[".titlebar-button:hover"] = { background = "list-hover", color = "foreground" },
		-- The sidebar sits on the frame: no line of its own.
		[".side-nav"] = { border = "none" },
		[".side-nav-head"] = { border = "none" },
		[".sheet-pane"] = { padding = { 0, 8, 8, 0 } },
		[".sheet"] = { background = "background", radius = 12, border = true, border_color = "border", shadow = "0 1px 2px #0000000a" },
	},
}
