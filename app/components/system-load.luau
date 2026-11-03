-- A machine's CPU and memory in one row: a CPU sparkline and a memory
-- meter, each with its percentage, or with `compact` two upright bars that
-- fit beside a host's name. The readings sit in boxes of fixed width, so a
-- changing number never pushes the widgets beside it.

local sparkline = use "sparkline"
local meter = use "meter"
local vertical_bar = use "vertical-bar"

type Props = {
	cpu: number,
	memory: number,
	-- CPU samples from 0 to 1, oldest first; defaults to `cpu` alone.
	history: { number }?,
	samples: number?,
	compact: boolean?,
	icons: boolean?,
}

local function percent(fraction: number): string
	return `{math.floor(math.clamp(fraction, 0, 1) * 100 + 0.5)}%`
end

return component {
	title = "System Load",
	category = "Data",
	icon = "cpu",
	description = "A machine's CPU (a sparkline of recent samples) and memory (a meter), each with its percentage; `compact` shows two upright bars instead.",
	when = "CPU and memory of a machine in a status bar, a host row of a sidebar, or a card header. Use `compact` beside a name in a tight row. For one reading alone use `sparkline`, `meter` or `vertical-bar`.",
	props = {
		["cpu"] = "CPU use, 0 to 1",
		["memory"] = "memory use, 0 to 1",
		["history"] = "recent CPU samples, 0 to 1, oldest first",
		["samples"] = "how many CPU samples the sparkline shows (default 16)",
		["compact"] = "two upright bars with percentages, for a tight row",
		["icons"] = "label with cpu and memory icons instead of CPU and MEM",
	},
	styles = {
		[".system-load-label"] = { size = "small", color = "muted-foreground", weight = "medium" },
		[".system-load-percent"] = { size = "small", color = "muted-foreground" },
	},
	example = { cpu = 0.14, memory = 0.5, history = { 0.1, 0.2, 0.1, 0.3, 0.2, 0.6, 0.4, 0.2, 0.1, 0.14 } },

	build = function(props: Props)
		if props.compact then
			return ui.row {
				align = "center",
				gap = "small",
				vertical_bar { value = props.cpu, tooltip = `CPU {percent(props.cpu)}` },
				vertical_bar { value = props.memory, tooltip = `Memory {percent(props.memory)}` },
			}
		end
		local function label(icon: string, text: string)
			return if props.icons
				then ui.icon { name = icon :: any, class = "system-load-label" }
				else ui.text { text, class = "system-load-label" }
		end
		return ui.row {
			align = "center",
			gap = "large",
			ui.row {
				align = "center",
				gap = "small",
				label("cpu", "CPU"),
				sparkline { values = props.history or { props.cpu }, samples = props.samples },
				ui.row {
					width = 32,
					justify = "end",
					ui.text { percent(props.cpu), class = "system-load-percent" },
				},
			},
			ui.row {
				align = "center",
				gap = "small",
				label("memory-stick", "MEM"),
				meter { value = props.memory, width = 28 },
			},
		}
	end,
}
