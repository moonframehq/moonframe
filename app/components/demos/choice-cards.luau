-- Terminal theme and density: click a card to choose it ("Solarized" is
-- disabled). The second grid uses icons instead of colour strips.
local choice_cards = use "choice-cards"

return mod {
	init = function(previous)
		return previous or { theme = "default", density = "comfortable" }
	end,

	update = function(state, msg)
		if msg.select then
			state.theme = msg.select
		elseif msg.density then
			state.density = msg.density
		end
	end,

	view = function(state)
		return ui.column {
			gap = "large",
			choice_cards {
				selected = state.theme,
				width = 168,
				options = {
					{
						id = "darkside",
						title = "Darkside",
						description = "Ghostty",
						colors = { "#222324", "#e8341c", "#68c256", "#f2d42c", "#1c98e8", "#8e69c9", "#bababa" },
					},
					{
						id = "default",
						title = "Default",
						description = "Built-in / Saved",
						colors = { "#000000", "#990000", "#00a600", "#999900", "#0000b2", "#b200b2", "#bfbfbf" },
					},
					{
						id = "dawnfox",
						title = "Dawnfox",
						description = "Ghostty",
						colors = { "#575279", "#b4637a", "#618774", "#ea9d34", "#286983", "#907aa9", "#e5e9f0" },
					},
					{
						id = "solarized",
						title = "Solarized",
						description = "Not installed",
						disabled = true,
						colors = { "#002b36", "#dc322f", "#859900", "#b58900", "#268bd2", "#d33682", "#eee8d5" },
					},
				},
			},
			choice_cards {
				event = "density",
				selected = state.density,
				width = 168,
				options = {
					{ id = "comfortable", title = "Comfortable", icon = "layout-dashboard", description = "Roomy rounded rows" },
					{ id = "compact", title = "Compact", icon = "menu", description = "Dense rows, more fits" },
				},
			},
		}
	end,
}
