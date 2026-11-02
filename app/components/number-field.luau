-- A number in a small field the user types into, whose label scrubs it:
-- drag the label left or right and the host turns the pixels into the
-- value (`range` with a `speed`), as in photo and 3D apps. Optional − and
-- + buttons nudge it by `step`. A preset list can sit beside it.

type Props = {
	value: number,
	change: any,
	min: number?,
	max: number?,
	step: number?,
	speed: number?,
	decimals: number?,
	label: string?,
	unit: string?,
	width: number?,
	buttons: boolean?,
	id: string?,
	disabled: boolean?,
	to: string?,
}

-- `change` with the value's fields added, as the host adds them to a drag.
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

local function format(value: number, decimals: number?): string
	if decimals and decimals > 0 then
		return string.format(`%.{decimals}f`, value)
	end
	return tostring(math.floor(value + 0.5))
end

return component {
	title = "Number Field",
	category = "Forms",
	icon = "hash",
	description = "A number field with a label that scrubs it when dragged sideways, a unit, optional − and + buttons, and min, max and step; typing sends the text, dragging and the buttons send numbers.",
	when = "A number the user sets exactly or nudges: a resolution, a frame number, a zoom, a count, in a dense settings form or inspector. Read `tonumber(msg.value)` for both typing and dragging. For a value along a range where the feel matters use `slider`; for a small count with buttons only use `stepper`.",
	props = {
		["value"] = "the current number",
		["change"] = "message sent as it changes, with `value` added: a number from a drag or a button, the text typed on Enter (`tonumber(msg.value)` reads both), and `phase` (down, move, up) while dragging",
		["min"] = "lowest value; dragging and the buttons stop there",
		["max"] = "highest value",
		["step"] = "what a button adds, and what values snap to (default 1)",
		["speed"] = "value per pixel dragged (default `step`, or a two-hundredth of min to max when that is larger)",
		["decimals"] = "digits after the point shown (default 0)",
		["label"] = "text before the field, which scrubs the value when dragged",
		["unit"] = "text after the number (\"ppi\", \"%\")",
		["width"] = "the field's width in pixels (default 64)",
		["buttons"] = "show − and + on either side",
		["id"] = "keeps what is typed apart from another field's (default the label)",
		["disabled"] = "greyed out, read-only",
		["to"] = "mod to send `change` to, if not the drawing one",
	},
	styles = {
		[".number-field"] = { gap = 8 },
		[".number-field-label"] = { size = "small", color = "muted-foreground" },
		[".number-field-scrub"] = { radius = 4, padding = { 2, 2 } },
		[".number-field-scrub:hover"] = { background = "list-hover" },
		[".number-field-box"] = {
			gap = 0,
			radius = 6,
			border = "all",
			border_color = "border",
			background = "background",
		},
		[".number-field-button"] = { padding = { 4, 6 }, color = "muted-foreground", radius = 5 },
		[".number-field-button:hover"] = { background = "list-hover", color = "foreground" },
		[".number-field-input"] = {
			background = "#00000000",
			border = "none",
			shadow = "none",
			padding = { 4, 6 },
		},
		[".number-field-unit"] = { size = "small", color = "muted-foreground", padding = { 0, 6, 0, 0 } },
		[".number-field-disabled"] = { opacity = 0.45 },
	},
	example = { label = "Output", value = 300, unit = "ppi", min = 10, max = 10000, step = 1, buttons = true },

	build = function(props: Props)
		local step = props.step or 1
		local min, max = props.min, props.max
		local speed = props.speed
		if speed == nil then
			speed = step
			if min and max then
				speed = math.max(step, (max - min) / 2000)
			end
		end
		local live = not props.disabled and props.change ~= nil
		-- The drag's range: open-ended where no bound is given.
		local range = {
			min = min or -1e12,
			max = max or 1e12,
			step = step,
			value = props.value,
			speed = speed,
		}
		local function nudge(icon: string, by: number)
			local next_value = props.value + by
			if min then
				next_value = math.max(min, next_value)
			end
			if max then
				next_value = math.min(max, next_value)
			end
			local stuck = next_value == props.value
			return ui.row {
				class = "number-field-button",
				align = "center",
				msg = if live and not stuck then with_value(props.change, next_value) else nil,
				to = if live and not stuck then props.to else nil,
				ui.icon { name = icon :: any, size = "small" },
			}
		end
		local field = ui.row {
			class = "number-field-box",
			align = "center",
			props.buttons and nudge("minus", -step),
			ui.input {
				class = "number-field-input",
				id = props.id or props.label or "number",
				value = format(props.value, props.decimals),
				submit = if live then props.change else nil,
				to = if live then props.to else nil,
				disabled = props.disabled,
				mono = true,
				width = props.width or 64,
			},
			props.unit and ui.text { props.unit, class = "number-field-unit" },
			props.buttons and nudge("plus", step),
		}
		local label = props.label
			and ui.row {
				class = "number-field-scrub",
				pointer = if live then props.change else nil,
				to = if live then props.to else nil,
				range = if live then range else nil,
				cursor = if live then "ew-resize" else nil,
				tooltip = if live then "Drag to change" else nil,
				ui.text { props.label, class = "number-field-label" },
			}
		local classes = { "number-field" }
		if props.disabled then
			table.insert(classes, "number-field-disabled")
		end
		return ui.row { class = classes, align = "center", hug = true, label, field }
	end,
}
