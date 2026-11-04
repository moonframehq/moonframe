-- Lightbox's shell: a bar across the top, the console under it.
return mod {
	window = { size = { width = 1240, height = 820 } },

	about = {
		version = "0.1",
		description = "A pretend film scanner: live video, its instruments, and what a long unattended job needs.",
		website = "https://github.com/moonframehq/moonframe",
		copyright = "MIT licensed",
	},

	view = function()
		return ui.column {
			grow = true,
			ui.row { height = 52, shrink = false, ui.region { name = "top", direction = "row" } },
			ui.region "main",
		}
	end,
}
