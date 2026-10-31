-- A Ghostty shell in the bottom pane, starting in the home folder.
local ghostty = use "ghostty-terminal"

return mod {
	region = "bottom",
	title = "Terminal",

	view = function(state, app)
		return ui.panel {
			title = "Terminal",
			grow = true,
			padding = "none",
			ghostty { id = "bottom", cwd = "~" },
		}
	end,
}
