-- A region and a mount.
return mod {
	view = function()
		return ui.row { ui.region { "left", direction = "column", gap = "small" }, ui.mount "detail" }
	end,
}
