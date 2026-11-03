-- The picture alone, for a second screen: the same ring as the main
-- window, read by a video of its own, at the rate frames arrive.
return mod {
	title = "Preview",
	window = { size = { width = 960, height = 720 } },

	view = function()
		return ui.column {
			grow = true,
			style = { background = "#000000" },
			ui.video { source = "framebench", white = 4095 },
		}
	end,
}
