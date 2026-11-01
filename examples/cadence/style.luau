-- Cadence's look: a near-white sidebar, a soft grey work area, white cards
-- with a hairline border, quiet grey text and one dark button. Dark mode
-- follows.
return style {
	font_size = 13,
	radius = 8,

	colors = {
		side = { light = "#fcfcfc", dark = "#18181b" },
		work = { light = "#f6f6f6", dark = "#111113" },
		background = { light = "#ffffff", dark = "#1d1d20" },
		card = { light = "#ffffff", dark = "#1d1d20" },
		muted = { light = "#f2f2f3", dark = "#26262a" },
		secondary = { light = "#f6f6f7", dark = "#26262a" },
		border = { light = "#ececee", dark = "#2e2e33" },
		foreground = { light = "#1f1f22", dark = "#ececee" },
		["muted-foreground"] = { light = "#8b8b92", dark = "#8e8e96" },
		["list-hover"] = { light = "#00000008", dark = "#ffffff0d" },
		ink = { light = "#232326", dark = "#f2f2f3" },
		good = { light = "#22a35a", dark = "#4ade80" },
		bad = { light = "#e5484d", dark = "#f87171" },
		amber = { light = "#d4a017", dark = "#facc15" },
	},

	rules = {
		pane = { transition = "slide-fade normal ease-out" },
		[".side-pane"] = { background = "side", border = "right", border_color = "border" },
		[".main-pane"] = { background = "work" },
		[".dark-button"] = { background = "ink", color = "background", radius = 8 },
		["overlay-backdrop"] = { background = "#f4f4f5", opacity = 0.6 },
	},
}
