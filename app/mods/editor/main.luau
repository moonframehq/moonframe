-- The main area: a live shell, from the `terminal` component (alacritty).
-- Swap in `use "ghostty-terminal"` to run it on Ghostty's emulator.
local terminal = use "terminal"

return mod {
	region = "main",

	-- Messages from the explorer still arrive; the shell has no use for them.
	update = function() end,

	view = function()
		return ui.panel {
			title = "Terminal",
			grow = true,
			gap = "none",
			padding = "none",
			terminal { id = "main" },
		}
	end,
}
