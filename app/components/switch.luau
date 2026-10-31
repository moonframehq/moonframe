-- An on/off switch: a pill track with a round knob, and an optional label
-- and description beside it. The whole row is clickable and sends `msg`.

type Props = {
	on: boolean?,
	label: string?,
	description: string?,
	msg: any,
	disabled: boolean?,
	small: boolean?,
	to: string?,
}

return component {
	title = "Switch",
	category = "Forms",
	icon = "power",
	description = "An on/off switch (pill track and knob) with an optional label and description; a click sends `msg`.",
	when = "A setting that takes effect at once, on or off (\"High contrast\"). For a choice that is confirmed later or part of a list of options use `checkbox`; for one of several values use `segmented` or `radio-group`.",
	props = {
		["on"] = "true when switched on",
		["label"] = "text beside the switch",
		["description"] = "muted line under the label",
		["msg"] = "message a click sends (the mod flips its state)",
		["disabled"] = "greys it out and ignores clicks",
		["small"] = "a smaller track",
		["to"] = "mod to send `msg` to, if not the drawing one",
	},
	styles = {
		[".switch"] = { gap = 12, padding = { 2, 0 } },
		[".switch-disabled"] = { opacity = 0.5 },
		[".switch-label"] = { weight = "medium" },
		[".switch-description"] = { color = "muted-foreground", size = "small" },
		[".switch-track"] = { background = "muted-foreground/30", radius = 999, padding = 2 },
		[".switch-track:selected"] = { background = "accent" },
		[".switch-knob"] = { background = "#ffffff", radius = 999, shadow = "0 1 2 #00000040" },
	},
	example = {
		on = true,
		label = "Follow system theme",
		description = "Switch between light and dark with macOS.",
		msg = "toggle-follow",
	},

	build = function(props: Props)
		local width, height = 34, 20
		if props.small then
			width, height = 26, 16
		end
		local knob = height - 4
		local track = ui.row {
			class = "switch-track",
			width = width,
			height = height,
			align = "center",
			justify = if props.on then "end" else "start",
			selected = props.on == true,
			ui.row { class = "switch-knob", width = knob, height = knob },
		}
		local class = { "switch" }
		if props.disabled then
			table.insert(class, "switch-disabled")
		end
		local text = (props.label or props.description)
			and ui.column {
				gap = "none",
				grow = true,
				props.label and ui.text { props.label, class = "switch-label" },
				props.description and ui.text { props.description, class = "switch-description" },
			}
		return ui.row {
			class = class,
			align = "center",
			msg = if props.disabled then nil else props.msg,
			to = props.to,
			text,
			track,
		}
	end,
}
