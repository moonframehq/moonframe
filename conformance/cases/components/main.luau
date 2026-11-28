-- A component used twice, styled by the stylesheet and the mod's own rules.
local stat = use "stat"

return mod {
	styles = {
		[".stat"] = { padding = "normal", radius = 8 },
	},
	view = function()
		return ui.row {
			gap = "large",
			stat { label = "Open", value = 12 },
			stat { label = "Merged", value = 3 },
		}
	end,
}
