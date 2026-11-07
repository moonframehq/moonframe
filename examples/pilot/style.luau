-- Pilot's look: warm neutrals, a white sheet for the conversation beside a
-- soft sidebar, thin borders, and colour only for what state something is
-- in. Themes from View > Themes replace the theme colours.
return style {
	font_family = "Inter",
	font_size = 14,
	radius = 8,
	mono = { family = "JetBrains Mono", size = 12.5, line_height = 1.45 },

	colors = {
		background = { light = "#ffffff", dark = "#1f1f1e" },
		card = { light = "#ffffff", dark = "#262625" },
		sidebar = { light = "#f7f6f4", dark = "#191918" },
		secondary = { light = "#f4f3f0", dark = "#2b2b2a" },
		muted = { light = "#f1f0ed", dark = "#2b2b2a" },
		foreground = { light = "#22211f", dark = "#ecebe8" },
		["muted-foreground"] = { light = "#7c7a75", dark = "#96948f" },
		border = { light = "#e9e7e3", dark = "#333331" },
		["list-hover"] = { light = "#00000008", dark = "#ffffff0c" },
		["list-active"] = { light = "#0000000e", dark = "#ffffff14" },
		accent = { light = "#4c5a9a", dark = "#8f9ddc" },
		success = { light = "#2f9e57", dark = "#55c27c" },
		warning = { light = "#d97a12", dark = "#e8a04a" },
		danger = { light = "#c8384a", dark = "#ee6b7b" },
		added = { light = "#dff0e3", dark = "#26402f" },
		removed = { light = "#f9e0e1", dark = "#472a2c" },
		hunk = { light = "#e9eef9", dark = "#232c3d" },
		bubble = { light = "#f5f4f1", dark = "#2b2b2a" },
		ink = { light = "#22211f", dark = "#ecebe8" },
	},

	rules = {
		input = { background = "#00000000", border = "none", padding = 0 },
		["input-placeholder"] = { color = "muted-foreground" },
		overlay = { background = "card", radius = 14, border = true, shadow = "0 24px 60px -12px #00000055", transition = "slide-fade fast ease-out" },
		["overlay-backdrop"] = { background = "#000000/30" },
		popover = { background = "card", radius = 10, border = true, padding = 4, shadow = "0 12px 32px -8px #00000040", transition = "slide-fade fast ease-out" },

		-- The bars at the top of each pane, one height across the window.
		[".pane-head"] = { padding = { 0, 20 }, border = "bottom" },
		[".crumb"] = { color = "muted-foreground" },
		[".quiet-button"] = { padding = { 6, 6 }, radius = 6, color = "muted-foreground" },
		[".quiet-button:hover"] = { background = "list-hover", color = "foreground" },

		-- Pages that are not a conversation.
		[".page"] = { padding = { 28, 36 }, gap = 20 },
		[".page-title"] = { size = "title", weight = "semibold" },
		[".page-card"] = { background = "card", border = true, radius = 12, padding = { 14, 16 }, gap = 6 },
		[".page-field"] = { background = "card", border = true, radius = 8, padding = { 8, 10 } },
		[".scope-chip"] = { padding = { 1, 7 }, radius = 6, background = "muted", size = "small", color = "muted-foreground" },
	},
}
