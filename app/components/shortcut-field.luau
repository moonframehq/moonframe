-- A keyboard shortcut the user sets by pressing it: the keys as caps, and
-- while recording, a field that takes the next chord, whatever it is. Escape
-- gives up, and the × clears it.

local kbd = use "kbd"
local icon_button = use "icon-button"

type Props = {
	id: string?,
	value: string?,
	recording: boolean?,
	event: string?,
	clearable: boolean?,
	placeholder: string?,
	width: number?,
	to: string?,
}

return component {
	title = "Shortcut Field",
	category = "Forms",
	icon = "keyboard",
	description = "A shortcut shown as key caps; a click starts recording, and the next chord pressed is sent with its name in `keys`.",
	when = "Letting the user rebind a shortcut, in a `settings-row` or a `shortcuts` page. For showing a shortcut use `kbd`; for a list of them use `shortcuts`.",
	props = {
		["id"] = "keeps the field apart from another (default \"shortcut\")",
		["value"] = "the shortcut, as `kbd` takes it (\"cmd-shift-p\"); none when unset",
		["recording"] = "listening for keys",
		["event"] = "key of its messages, default \"shortcut\": { shortcut = \"record\" } on a click, { shortcut = \"set\", keys = \"cmd-shift-p\" } for the chord pressed, { shortcut = \"cancel\" } on Escape or leaving the field, { shortcut = \"clear\" } from the ×",
		["clearable"] = "show a × while it has a value",
		["placeholder"] = "text while unset (default \"Not set\")",
		["width"] = "pixels (default 160)",
		["to"] = "mod to send them to, if not the drawing one",
	},
	styles = {
		[".shortcut-field"] = { border = "all", border_color = "border", radius = 8, padding = { 3, 4, 3, 8 }, gap = 4 },
		[".shortcut-field:hover"] = { background = "list-hover" },
		[".shortcut-field-recording"] = { border_color = "accent" },
		[".shortcut-field-input"] = { background = "#00000000", border = "none", shadow = "none", padding = 0 },
		[".shortcut-field-none"] = { color = "muted-foreground" },
	},
	example = { value = "cmd-shift-p", clearable = true },
	usage = [[local shortcut_field = use "shortcut-field"

shortcut_field { value = state.keys, recording = state.recording, clearable = true }
-- update: msg.shortcut == "record" sets state.recording; "set" keeps msg.keys
-- and stops; "cancel" stops; "clear" sets state.keys = nil.]],

	build = function(props: Props)
		local width = props.width or 160
		local event = props.event or "shortcut"
		-- Tables, as a field's keys and a click then read the same.
		local cancel = { [event] = "cancel" }
		if props.recording then
			return ui.row {
				class = "shortcut-field shortcut-field-recording",
				width = width,
				align = "center",
				ui.input {
					id = props.id or "shortcut",
					class = "shortcut-field-input",
					value = "",
					placeholder = "Press keys…",
					capture = { [event] = "set" },
					keys = { escape = cancel },
					blur = cancel,
					focus = true,
					to = props.to,
					grow = true,
				},
			}
		end
		return ui.row {
			class = "shortcut-field",
			width = width,
			align = "center",
			msg = { [event] = "record" },
			to = props.to,
			if props.value
				then kbd { keys = props.value, small = true }
				else ui.text { props.placeholder or "Not set", class = "shortcut-field-none" },
			ui.spacer(),
			props.value and props.clearable and icon_button { icon = "close", tooltip = "Clear", size = "small", msg = { [event] = "clear" }, to = props.to },
		}
	end,
}
