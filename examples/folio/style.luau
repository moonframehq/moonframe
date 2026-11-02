-- Folio's look: warm paper and soft ink in light, a dim charcoal in dark,
-- a blue caret, and as little chrome as will do.
return style {
	font_size = 14,
	radius = 8,

	colors = {
		background = { light = "#fbfaf7", dark = "#1c1c1e" },
		foreground = { light = "#262626", dark = "#d9d9d6" },
		sidebar = { light = "#f2f1ed", dark = "#18181a" },
		border = { light = "#e6e4de", dark = "#2c2c2f" },
		["muted-foreground"] = { light = "#8b8a85", dark = "#8a8a8e" },
		accent = { light = "#1a9ad6", dark = "#3fb1ea" },
		["list-hover"] = { light = "#00000008", dark = "#ffffff0a" },
		["list-active"] = { light = "#1a9ad61a", dark = "#3fb1ea24" },
		warning = { light = "#e0a10b", dark = "#f2b630" },
	},

	rules = {
		pane = { transition = "slide-fade normal ease-out" },
		-- The caret is the one bright thing on the page.
		["input-caret"] = { background = "accent" },
		["input-selection"] = { background = "accent/22" },
		["input-placeholder"] = { color = "muted-foreground" },
		markdown = { size = "large" },
	},
}
