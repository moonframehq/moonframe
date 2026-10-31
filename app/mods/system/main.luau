-- CPU, memory, and network in the status bar, from the samples the host
-- shares as `app.system` once a second.

type System = {
	cpu: number,
	cpu_history: { number },
	memory: number,
	memory_used: number,
	memory_total: number,
	received: number,
	transmitted: number,
}

local SPARK_HEIGHT = 12
-- Samples the sparkline shows (the provider's `history` setting), each a
-- 2px bar with a 1px gap. Its width is fixed from the first sample, so the
-- widgets beside it stay put while the history fills.
local SPARK_SAMPLES = 16
local SPARK_WIDTH = SPARK_SAMPLES * 3 - 1
-- Widths that fit the longest reading ("100%", "999K"), so a changing
-- number never pushes the widgets beside it.
local PERCENT_WIDTH = 32
local RATE_WIDTH = 34

local function percent(fraction: number): string
	return `{math.floor(fraction * 100 + 0.5)}%`
end

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

-- The latest CPU samples as bars rising from a shared baseline, newest on
-- the right.
local function sparkline(history: { number })
	local bars = {}
	for index = math.max(1, #history - SPARK_SAMPLES + 1), #history do
		local sample = history[index]
		bars[#bars + 1] = ui.row {
			class = if sample > 0.8 then "spark spark-hot" else "spark",
			width = 2,
			height = math.max(1, sample * SPARK_HEIGHT),
		}
	end
	return ui.row {
		class = "sparkline",
		align = "end",
		justify = "end",
		width = SPARK_WIDTH,
		height = SPARK_HEIGHT,
		table.unpack(bars),
	}
end

-- A reading in a box of fixed width, aligned to `justify`.
local function fixed(text: string, width: number, justify: "start" | "end")
	return ui.row { width = width, justify = justify, ui.text { text, class = "status-text" } }
end

local function bar(fraction: number, width: number)
	return ui.row {
		class = "track",
		width = width,
		height = 4,
		gap = "none",
		ui.row { class = if fraction > 0.85 then "fill fill-hot" else "fill", width = math.clamp(fraction, 0, 1) * width, height = 4 },
	}
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
			ui.row {
				align = "center",
				gap = "small",
				ui.icon { name = "cpu", class = "status-label" },
				sparkline(system.cpu_history),
				fixed(percent(system.cpu), PERCENT_WIDTH, "end"),
			},
			ui.row {
				align = "center",
				gap = "small",
				ui.icon { name = "memory-stick", class = "status-label" },
				bar(system.memory, 28),
				fixed(percent(system.memory), PERCENT_WIDTH, "end"),
			},
			ui.row {
				align = "center",
				gap = "small",
				ui.icon { name = "network", class = "status-label" },
				ui.icon { name = "arrow-down", class = "status-label" },
				fixed(rate(system.received), RATE_WIDTH, "start"),
				ui.icon { name = "arrow-up", class = "status-label" },
				fixed(rate(system.transmitted), RATE_WIDTH, "start"),
			},
		}
	end,
}
