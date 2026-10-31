-- Resource meters: − and + move each reading by 10%; past 80% the bar
-- turns to the warning colour.
local meter = use "meter"
local stepper = use "stepper"

local NAMES = { "memory", "disk", "quota" }
local LABELS = { memory = "Memory", disk = "Disk", quota = "API quota" }

return mod {
	init = function(previous)
		return previous or { memory = 0.62, disk = 0.85, quota = 0.3 }
	end,

	update = function(state, msg)
		if msg.step and msg.name then
			state[msg.name] = math.clamp(math.round(state[msg.name] * 10 + msg.step) / 10, 0, 1)
		end
	end,

	view = function(state)
		local rows = {}
		for _, name in NAMES do
			table.insert(
				rows,
				ui.row {
					gap = "large",
					align = "center",
					ui.row { width = 80, ui.text { LABELS[name], tone = "muted" } },
					ui.row { width = 170, meter { value = state[name], width = 120, warn = 0.8 } },
					stepper {
						value = math.round(state[name] * 100),
						unit = "%",
						min = 0,
						max = 100,
						width = 52,
						decrement = { name = name, step = -1 },
						increment = { name = name, step = 1 },
					},
				}
			)
		end
		return ui.column { gap = "normal", table.unpack(rows) }
	end,
}
