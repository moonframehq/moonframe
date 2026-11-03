-- Switchyard's look: a soft window behind one white card, round corners,
-- quiet controls, and terminal colours for shell output. Light first, with
-- dark colours for each.
return style {
	font_size = 14,
	radius = 10,

	colors = {
		background = { light = "#efefec", dark = "#17181b" },
		card = { light = "#ffffff", dark = "#212226" },
		sidebar = { light = "#efefec", dark = "#17181b" },
		secondary = { light = "#f4f4f1", dark = "#2a2b30" },
		["list-hover"] = { light = "#00000009", dark = "#ffffff0d" },
		["muted-foreground"] = { light = "#7b7d84", dark = "#8c8f98" },
		border = { light = "#e4e4e0", dark = "#2e3035" },
		accent = { light = "#7c5cff", dark = "#a08bff" },
		-- Shell output, by the 16 colours programs pick.
		["ansi-black"] = { light = "#3b3d42", dark = "#5c5f66" },
		["ansi-red"] = { light = "#c4314b", dark = "#ff6b81" },
		["ansi-green"] = { light = "#2e8b3a", dark = "#6fd17a" },
		["ansi-yellow"] = { light = "#b7791f", dark = "#e8c06a" },
		["ansi-blue"] = { light = "#2f6fd6", dark = "#6ea8ff" },
		["ansi-magenta"] = { light = "#a33ea1", dark = "#e07ee0" },
		["ansi-cyan"] = { light = "#1d8a99", dark = "#5fd0df" },
		["ansi-white"] = { light = "#8a8d93", dark = "#c9ccd2" },
		["ansi-bright-black"] = { light = "#8a8d93", dark = "#7d8089" },
		["ansi-bright-red"] = { light = "#e0445f", dark = "#ff8c9e" },
		["ansi-bright-green"] = { light = "#3aa648", dark = "#8ee398" },
		["ansi-bright-yellow"] = { light = "#c98a1c", dark = "#f5d58c" },
		["ansi-bright-blue"] = { light = "#4b86e8", dark = "#93bfff" },
		["ansi-bright-magenta"] = { light = "#bf55bd", dark = "#f0a0f0" },
		["ansi-bright-cyan"] = { light = "#26a2b3", dark = "#86e2ee" },
		["ansi-bright-white"] = { light = "#3b3d42", dark = "#eef0f3" },
	},

	rules = {
		pane = { transition = "slide-fade normal ease-out" },
		[".side"] = { background = "background" },
		[".middle"] = { background = "background", padding = { 10, 0, 10, 0 } },
		[".card"] = {
			background = "card",
			radius = 14,
			border = true,
			border_color = "border",
			shadow = "0 1px 2px #0000000d, 0 4px 16px -4px #00000014",
			clip = true,
		},
		[".console-pane"] = { border = "top", border_color = "border" },
		terminal = { background = "card" },
		popover = { background = "card", radius = 12, border = true, border_color = "border", shadow = "0 12px 32px -8px #0000002e", transition = "slide-fade fast ease-out" },
	},
}
