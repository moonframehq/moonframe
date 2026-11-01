-- Trellis's look: a dark, quiet multiplexer. The terminals and the source
-- control sit on one near-black, the projects on a lighter sidebar, the
-- chosen rows in a raised grey, and a soft violet marks what is current.
-- Dark in both modes, as a terminal is: the light theme changes nothing.
return style {
	font_size = 13,
	radius = 7,

	mono = { size = 14, line_height = 1.4 },

	colors = {
		background = "#1c1b22",
		foreground = "#e8e7ec",
		card = "#25242b",
		sidebar = "#25242c",
		secondary = "#323138",
		muted = "#323138",
		border = "#303036",
		["muted-foreground"] = "#8b8a92",
		["list-hover"] = "#ffffff0a",
		accent = "#bb74c6",
		success = "#4cc38a",
		danger = "#e8648f",
		warning = "#e8c34a",
		-- The raised grey of a chosen row.
		raised = "#36353c",
		-- A button's face.
		face = "#3f3e46",
		-- The commit box and the Commit button at rest.
		well = "#27262d",
		["accent-foreground"] = "#ffffff",
		["list-active"] = "#36353c",
	},

	rules = {
		pane = { transition = "slide-fade normal ease-out" },
		[".trellis-side"] = { background = "sidebar", border = "right", border_color = "border" },
		[".trellis-work"] = { background = "background" },
		[".trellis-center"] = { background = "background" },
		[".trellis-right"] = { background = "background", border = "left", border_color = "border" },
		terminal = { background = "background", color = "foreground", padding = { 4, 2, 0, 8 } },
		tooltip = { background = "card", radius = 6 },
		popover = { background = "card", radius = 10, border = true, border_color = "border", padding = 6 },
		-- The status bar under the terminals and source control.
		[".status-bar"] = { background = "background", padding = { 0, 6 }, gap = 0 },
		[".status-bar-group"] = { gap = 0 },
		[".status-bar-item"] = { padding = { 0, 8 }, gap = 6 },
		[".status-bar-separator"] = { background = "border" },
		-- Changed files: the letter in violet, the counts in green and pink.
		[".change-tree-row"] = { padding = { 3, 8 }, radius = 6, gap = 6 },
		[".change-tree-row:selected"] = { background = "raised", shadow = "none" },
		[".change-tree-mark-modified"] = { color = "accent" },
		[".change-tree-added"] = { color = "success" },
		[".change-tree-deleted"] = { color = "danger" },
	},
}
