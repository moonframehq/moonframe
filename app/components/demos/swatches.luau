-- Workspace colour: click a swatch to pick it, or the first one for none.
-- The second row picks from named colours with bigger swatches.
local swatches = use "swatches"

return mod {
	init = function(previous)
		return previous or { color = "#8b5cf6", accent = "#3b82f6" }
	end,

	update = function(state, msg)
		if msg.pick then
			state.color = msg.pick
		elseif msg.accent then
			state.accent = msg.accent
		end
	end,

	view = function(state)
		return ui.column {
			gap = "large",
			ui.column {
				gap = "small",
				ui.text { "Workspace colour", weight = "medium" },
				swatches {
					none = true,
					selected = state.color,
					colors = { "#ef4444", "#f97316", "#eab308", "#22c55e", "#14b8a6", "#3b82f6", "#8b5cf6", "#ec4899", "#64748b" },
				},
			},
			ui.column {
				gap = "small",
				ui.text { "Accent", weight = "medium" },
				swatches {
					event = "accent",
					size = 28,
					selected = state.accent,
					colors = {
						{ color = "#3b82f6", label = "Blue" },
						{ color = "#22c55e", label = "Green" },
						{ color = "#f97316", label = "Orange" },
						{ color = "#ec4899", label = "Pink" },
					},
				},
			},
			ui.row {
				gap = "normal",
				align = "center",
				ui.row {
					width = 14,
					height = 14,
					style = { background = if state.color == "none" then "#00000000" else state.color, radius = 4 },
				},
				ui.text { `ledger-api: {state.color}, accent {state.accent}`, tone = "muted", size = "small" },
			},
		}
	end,
}
