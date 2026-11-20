-- Strata's look: the built-in theme, with the repositories pane on the
-- window's background and the panes sliding as they fold.
return style {
	rules = {
		pane = { transition = "slide-fade normal ease-out" },
		[".repos-pane"] = { background = "sidebar" },
	},
}
