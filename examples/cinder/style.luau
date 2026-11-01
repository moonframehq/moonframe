-- Cinder's look: dark, quiet and flat. A charcoal sidebar beside a nearly
-- black canvas, light text, raised rows in a lighter grey, one blue for
-- links and one orange for the mascot. The app is dark in both modes.
return style {
	font_size = 14,
	radius = 8,

	colors = {
		background = "#191919",
		foreground = "#efefef",
		sidebar = "#262627",
		card = "#303030",
		secondary = "#313132",
		muted = "#2a2a2b",
		border = "#3a3a3b",
		["muted-foreground"] = "#a1a1a2",
		accent = "#4f8fe8",
		["accent-foreground"] = "#ffffff",
		success = "#6bd45f",
		["list-hover"] = "#ffffff0d",
		["list-active"] = "#ffffff14",
		-- Cinder's own.
		["em-raised"] = "#464647",
		["em-faint"] = "#7c7c7d",
		["em-link"] = "#6ea8fe",
		["em-line"] = "#2c2c2d",
		["em-field"] = "#303030",
		["em-field-line"] = "#404040",
		["em-cinder"] = "#d9774b",
	},

	rules = {
		pane = { transition = "slide-fade normal ease-out" },
		tooltip = { background = "#3a3a3b", color = "foreground", border = true, border_color = "#4a4a4b", radius = 6, padding = { 4, 8 } },
		popover = {
			background = "#2b2b2c",
			border = true,
			border_color = "#3e3e3f",
			radius = 12,
			padding = 6,
			shadow = "0 12px 32px -8px #000000aa",
			transition = "slide-fade fast ease-out",
		},
		["input-placeholder"] = { color = "#8e8e8f" },
		["input-caret"] = { background = "foreground" },
		["input-selection"] = { background = "accent/35" },
		-- Fields show only their caret: no frame, no focus ring.
		[".em-input"] = { background = "#00000000", border = "none", shadow = "none", padding = { 4, 0 } },

		-- The two halves of the window.
		[".em-side"] = { background = "sidebar" },
		[".em-main"] = { background = "background" },

		-- Title bar.
		[".em-titlebar"] = { padding = { 0, 12 }, gap = 6 },
		[".em-tool"] = { radius = 6, color = "#c4c4c5", padding = 5 },
		[".em-tool:hover"] = { background = "list-hover", color = "foreground" },
		[".em-tool-off"] = { color = "#5c5c5d" },
		[".em-modes"] = { background = "secondary", radius = 8, padding = 2, gap = 2 },
		[".em-mode"] = { radius = 6, color = "#b8b8b9", padding = { 5, 9 } },
		[".em-mode:hover"] = { color = "foreground" },
		[".em-mode:selected"] = { background = "#454546", color = "foreground", shadow = "0 1px 1px #00000066" },
		[".em-title"] = { color = "muted-foreground" },
		[".em-link"] = { color = "em-link", radius = 6, padding = { 4, 6 } },
		[".em-link:hover"] = { background = "list-hover" },
	},
}
