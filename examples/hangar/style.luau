-- Hangar's look: quiet cards on a dark-leaning ground, so the status of each
-- bay is the brightest thing in the window.
return style {
	font_size = 14,
	radius = 10,

	colors = {
		background = { light = "#eceef2", dark = "#131519" },
		card = { light = "#ffffff", dark = "#1c1f25" },
		sidebar = { light = "#eceef2", dark = "#131519" },
		accent = { light = "#2563eb", dark = "#7aa2ff" },
		["list-hover"] = { light = "#0000000a", dark = "#ffffff0d" },
		["muted-foreground"] = { light = "#6f7683", dark = "#8a93a3" },
		border = { light = "#dfe2e8", dark = "#2a2e36" },
	},

	rules = {
		[".side"] = { background = "background" },
		[".middle"] = { background = "background", padding = { 10, 0, 10, 0 } },
		panel = { background = "card", radius = 12, margin = { 10, 10 }, border = true },
		terminal = { background = "card" },
	},
}
