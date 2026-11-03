-- A picture's levels as an RGB parade, or overlaid. An engine publishes
-- these at its own rate; here they are made up.
local waveform = use "waveform"
local switch = use "switch"

local function columns(base: number, swing: number): ({ number }, { number })
	local low, high = {}, {}
	for column = 1, 96 do
		local level = base + swing * math.sin(column / 9)
		low[column] = math.max(level - 0.18, 0)
		high[column] = math.min(level + 0.22, 1)
	end
	return low, high
end

local RED_LOW, RED_HIGH = columns(0.5, 0.2)
local GREEN_LOW, GREEN_HIGH = columns(0.45, 0.25)
local BLUE_LOW, BLUE_HIGH = columns(0.35, 0.15)

return mod {
	init = function(previous)
		return previous or { parade = true }
	end,

	update = function(state, msg)
		if msg == "parade" then
			state.parade = not state.parade
		end
	end,

	view = function(state)
		return ui.column {
			gap = "normal",
			hug = true,
			waveform {
				width = 340,
				height = 150,
				parade = state.parade,
				channels = {
					{ name = "Red", color = "#ef4444", low = RED_LOW, high = RED_HIGH },
					{ name = "Green", color = "#22c55e", low = GREEN_LOW, high = GREEN_HIGH },
					{ name = "Blue", color = "#3b82f6", low = BLUE_LOW, high = BLUE_HIGH },
				},
			},
			switch { label = "Parade", value = state.parade, change = "parade" },
		}
	end,
}
