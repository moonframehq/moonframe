-- Where agents run: click an option to choose it ("Cloud machine" is
-- disabled). The row below picks a layout horizontally.
local radio_group = use "radio-group"

return mod {
	init = function(previous)
		return previous or { place = "worktree", layout = "tabs" }
	end,

	update = function(state, msg)
		if msg.select then
			state.place = msg.select
		elseif msg.layout then
			state.layout = msg.layout
		end
	end,

	view = function(state)
		return ui.column {
			gap = "large",
			radio_group {
				selected = state.place,
				options = {
					{ id = "worktree", label = "New worktree", description = "Each agent gets its own checkout and branch." },
					{ id = "current", label = "Current folder", description = "Agents share the working tree you are in." },
					{ id = "remote", label = "Cloud machine", description = "Run on a remote box; needs a connected device.", disabled = true },
				},
			},
			radio_group {
				horizontal = true,
				event = "layout",
				selected = state.layout,
				options = {
					{ id = "tabs", label = "Tabs" },
					{ id = "split", label = "Split" },
					{ id = "grid", label = "Grid" },
				},
			},
			ui.text { `Agents run in: {state.place}, laid out as {state.layout}`, tone = "muted", size = "small" },
		}
	end,
}
