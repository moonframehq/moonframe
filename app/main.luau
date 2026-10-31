-- The shell: lays out the whole window. Mods fill the regions it draws.
return mod {
	init = function(previous)
		return previous or { left = true, right = true, bottom = false }
	end,

	update = function(state, msg)
		if msg == "toggle-left" then
			state.left = not state.left
		elseif msg == "toggle-right" then
			state.right = not state.right
		elseif msg == "toggle-bottom" then
			state.bottom = not state.bottom
		end
	end,

	view = function(state, app)
		-- An icon button for the toolbar: lit while its panel is shown.
		local function tool(icon, msg, on)
			return ui.row {
				class = on and "tool tool-on" or "tool",
				align = "center",
				justify = "center",
				msg = msg,
				ui.icon { icon },
			}
		end

		local toolbar = ui.row {
			padding = "small",
			gap = "small",
			align = "center",
			class = "toolbar",
			tool("panel-left", "toggle-left", state.left),
			tool("panel-bottom", "toggle-bottom", state.bottom),
			ui.spacer(),
			ui.text { "Moonframe", weight = "medium", class = "toolbar-title" },
			ui.spacer(),
			ui.row {
				class = "tool",
				align = "center",
				justify = "center",
				command = "show-themes",
				ui.icon { "palette" },
			},
			tool("panel-right", "toggle-right", state.right),
		}

		local center = ui.vsplit {
			ui.pane { surface = "sidebar", ui.region "main" },
			ui.pane { size = 180, min = 100, fixed = true, visible = state.bottom, surface = "sidebar", ui.region "bottom" },
		}

		return ui.column {
			grow = true,
			gap = "none",
			toolbar,
			ui.hsplit {
				ui.pane { size = 240, min = 160, max = 480, fixed = true, visible = state.left, surface = "sidebar", ui.region "left" },
				ui.pane { center },
				ui.pane { size = 280, min = 180, max = 520, fixed = true, visible = state.right, surface = "sidebar", ui.region "right" },
			},
			ui.row {
				padding = "small",
				gap = "large",
				align = "center",
				surface = "sidebar",
				border = "top",
				-- Mods on the left, and mods pushed to the right edge.
				ui.region { name = "status", direction = "row", gap = "large" },
				ui.region { name = "status-right", direction = "row", gap = "large", justify = "end" },
			},
		}
	end,
}
