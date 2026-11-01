-- Gantry's buttons: an icon and capitals in a square box, dark (bordered),
-- light (white, for the main action) or quiet (a raised grey). Gantry only.

type Props = {
	label: string?,
	icon: string?,
	variant: string?,
	msg: any,
	to: string?,
	disabled: boolean?,
	busy: boolean?,
	small: boolean?,
	tooltip: string?,
}

return component {
	title = "Square Button",
	category = "Controls",
	icon = "square",
	description = "A square button with an icon and an uppercase label, dark, light or quiet.",
	when = "Actions in Gantry's headers and sections. Elsewhere use `ui.button`.",
	props = {
		["label"] = "the text, shown in capitals",
		["icon"] = "icon before it",
		["variant"] = "dark (default), light or quiet",
		["msg"] = "message a click sends",
		["to"] = "mod to send it to",
		["disabled"] = "greyed out, no clicks",
		["busy"] = "a spinner instead of the icon, no clicks",
		["small"] = "a smaller box (an icon alone)",
		["tooltip"] = "text under the pointer",
	},
	styles = {
		[".square-button"] = { padding = { 0, 12 }, gap = 8, border = true, border_color = "border", background = "#1c1c1c", color = "foreground" },
		[".square-button:hover"] = { background = "#252525", border_color = "#3a3a3a" },
		[".square-button-light"] = { background = "#f2f2f2", border_color = "#f2f2f2", color = "#111111" },
		[".square-button-light:hover"] = { background = "#ffffff", border_color = "#ffffff" },
		[".square-button-quiet"] = { background = "#222222", border_color = "#222222" },
		[".square-button-quiet:hover"] = { background = "#2a2a2a", border_color = "#2a2a2a" },
		[".square-button-off"] = { opacity = 0.55 },
		[".square-button-small"] = { padding = 0 },
		[".square-button-text"] = { weight = "medium" },
	},
	example = { label = "New deploy", icon = "plus", variant = "light" },

	build = function(props: Props)
		local class = { "square-button" }
		if props.variant == "light" then
			table.insert(class, "square-button-light")
		elseif props.variant == "quiet" then
			table.insert(class, "square-button-quiet")
		end
		if props.disabled or props.busy then
			table.insert(class, "square-button-off")
		end
		if props.small then
			table.insert(class, "square-button-small")
		end
		local icon = if props.busy
			then ui.icon { name = "loader-circle", size = "small", spin = true }
			elseif props.icon then ui.icon { name = props.icon :: any, size = "small" }
			else nil
		return ui.row {
			class = class,
			height = if props.small then 26 else 32,
			width = if props.small then 26 else nil,
			align = "center",
			justify = if props.small then "center" else nil,
			shrink = false,
			msg = if props.disabled or props.busy then nil else props.msg,
			to = props.to,
			tooltip = props.tooltip,
			icon,
			props.label and ui.text { string.upper(props.label), class = "square-button-text", size = "small", wrap = false },
		}
	end,
}
