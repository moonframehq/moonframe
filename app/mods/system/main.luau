-- CPU, memory, and network in the status bar, from the samples the host
-- shares as `app.system` once a second.

local system_load = use "system-load"
local stat = use "stat"

type System = {
	cpu: number,
	cpu_history: { number },
	memory: number,
	memory_used: number,
	memory_total: number,
	received: number,
	transmitted: number,
}

-- Fits the longest rate ("999K"), so a changing number never pushes the
-- widgets beside it.
local RATE_WIDTH = 34

-- Bytes per second in at most four characters: "911B", "53K", "1.2M", "12M".
local function rate(bytes: number): string
	local units = { "B", "K", "M", "G" }
	local unit = 1
	while bytes >= 1000 and unit < #units do
		bytes /= 1024
		unit += 1
	end
	if unit > 1 and bytes < 10 then
		return string.format("%.1f%s", bytes, units[unit])
	end
	return `{math.floor(bytes)}{units[unit]}`
end

return mod {
	region = "status-right",
	order = 10,

	view = function(_, app)
		local system: System? = app.system
		if not system then
			return ui.text { "Sampling…", class = "status-text" }
		end
		return ui.row {
			align = "center",
			gap = "large",
			system_load { cpu = system.cpu, memory = system.memory, history = system.cpu_history, icons = true },
			ui.row {
				align = "center",
				gap = "small",
				ui.icon { name = "network", class = "status-label" },
				stat { icon = "arrow-down", value = rate(system.received), width = RATE_WIDTH, tone = "muted" },
				stat { icon = "arrow-up", value = rate(system.transmitted), width = RATE_WIDTH, tone = "muted" },
			},
		}
	end,
}
