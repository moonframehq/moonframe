-- Tessel's look: a warm off-white page beside a slightly greyer sidebar, a
-- hairline between them, quiet grey buttons, cards whose footer sits on a
-- grey band, and one orange for the brand. Dark mode follows.
return style {
	font_size = 14,
	radius = 8,

	colors = {
		background = { light = "#fdfdfc", dark = "#19191b" },
		sidebar = { light = "#f8f8f7", dark = "#141416" },
		card = { light = "#ffffff", dark = "#1f1f22" },
		secondary = { light = "#f4f4f3", dark = "#26262a" },
		muted = { light = "#f4f4f3", dark = "#26262a" },
		band = { light = "#f8f8f7", dark = "#232326" },
		border = { light = "#ebebea", dark = "#2e2e33" },
		foreground = { light = "#1d1d1f", dark = "#ececee" },
		["muted-foreground"] = { light = "#8b8b8b", dark = "#8e8e96" },
		["list-hover"] = { light = "#0000000a", dark = "#ffffff0d" },
		accent = { light = "#f2542d", dark = "#ff6a45" },
		brand = "#f2542d",
		-- The five states a run can be in.
		["run-active"] = { light = "#3a6cf4", dark = "#6b93ff" },
		["run-completed"] = { light = "#1f9d55", dark = "#3ec27a" },
		["run-review"] = { light = "#b02fd6", dark = "#cf6cf0" },
		["run-pending"] = { light = "#d29b0b", dark = "#e7b631" },
		["run-failed"] = { light = "#e5484d", dark = "#ff6b6f" },
		danger = { light = "#e5484d", dark = "#ff6b6f" },
		success = { light = "#1f9d55", dark = "#3ec27a" },
	},

	rules = {
		pane = { transition = "slide-fade normal ease-out" },
		[".side-frame"] = { background = "sidebar", border = "right", border_color = "border" },
		[".main-frame"] = { background = "background" },
		popover = { background = "card", radius = 10, border = true, border_color = "border", shadow = "0 10px 28px -8px #0000002a", transition = "slide-fade fast ease-out" },
		overlay = { background = "card", radius = 14, border = true, border_color = "border", shadow = "0 24px 60px -12px #00000040", transition = "slide-fade fast ease-out" },
		["overlay-backdrop"] = { background = "#000000", opacity = 0.18 },
		-- Quiet grey buttons, as the header's "Create an Agent".
		[".soft-button"] = { background = "secondary", radius = 8, padding = { 6, 11 }, gap = 6 },
		[".soft-button:hover"] = { background = "#ebebea" },
		[".dark-button"] = { background = "foreground", color = "background", radius = 8, padding = { 7, 12 }, gap = 7 },
		-- A field shows only its caret on focus.
		[".bare-input"] = { background = "#00000000", border = "none", shadow = "none", padding = { 4, 0 } },
		[".bare-input:focus"] = { border = "none", shadow = "none" },
	},
}
