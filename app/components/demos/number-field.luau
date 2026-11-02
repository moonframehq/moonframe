-- An output's size: type a number and press Enter, drag a label sideways
-- to scrub it, or use − and +. `tonumber(msg.value)` reads the typed text
-- and the dragged number alike.
local number_field = use "number-field"

local LIMITS = {
	ppi = { min = 10, max = 10000 },
	slot = { min = 1, max = 12 },
	zoom = { min = 1, max = 400 },
}

return mod {
	init = function(previous)
		return previous or { ppi = 300, slot = 1, zoom = 100 }
	end,

	update = function(state, msg)
		if typeof(msg) == "table" and msg.field then
			local value = tonumber(msg.value)
			local limit = LIMITS[msg.field]
			if value and limit then
				state[msg.field] = math.clamp(value, limit.min, limit.max)
			end
		end
	end,

	view = function(state)
		local function field(name: string, label: string, extra: any)
			local props = {
				id = name,
				label = label,
				value = state[name],
				min = LIMITS[name].min,
				max = LIMITS[name].max,
				change = { field = name },
			}
			for key, value in extra do
				props[key] = value
			end
			return number_field(props)
		end
		return ui.column {
			gap = "normal",
			align = "end",
			hug = true,
			field("ppi", "Output", { unit = "ppi", buttons = true }),
			field("slot", "Slide", { buttons = true, width = 40 }),
			field("zoom", "Zoom", { unit = "%", speed = 0.5 }),
		}
	end,
}
