-- Agents and their state: click an agent to cycle its status; the controls
-- switch dot size, spinning and the chip form.
local status_dot = use "status-dot"
local segmented = use "segmented"
local switch = use "switch"

local CYCLE = { working = "blocked", blocked = "done", done = "idle", idle = "unknown", unknown = "working" }

return mod {
	init = function(previous)
		return previous
			or {
				size = "normal",
				spin = true,
				chip = false,
				agents = {
					{ name = "Claude · feature/inspect", status = "working" },
					{ name = "Codex · fix/menus", status = "blocked" },
					{ name = "Gemini · docs/readme", status = "done" },
					{ name = "Claude · main", status = "idle" },
				},
			}
	end,

	update = function(state, msg)
		if msg == "toggle-spin" then
			state.spin = not state.spin
		elseif msg == "toggle-chip" then
			state.chip = not state.chip
		elseif type(msg) == "table" and msg.select then
			state.size = msg.select
		elseif type(msg) == "table" and msg.cycle then
			local agent = state.agents[msg.cycle]
			agent.status = CYCLE[agent.status]
		end
	end,

	view = function(state)
		local rows = {}
		for index, agent in state.agents do
			rows[index] = ui.row {
				gap = "normal",
				align = "center",
				msg = { cycle = index },
				tooltip = "Click to change its state",
				status_dot { status = agent.status, size = state.size, spin = state.spin, chip = state.chip },
				ui.text { agent.name },
			}
		end
		return ui.column {
			gap = "normal",
			ui.row {
				gap = "normal",
				align = "center",
				segmented {
					selected = state.size,
					small = true,
					options = { { id = "small", label = "Small" }, { id = "normal", label = "Normal" }, { id = "large", label = "Large" } },
				},
				switch { on = state.spin, label = "Spin", small = true, msg = "toggle-spin" },
				switch { on = state.chip, label = "Chip", small = true, msg = "toggle-chip" },
			},
			ui.column { gap = "small", table.unpack(rows) },
		}
	end,
}
