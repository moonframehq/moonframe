-- Sample data. The reusable pattern does not own application state.
local issue_card = use "issue-card"
local segmented = use "segmented"

return mod {
	init = function() return { density = "comfortable", opened = false } end,
	update = function(state, msg)
		if type(msg) == "table" and msg.density then state.density = msg.density
		elseif msg == "open" then state.opened = not state.opened end
	end,
	view = function(state)
		return ui.column { gap = "large",
			segmented { selected = state.density, event = "density", options = {
				{ id = "comfortable", label = "Comfortable" }, { id = "compact", label = "Compact" },
			} },
			issue_card {
				title = "Dark mode for the settings page", identifier = "APP-150",
				blocked_by = "APP-142", labels = { "feature" }, source_icon = "brand-github",
				seconds = 86400, priority = "high", assignee = "Jamie",
				compact = state.density == "compact", msg = "open",
			},
			state.opened and ui.text { "Card opened — handle this message in your app.", tone = "muted", size = "small" },
		}
	end,
}
