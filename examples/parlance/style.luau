-- Parlance's look: dark only. A lighter frame (title bar and rail) around
-- the sidebar and the conversation, quiet grey controls, one blue accent.
-- App-local classes start with `pl-`.
return style {
	font_size = 15,
	radius = 8,

	colors = {
		frame = "#424242",
		sidebar = "#2d2d2d",
		background = "#1f1f1f",
		foreground = "#f2f2f2",
		["muted-foreground"] = "#a3a3a3",
		secondary = "#353535",
		card = "#2d2d2d",
		border = "#363636",
		accent = "#5c96f2",
		["accent-foreground"] = "#ffffff",
		["list-hover"] = "#ffffff0d",
		bubble = "#3a3a3a",
		field = "#424242",
	},

	rules = {
		pane = { transition = "slide-fade normal ease-out" },
		tooltip = { background = "#111111", color = "foreground", radius = 6, padding = { 4, 8 } },
		popover = { background = "#353535", radius = 12, border = true, border_color = "#ffffff14", padding = 6, shadow = "0 12px 32px -8px #000000aa" },

		[".pl-frame"] = { background = "frame" },
		[".pl-titlebar"] = { padding = { 0, 10, 0, 0 } },
		[".pl-title"] = { size = "large", weight = "medium", padding = { 0, 0, 0, 13 } },
		[".pl-bar-rule"] = { width = 1, height = 26, background = "#ffffff1a" },
		[".pl-bar-button"] = { radius = 7, padding = { 5, 7 }, gap = 6, color = "#c8c8c8" },
		[".pl-bar-button:hover"] = { background = "#ffffff12", color = "foreground" },
		[".pl-bar-button-off"] = { color = "#7a7a7a" },
		[".pl-box-plus"] = { border = true, border_color = "#c8c8c8", radius = 4 },

		-- The text field in the composer: no frame of its own, the caret only.
		[".pl-input"] = { background = "#00000000", border = "none", shadow = "none", padding = { 4, 0 } },
		["input-placeholder"] = { color = "#9a9a9a" },
	},
}
