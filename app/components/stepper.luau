-- A number with − and + buttons joined into one bordered control, like a
-- font size or a pane count. The ends grey out at `min` and `max`.

type Props = {
	value: number,
	unit: string?,
	min: number?,
	max: number?,
	decrement: any,
	increment: any,
	label: string?,
	width: number?,
	to: string?,
}

return component {
	title = "Stepper",
	category = "Forms",
	icon = "hash",
	description = "\"− value +\" in one bordered control, with an optional unit; the ends disable at min and max. Sends { step = -1 } and { step = 1 }.",
	when = "A small number nudged up or down: a font size, a column count, a delay. For a fraction shown as a bar use `meter`; for one of a few named sizes use `segmented`.",
	props = {
		["value"] = "the number shown",
		["unit"] = "text after the value (\"px\", \"s\")",
		["min"] = "lowest value; − is disabled there",
		["max"] = "highest value; + is disabled there",
		["decrement"] = "message − sends, default { step = -1 }",
		["increment"] = "message + sends, default { step = 1 }",
		["label"] = "text before the control",
		["width"] = "pixels for the value, wide enough for its longest reading (default 44)",
		["to"] = "mod to send messages to, if not the drawing one",
	},
	styles = {
		[".stepper"] = { gap = 10 },
		[".stepper-label"] = { color = "muted-foreground" },
		[".stepper-group"] = {
			gap = 0,
			radius = 8,
			border = "all",
			border_color = "border",
			background = "background",
		},
		[".stepper-button"] = { padding = { 5, 8 }, color = "muted-foreground", radius = 7 },
		[".stepper-button:hover"] = { background = "list-hover", color = "foreground" },
		[".stepper-button-disabled"] = { opacity = 0.35 },
		[".stepper-value"] = { padding = { 4, 4 }, gap = 2 },
		[".stepper-separator"] = { background = "border" },
		[".stepper-unit"] = { color = "muted-foreground", size = "small" },
	},
	example = { label = "Font size", value = 14, unit = "px", min = 8, max = 32 },

	build = function(props: Props)
		local low = props.min ~= nil and props.value <= props.min
		local high = props.max ~= nil and props.value >= props.max
		local function button(icon: string, disabled: boolean, msg: any)
			local class = { "stepper-button" }
			if disabled then
				table.insert(class, "stepper-button-disabled")
			end
			return ui.row {
				class = class,
				align = "center",
				justify = "center",
				msg = if disabled then nil else msg,
				to = props.to,
				ui.icon { name = icon :: any },
			}
		end
		local value = ui.row {
			class = "stepper-value",
			width = props.width or 44,
			align = "center",
			justify = "center",
			ui.text { tostring(props.value) },
			props.unit and ui.text { props.unit, class = "stepper-unit" },
		}
		return ui.row {
			class = "stepper",
			align = "center",
			props.label and ui.text { props.label, class = "stepper-label" },
			ui.row {
				class = "stepper-group",
				align = "center",
				button("minus", low, props.decrement or { step = -1 }),
				ui.row { class = "stepper-separator", width = 1, height = 18 },
				value,
				ui.row { class = "stepper-separator", width = 1, height = 18 },
				button("plus", high, props.increment or { step = 1 }),
			},
		}
	end,
}
