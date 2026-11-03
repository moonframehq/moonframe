-- A local, interactive task-management showcase; all work lives in one mod.
return mod {
	window = { titlebar = "hidden", traffic_lights = { x = 16, y = 16 }, size = { width = 1580, height = 960 } },
	about = { version = "0.1", description = "A calm place for projects. Interactive Moonframe component demo." },
	menus = {
		{ title = "Project", items = {
			{ label = "New Task", msg = "new-task", to = "workspace", shortcut = "secondary-n" },
			{ label = "Projects", msg = { page = "home" }, to = "workspace", shortcut = "secondary-0" },
		} },
		{ title = "View", items = {
			{ label = "Board", msg = { view = "board" }, to = "workspace", shortcut = "secondary-1" },
			{ label = "List", msg = { view = "list" }, to = "workspace", shortcut = "secondary-2" },
			{ label = "Calendar", msg = { view = "calendar" }, to = "workspace", shortcut = "secondary-3" },
			{ label = "Timeline", msg = { view = "timeline" }, to = "workspace", shortcut = "secondary-4" },
		} },
	} :: { { title: string, items: { any } } },
	view = function()
		return ui.column { grow = true, gap = "none", ui.mount "workspace" }
	end,
}
