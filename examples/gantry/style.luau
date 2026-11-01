-- Gantry's look: near-black, one monospace face for everything, square
-- corners and thin grey rules; colour only where it means something (CPU
-- blue, GPU green, memory orange, states).
return style {
	font_size = 14,
	font_family = "BlexMono Nerd Font Mono",
	mono = { family = "BlexMono Nerd Font Mono", size = 13 },
	radius = 0,

	colors = {
		background = "#161616",
		foreground = "#e6e6e6",
		card = "#161616",
		sidebar = "#161616",
		secondary = "#1f1f1f",
		muted = "#1f1f1f",
		border = "#2a2a2a",
		["muted-foreground"] = "#8a8a8a",
		accent = "#3b82f6",
		success = "#22c55e",
		warning = "#eab308",
		danger = "#ef4444",
		["list-hover"] = "#1d1d1d",
		raised = "#1f1f1f",
		hairline = "#262626",
		bright = "#f2f2f2",
		cpu = "#3b82f6",
		gpu = "#22c55e",
		memory = "#f97316",
	},

	rules = {
		[".window"] = { background = "background", color = "foreground" },
		[".topline"] = { border = "bottom", border_color = "border" },
		tooltip = { background = "#202020", border = true, border_color = "border", radius = 0, color = "foreground" },
		popover = { background = "#1a1a1a", radius = 0, border = true, border_color = "border", padding = 4, shadow = "0 12px 32px -8px #000000aa", transition = "fade fast" },
		overlay = { background = "#181818", radius = 0, border = true, border_color = "border" },
		input = { background = "#1a1a1a", border = true, border_color = "border", radius = 0, padding = { 6, 10 } },
		["input:hover"] = { border_color = "#3a3a3a" },
		-- Switches go square, as the rest.
		[".switch-track"] = { radius = 0, background = "#262626", padding = 2 },
		[".switch-track:selected"] = { background = "#3a3a3a" },
		[".switch-knob"] = { radius = 0, background = "#e6e6e6", shadow = "none" },
		[".menu-item"] = { radius = 0 },
		[".dropdown-trigger"] = { radius = 0, padding = { 6, 10, 6, 12 }, background = "background" },
		[".dropdown-trigger:selected"] = { border_color = "#4a4a4a" },
		[".dropdown-popover"] = { radius = 0 },
		[".stepper-group"] = { radius = 0 },
		[".stepper-button"] = { radius = 0 },
		[".sheet-close"] = { radius = 0 },
	},
}
