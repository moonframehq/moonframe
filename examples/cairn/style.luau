-- cairn's look: near-black window, two dark cards with thin edges, grey
-- text with white for what matters, and one blue for unread. It is a dark
-- app, so each colour has one value.
return style {
	font_size = 14,
	font_family = "Inter",
	radius = 8,

	mono = { size = 13 },

	colors = {
		background = "#0b0b0c",
		sidebar = "#0b0b0c",
		card = "#171717",
		secondary = "#242424",
		muted = "#242424",
		foreground = "#ececec",
		["muted-foreground"] = "#8f8f8f",
		border = "#272727",
		accent = "#4c5dff",
		success = "#4ade80",
		["list-hover"] = "#ffffff0d",
		["list-active"] = "#ffffff14",
		raised = "#232323",
		tile = "#8c8c8c",
		link = "#a9bcff",
	},

	rules = {
		pane = { transition = "slide-fade normal ease-out" },
		[".side"] = { background = "background", padding = { 0, 6, 6, 6 } },
		[".middle"] = { background = "background", padding = { 0, 6, 8, 0 } },
		[".right"] = { background = "background", padding = { 0, 8, 8, 6 } },
		[".card"] = { background = "card", radius = 12, border = true, border_color = "border", clip = true },

		[".title-bar"] = { background = "background", border = "none", padding = { 0, 8 } },
		[".status-bar"] = { background = "background", border = "top", border_color = "border", padding = { 5, 12 } },

		-- The composer's and the search's fields draw no box of their own;
		-- `.field` is the boxed kind.
		input = { background = "#00000000", border = "none", padding = 0 },
		[".field"] = { background = "raised", radius = 6, padding = { 7, 10 } },
		["input-caret"] = { background = "foreground" },
		["input-selection"] = { background = "accent/40" },

		["markdown-code"] = { background = "#1c1c1c", border = true, border_color = "border", radius = 6, padding = { 10, 12 }, font = "mono" },

		popover = { background = "#1f1f1f", radius = 10, border = true, border_color = "border", shadow = "0 12px 32px -8px #000000aa", transition = "slide-fade fast ease-out" },
		overlay = { background = "#1c1c1c", radius = 12, border = true, border_color = "border", shadow = "0 24px 64px -12px #000000cc", transition = "slide-fade fast ease-out" },
		["overlay-backdrop"] = { background = "#000000/50" },
		tooltip = { background = "#2a2a2a", color = "foreground", radius = 6, padding = { 4, 8 }, transition = "fade fast 400ms" },

		-- A project's letter, in the sidebar and the chat's header.
		[".tile"] = { background = "tile", color = "#111111", radius = 4, size = "small", weight = "semibold" },

		[".tabs-pills-tab"] = { radius = 7, padding = { 6, 11 } },
		[".tabs-pills-tab:selected"] = { background = "#2a2a2a", color = "foreground" },
		[".tabs-badge"] = { background = "#00000000", padding = 0, size = "normal", color = "muted-foreground" },

		[".stat-tile"] = { background = "raised", border = "none" },

		[".dropdown-trigger"] = { background = "#00000000", border = "none", padding = { 5, 8 }, color = "#c8c8c8" },
	},
}
