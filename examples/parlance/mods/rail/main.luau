-- The icon rail on the frame's colour: home (the chats), library, history,
-- explore, more, and apps; a download offer and the signed-in person at the
-- foot. Only home leads anywhere in this example; the others are there for
-- the shape of the app.

type State = { selected: string }

local TOP = {
	{ id = "home", icon = "house", tooltip = "Home" },
	{ id = "library", icon = "layers", tooltip = "Library" },
	{ id = "history", icon = "clock", tooltip = "History" },
	{ id = "explore", icon = "globe", tooltip = "Explore" },
	{ id = "more", icon = "ellipsis", tooltip = "More" },
}

return mod {
	title = "Rail",

	styles = {
		[".pl-rail"] = { padding = { 6, 0, 14, 0 }, gap = 8 },
		[".pl-rail-item"] = { radius = 10, color = "#bdbdbd" },
		[".pl-rail-item:hover"] = { background = "#ffffff0f", color = "foreground" },
		[".pl-rail-item:selected"] = { background = "#555555", color = "foreground" },
		[".pl-rail-dot"] = { background = "accent", radius = 4, border = true, border_color = "#555555" },
		[".pl-rail-rule"] = { height = 1, width = 26, background = "#ffffff1f", margin = { 6, 0 } },
		[".pl-rail-download"] = { background = "accent", radius = 12, color = "#ffffff" },
		[".pl-rail-avatar"] = { background = "#3f9e5c", radius = 12 },
		[".pl-rail-initials"] = { size = "small", weight = "medium", color = "#ffffff" },
	},

	init = function(previous: State?): State
		return previous or { selected = "home" }
	end,

	update = function(state: State, msg)
		if type(msg) == "table" and msg.select then
			state.selected = msg.select
		end
	end,

	view = function(state: State)
		local items = {}
		for _, item in TOP do
			local selected = item.id == state.selected
			table.insert(items, ui.row {
				class = "pl-rail-item",
				width = 36,
				height = 36,
				align = "center",
				justify = "center",
				selected = selected,
				tooltip = item.tooltip,
				msg = { select = item.id },
				ui.icon { name = item.icon :: any, size = "large" },
				item.id == "home" and ui.row { class = "pl-rail-dot", width = 8, height = 8, float = { top = 5, right = 5 } },
			})
		end
		table.insert(items, ui.row { class = "pl-rail-rule" })
		table.insert(items, ui.row {
			class = "pl-rail-item",
			width = 36,
			height = 36,
			align = "center",
			justify = "center",
			tooltip = "Apps",
			msg = { select = "apps" },
			selected = state.selected == "apps",
			ui.icon { name = "layout-dashboard", size = "large" },
		})
		table.insert(items, ui.spacer())
		table.insert(items, ui.row {
			class = "pl-rail-download",
			width = 24,
			height = 24,
			align = "center",
			justify = "center",
			tooltip = "Get the desktop helper",
			ui.icon { name = "download", size = "small" },
		})
		table.insert(items, ui.row { height = 14 })
		table.insert(items, ui.row {
			class = "pl-rail-avatar",
			width = 24,
			height = 24,
			align = "center",
			justify = "center",
			tooltip = "Ada Varga",
			ui.text { "AV", class = "pl-rail-initials" },
		})
		return ui.column {
			class = "pl-rail",
			width = 52,
			shrink = false,
			align = "center",
			table.unpack(items),
		}
	end,
}
