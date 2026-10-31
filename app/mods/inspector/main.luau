-- The right sidebar: details about the explorer's selection.
return mod {
	region = "right",

	-- Rules of its own, for what this mod draws only. The app's style.luau
	-- outranks them, so the app can always restyle an extension.
	styles = {
		[".label"] = { size = "small", color = "muted-foreground", weight = "medium", margin = { top = 4 } },
		[".value"] = { size = "large", color = "accent" },
	},

	init = function(previous)
		return previous or { selected = "main.luau" }
	end,

	update = function(state, msg)
		if type(msg) == "table" and msg.select then
			state.selected = msg.select
		end
	end,

	view = function(state, app)
		return ui.panel {
			title = "Inspector",
			grow = true,
			ui.text { "Selected", class = "label" },
			ui.text { state.selected, class = "value" },
			ui.divider(),
			ui.text { "Mods", class = "label" },
			ui.row { gap = "small", wrap = true, table.unpack((function()
				local badges = {}
				for index, name in app.mods do
					badges[index] = ui.badge { name, tone = "accent" }
				end
				return badges
			end)()) },
		}
	end,
}
