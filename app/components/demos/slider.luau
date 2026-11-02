-- A photo's tone controls: drag a thumb or click the track to set it; a
-- click on a value that is not at its default puts the default back. The
-- last line says what the drag reported, so `phase = "up"` (where an
-- undo step belongs) shows as the button comes up.
local slider = use "slider"

local DEFAULTS = { contrast = 0, brightness = 0, saturation = 0, radius = 1.2, quality = 90 }

return mod {
	init = function(previous)
		return previous
			or { contrast = 12, brightness = -4, saturation = 30, radius = 1.2, quality = 90, hue = 200, last = "" }
	end,

	update = function(state, msg)
		if typeof(msg) == "table" and msg.set and msg.value then
			state[msg.set] = msg.value
			state.last = `{msg.set} = {msg.value} ({msg.phase})`
		end
	end,

	view = function(state)
		local function row(field: string, label: string, min: number, max: number, extra: any?)
			local props = {
				label = label,
				value = state[field],
				min = min,
				max = max,
				default = DEFAULTS[field],
				change = { set = field },
				inline = true,
				width = 180,
			}
			for key, value in extra or {} do
				props[key] = value
			end
			return slider(props)
		end
		return ui.column {
			gap = "normal",
			row("contrast", "Contrast", -50, 50),
			row("brightness", "Brightness", -50, 50),
			row("saturation", "Saturation", -100, 100),
			row("radius", "Radius", 0.1, 10, { step = 0.1, decimals = 1, unit = " px" }),
			row("quality", "JPEG quality", 1, 100, { unit = "%" }),
			slider {
				label = "Hue",
				value = state.hue,
				min = 0,
				max = 360,
				unit = "°",
				change = { set = "hue" },
				width = 288,
				colors = { "#ff0000", "#ffff00", "#00ff00", "#00ffff", "#0000ff", "#ff00ff", "#ff0000" },
			},
			ui.text { state.last, tone = "muted", size = "small" },
		}
	end,
}
