-- A value picked by dragging a thumb along a track, or by clicking where
-- it should go: the host turns the pointer into the value (`range`), so a
-- drag sends numbers, not positions. The label and value sit above it, or
-- beside it with `inline`; a click on the value puts back `default`.

type Props = {
	value: number,
	min: number?,
	max: number?,
	step: number?,
	change: any,
	label: string?,
	unit: string?,
	decimals: number?,
	default: number?,
	width: number?,
	inline: boolean?,
	label_width: number?,
	colors: { string }?,
	disabled: boolean?,
	to: string?,
}

-- The thumb's diameter, and the track's thickness, in pixels.
local THUMB = 14
local TRACK = 4

-- `change` with the value's fields added, as the host adds them to a drag:
-- what a click on the value sends to put back `default`.
local function with_value(change: any, value: number): any
	local msg = {}
	if typeof(change) == "table" then
		for key, field in change do
			msg[key] = field
		end
	else
		msg.msg = change
	end
	msg.value = value
	msg.phase = "up"
	return msg
end

local function format(value: number, decimals: number?, unit: string?): string
	local text = if decimals and decimals > 0
		then string.format(`%.{decimals}f`, value)
		else tostring(math.floor(value + 0.5))
	return if unit then `{text}{unit}` else text
end

return component {
	title = "Slider",
	category = "Forms",
	icon = "sliders-horizontal",
	description = "A thumb on a track between `min` and `max`, with a label and its value; dragging or clicking the track sends the new value, snapped to `step`, and a click on the value puts back `default`.",
	when = "A setting along a continuous range where the feel matters more than the exact number: contrast, saturation, an amount, a radius, a volume. For a number the user types or nudges use `number-field` or `stepper`; for a fraction shown read-only use `meter`; for a few named values use `segmented`.",
	props = {
		["value"] = "the current value",
		["min"] = "the left end (default 0)",
		["max"] = "the right end (default 100)",
		["step"] = "the value snaps to multiples of it (default none, or 1 when min and max are whole)",
		["change"] = "message a drag sends, with `value` and `phase` (down, move, up) added; record undo on `phase == \"up\"`",
		["label"] = "text over the track's left (or before it, `inline`)",
		["unit"] = "text after the value (\"%\", \"px\")",
		["decimals"] = "digits after the point in the value shown (default 0)",
		["default"] = "the value a click on the shown value puts back; no reset without it",
		["width"] = "the track's width in pixels (default 180)",
		["inline"] = "label, track and value on one line",
		["label_width"] = "with `inline`, the label's width in pixels (default 88)",
		["colors"] = "the track as a gradient through these colours instead of a fill, such as a hue's { \"#ff0000\", \"#ffff00\", ... }",
		["disabled"] = "greyed out, not draggable",
		["to"] = "mod to send `change` to, if not the drawing one",
	},
	styles = {
		[".slider"] = { gap = 6 },
		[".slider-inline"] = { gap = 10 },
		[".slider-label"] = { size = "small", color = "foreground" },
		[".slider-value"] = { size = "small", color = "muted-foreground", font = "mono" },
		[".slider-reset"] = { radius = 4, padding = { 0, 3 } },
		[".slider-reset:hover"] = { background = "list-hover" },
		[".slider-track"] = { background = "muted-foreground/25", radius = 999 },
		[".slider-fill"] = { background = "accent", radius = 999 },
		[".slider-thumb"] = {
			background = "background",
			border = "all",
			border_color = "accent",
			radius = 999,
			shadow = "0 1px 3px #00000033",
		},
		[".slider-disabled"] = { opacity = 0.45 },
	},
	example = { label = "Contrast", value = 12, min = -50, max = 50, default = 0, width = 220 },

	build = function(props: Props)
		local min = props.min or 0
		local max = props.max or 100
		local whole = min == math.floor(min) and max == math.floor(max) and not props.decimals
		local step = props.step or (if whole then 1 else nil)
		local width = props.width or 180
		local span = if max ~= min then max - min else 1
		local fraction = math.clamp((props.value - min) / span, 0, 1)
		-- A gradient through several colours: one two-colour piece per pair.
		local pieces = {}
		local colors = props.colors or {}
		if #colors >= 2 then
			local piece = width / (#colors - 1)
			for index = 1, #colors - 1 do
				pieces[index] = ui.row {
					width = piece,
					height = TRACK,
					style = { gradient = `90deg, {colors[index]}, {colors[index + 1]}` },
				}
			end
		else
			pieces[1] = ui.row { class = "slider-fill", width = fraction * width, height = TRACK }
		end
		local track = ui.row {
			class = "slider-track",
			width = width,
			height = TRACK,
			gap = "none",
			style = { clip = true },
			float = { top = (THUMB - TRACK) / 2, left = 0 },
			table.unpack(pieces),
		}
		local thumb = ui.row {
			class = "slider-thumb",
			width = THUMB,
			height = THUMB,
			float = { top = 0, left = fraction * width - THUMB / 2 },
		}
		local live = not props.disabled and props.change ~= nil
		local area = ui.row {
			width = width,
			height = THUMB,
			pointer = if live then props.change else nil,
			to = if live then props.to else nil,
			range = if live then { min = min, max = max, step = step, value = props.value } else nil,
			cursor = if live then "pointer" else nil,
			track,
			thumb,
		}
		local shown = ui.text { format(props.value, props.decimals, props.unit), class = "slider-value" }
		local resettable = live and props.default ~= nil and props.value ~= props.default
		local value = if resettable
			then ui.row {
				class = "slider-reset",
				msg = with_value(props.change, props.default :: number),
				to = props.to,
				tooltip = `Reset to {format(props.default :: number, props.decimals, props.unit)}`,
				shown,
			}
			else shown
		local classes = { if props.inline then "slider-inline" else "slider" }
		if props.disabled then
			table.insert(classes, "slider-disabled")
		end
		if props.inline then
			return ui.row {
				class = classes,
				align = "center",
				props.label and ui.row {
					width = props.label_width or 88,
					ui.text { props.label, class = "slider-label", truncate = true },
				},
				area,
				ui.row { width = 52, justify = "end", value },
			}
		end
		return ui.column {
			class = classes,
			hug = true,
			ui.row {
				align = "center",
				width = width,
				props.label and ui.text { props.label, class = "slider-label", truncate = true },
				ui.spacer(),
				value,
			},
			area,
		}
	end,
}
