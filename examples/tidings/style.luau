-- Tidings's look: a pale blue-grey window tint, white cards with soft
-- corners, small grey pills, and a violet dot for what is unread. Dark mode
-- follows.
return style {
	font_size = 13,
	radius = 8,

	colors = {
		frame = { light = "#e3e8f1", dark = "#17181c" },
		background = { light = "#ffffff", dark = "#1f2024" },
		secondary = { light = "#f4f4f5", dark = "#2a2b30" },
		border = { light = "#e6e6ea", dark = "#313238" },
		accent = { light = "#6366f1", dark = "#8b8ef5" },
		["muted-foreground"] = { light = "#71717a", dark = "#9a9aa3" },
		["list-hover"] = { light = "#0000000a", dark = "#ffffff0d" },
		unread = { light = "#6366f1", dark = "#8b8ef5" },
		action = "#f59e0b",
		chrome = { light = "#ffffff99", dark = "#ffffff12" },
		ink = { light = "#3f3f46", dark = "#d4d4d8" },
	},

	rules = {
		[".frame"] = { background = "frame" },
		[".card"] = {
			background = "background",
			radius = 14,
			shadow = "0 1px 2px #0000000a, 0 0 0 1px #00000008",
		},
	},
}
