-- The bottom pane: a second tab bar of its own (`tabbed`), starting with a
-- Ghostty shell in the home folder.
local tabbed = use "tabbed"

return mod {
	region = "bottom",
	title = "Terminal",

	view = function(state, app)
		return ui.panel {
			grow = true,
			padding = "none",
			gap = "none",
			-- Denser than the main bar: its own look wins over `looks.tabs`.
			tabbed { id = "bottom", start = "component:ghostty-terminal", variant = "compact" },
		}
	end,
}
