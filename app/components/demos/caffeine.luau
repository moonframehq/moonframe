-- The cup, off and on. In a preview it asks the app for nothing.
local caffeine = use "caffeine"

return mod {
	view = function()
		return ui.row {
			gap = "normal",
			caffeine { reason = "Scanning" },
			caffeine { awake = { held = true, display = true }, reason = "Scanning" },
			caffeine { awake = { held = true, display = false }, scope = "system", size = "small" },
		}
	end,
}
