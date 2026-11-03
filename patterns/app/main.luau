local title_bar = use "title-bar"

return mod {
	window = { titlebar = "hidden", traffic_lights = { x = 16, y = 14 }, size = { width = 1260, height = 860 } },
	view = function(_, app)
		return ui.column {
			grow = true, gap = "none",
			title_bar { window = app.window, title = "Moonframe Patterns", actions = {
				ui.button { icon = "sun", command = "toggle-theme", variant = "ghost", tooltip = "Toggle theme" },
			} },
			ui.mount "catalog",
		}
	end,
}
