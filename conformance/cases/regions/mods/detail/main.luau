return mod {
	title = "Detail",
	view = function(state, app)
		return ui.text { if app.dark then "dark" else "light" }
	end,
}
