-- A settings page: flip a switch and the "Unsaved" footer appears; Save
-- keeps the change, Reset puts the saved values back.
local page = use "page"
local section = use "section"
local switch = use "switch"
local segmented = use "segmented"

local function settings()
	return { agents = true, counts = true, density = "comfortable" }
end

return mod {
	init = function(previous)
		return previous or { values = settings(), saved = settings() }
	end,

	update = function(state, msg)
		if msg == "save" then
			state.saved = table.clone(state.values)
		elseif msg == "reset" then
			state.values = table.clone(state.saved)
		elseif msg == "agents" or msg == "counts" then
			state.values[msg] = not state.values[msg]
		elseif typeof(msg) == "table" and msg.density then
			state.values.density = msg.density
		end
	end,

	view = function(state)
		local v, saved = state.values, state.saved
		local dirty = v.agents ~= saved.agents or v.counts ~= saved.counts or v.density ~= saved.density
		return ui.column {
			page {
				title = "Sidebar",
				subtitle = "Changes apply to every open window immediately.",
				icon = "panel-left",
				height = 380,
				actions = { ui.button { "Reset", small = true, variant = "ghost", msg = "reset", disabled = not dirty } },
				footer = if dirty
					then ui.row {
						align = "center",
						gap = "normal",
						ui.text { "Unsaved: sidebar", size = "small", tone = "warning" },
						ui.spacer(),
						ui.button { "Save now", small = true, variant = "outline", msg = "save" },
					}
					else nil,
				section {
					title = "LAYOUT",
					ui.row {
						align = "center",
						ui.column { gap = "none", grow = true, ui.text "Density", ui.text { "Row height and padding.", size = "small", tone = "muted" } },
						segmented {
							event = "density",
							selected = v.density,
							small = true,
							options = { { id = "compact", label = "Compact" }, { id = "comfortable", label = "Comfortable" } },
						},
					},
				},
				section {
					title = "CONTENT",
					switch { label = "Show agents", description = "List running agents under their workspace.", on = v.agents, msg = "agents" },
					switch { label = "Show counts", description = "A number after each section's title.", on = v.counts, msg = "counts" },
				},
			},
		}
	end,
}
