-- Hearth's look: warm black, a darker sidebar, cards a shade lighter with
-- hairline borders, monospace everywhere but the page title, and a burnt
-- orange for what is current. Dark in both modes: it is an observability
-- tool, read in a dim room.
return style {
	font_size = 13,
	radius = 7,
	mono = { family = "Menlo", size = 12.5 },

	colors = {
		background = "#1b1714",
		sidebar = "#16120f",
		card = "#1e1b16",
		raised = "#27241f",
		foreground = "#e7e2dc",
		["muted-foreground"] = "#857e78",
		muted = "#27241f",
		secondary = "#27241f",
		border = "#2a2620",
		accent = "#d98a52",
		["accent-foreground"] = "#16120f",
		["list-hover"] = "#ffffff0a",
		["list-active"] = "#27241f",
		success = "#5ec27a",
		warning = "#e1b64e",
		danger = "#d9604c",
		info = "#619cf8",
		bars = "#c8825e",
	},

	rules = {
		pane = { transition = "slide-fade normal ease-out" },
		tooltip = { background = "#2c2823", color = "foreground", border = true, border_color = "#3a352f", radius = 6, font = "mono" },
		popover = { background = "#211d19", border = true, border_color = "#36312b", radius = 9, shadow = "0 12px 32px -8px #000000aa" },
		[".app"] = { font = "mono" },
		[".frame-side"] = { background = "sidebar", border = "right", border_color = "border" },
		[".frame-main"] = { background = "background" },
		-- Room for the traffic lights before the top bar, carrying its rule.
		[".top-lights"] = { border = "bottom", border_color = "border" },
		[".card"] = { background = "card", border = true, border_color = "border", radius = 6 },
		[".bare-input"] = { background = "#00000000", border = "none", shadow = "none", padding = { 4, 0 } },
		["input-placeholder"] = { color = "muted-foreground" },
	},
}
