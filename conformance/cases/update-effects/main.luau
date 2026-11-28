-- The shell mounts the two mods; it keeps nothing itself.
return mod {
	view = function()
		return ui.column { ui.mount "counter", ui.mount { mod = "status" } }
	end,
}
