-- Terminal settings: click − and + to change a value; each stops at its
-- min and max, where that end greys out.
local stepper = use "stepper"

local LIMITS = {
	font = { min = 8, max = 32, step = 1 },
	panes = { min = 1, max = 6, step = 1 },
	delay = { min = 0, max = 2, step = 0.25 },
}

return mod {
	init = function(previous)
		return previous or { font = 14, panes = 2, delay = 0.5 }
	end,

	update = function(state, msg)
		if msg.step and msg.field then
			local limit = LIMITS[msg.field]
			state[msg.field] = math.clamp(state[msg.field] + msg.step * limit.step, limit.min, limit.max)
		end
	end,

	view = function(state)
		local function row(field: string, label: string, unit: string?)
			local limit = LIMITS[field]
			return stepper {
				label = label,
				value = state[field],
				unit = unit,
				min = limit.min,
				max = limit.max,
				width = 56,
				decrement = { field = field, step = -1 },
				increment = { field = field, step = 1 },
			}
		end
		return ui.column {
			gap = "normal",
			width = 260,
			align = "end",
			row("font", "Font size", "px"),
			row("panes", "Panes"),
			row("delay", "Hover delay", "s"),
		}
	end,
}
