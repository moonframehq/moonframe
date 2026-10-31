-- A status bar: "Refresh" takes new readings; the values keep a fixed
-- width so neighbours never shift, and CPU turns warning past 80%.
local stat = use "stat"

local READINGS = {
	{ cpu = 12, memory = 4.2, agents = 2 },
	{ cpu = 47, memory = 5.8, agents = 3 },
	{ cpu = 91, memory = 7.1, agents = 5 },
	{ cpu = 6, memory = 3.9, agents = 1 },
}

return mod {
	init = function(previous)
		return previous or { at = 1 }
	end,

	update = function(state, msg)
		if msg == "refresh" then
			state.at = state.at % #READINGS + 1
		end
	end,

	view = function(state)
		local reading = READINGS[state.at]
		return ui.column {
			gap = "large",
			ui.row {
				gap = "large",
				align = "center",
				style = { border = "all", border_color = "border", radius = 6, padding = { 4, 10 } },
				stat { icon = "git-branch", value = "main", width = 48 },
				stat {
					icon = "cpu",
					label = "CPU",
					value = `{reading.cpu}%`,
					width = 32,
					tone = if reading.cpu > 80 then "warning" else nil,
				},
				stat { icon = "memory-stick", label = "Mem", value = `{reading.memory} GB`, width = 48 },
				stat { icon = "agent-claude", value = `{reading.agents} agents`, width = 60, tone = "muted" },
			},
			ui.row { ui.button { "Refresh", small = true, msg = "refresh" } },
		}
	end,
}
