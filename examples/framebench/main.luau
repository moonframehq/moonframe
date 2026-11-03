-- Framebench's shell: the bench fills the window.
return mod {
	window = { size = { width = 1180, height = 760 } },

	about = {
		version = "0.1",
		description = "Measures live video on Moonframe against a stand-in capture engine.",
		website = "https://github.com/moonframehq/moonframe",
		copyright = "MIT licensed",
	},

	view = function()
		return ui.region "main"
	end,
}
