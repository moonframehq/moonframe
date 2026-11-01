-- Lantern's look, the Hallows palette: a near-black purple night, panes a
-- shade darker than the gaps between them, pumpkin orange for what has the
-- keys, ember pink for what runs hot. Dark in both modes: it is always
-- night here. The starter ships the same palette as the "Hallows" theme
-- (View > Themes), which also colours the terminals.
local NIGHT = "#0b0214"

return style {
	font_size = 12,
	radius = 6,
	mono = { size = 12, line_height = 1.25 },

	colors = {
		-- The theme's own names, so buttons and inputs follow.
		background = "#0f0818",
		foreground = "#ebe4f2",
		card = "#0f0818",
		sidebar = "#0f0819",
		secondary = "#1c1325",
		muted = "#1c1325",
		["muted-foreground"] = "#8c809c",
		border = "#241a31",
		accent = "#ff8a3d",
		["accent-foreground"] = "#1a0a04",
		success = "#3ddc84",
		warning = "#f5b545",
		danger = "#ff5f6d",
		["list-hover"] = "#ffffff0b",
		["list-active"] = "#261b36",
		-- Lantern's own.
		night = NIGHT,
		gap = "#170c1f",
		pumpkin = "#ff8a3d",
		ember = "#d97767",
		candle = "#ffc24a",
		focus = "#d98a62",
		hex = "#b48cff",
		["row-selected"] = "#261b36",
		field = "#1a1124",
	},

	rules = {
		pane = { transition = "slide-fade normal ease-out" },
		terminal = { background = "card", padding = { 4, 8 } },
		input = { background = "field", border = "none", radius = 6, padding = { 6, 10 } },
		["input-placeholder"] = { color = "muted-foreground" },
		["input-caret"] = { background = "pumpkin" },
		["input-selection"] = { background = "pumpkin/30" },
		tooltip = { background = "#1c1325", color = "foreground", border = true, border_color = "border", radius = 6 },
		popover = {
			background = "#140c1e",
			border = true,
			border_color = "border",
			radius = 8,
			padding = 4,
			shadow = "0 12px 32px -8px #000000aa",
			transition = "slide-fade fast ease-out",
		},

		-- The pane chrome (`pane-frame`): a hairline frame, a 30 px header,
		-- and an ember-orange frame on the pane that has the keys.
		[".pane-frame"] = { background = "card", border = "all", border_color = "border", radius = 6 },
		[".pane-frame-focused"] = { border_color = "focus" },
		[".pane-frame-header"] = { padding = { 0, 6, 0, 10 }, gap = 8, size = "normal" },
		[".pane-frame-header-top"] = { border = "bottom", border_color = "border" },
		[".pane-frame-title"] = { weight = "normal", color = "foreground" },
		[".pane-frame-icon"] = { color = "pumpkin" },
		[".pane-frame-body"] = { padding = 0 },
		[".pane-frame-menu"] = { gap = 4, padding = { 2, 4 }, radius = 5 },
		[".icon-button"] = { color = "muted-foreground" },
		[".icon-button:hover"] = { background = "list-hover", color = "foreground" },

		-- Menus inside popovers.
		[".menu-item:hover"] = { background = "row-selected" },

		-- Diffs: a dark teal wash for added lines, as the reference's.
		[".diff-view"] = { border = "none", radius = 0 },
		[".diff-view-add"] = { background = "#173027" },
		[".diff-view-del"] = { background = "#3a1420" },
		[".diff-view-hunk"] = { background = "row-selected" },
		[".diff-view-number"] = { color = "muted-foreground/70" },
		[".diff-view-sign-add"] = { color = "foreground" },
		[".diff-view-text"] = { color = "foreground" },

		-- The file tree: quiet rows, no card under the selected one.
		[".file-tree-row"] = { padding = { 4, 6 }, radius = 5, gap = 6 },
		[".file-tree-row:selected"] = { background = "row-selected", shadow = "none" },
	},
}
