-- A pane with history: click a page to visit it, then use the chevrons
-- to go back and forward. Each side greys out at the end of the history.
local nav_buttons = use "nav-buttons"

local PAGES = { "README.md", "src/main.rs", "src/tabs.rs", "Cargo.toml" }

return mod {
	init = function(previous)
		return previous or { history = { "README.md", "src/main.rs" }, at = 2 }
	end,

	update = function(state, msg)
		if msg == "back" then
			state.at = math.max(1, state.at - 1)
		elseif msg == "forward" then
			state.at = math.min(#state.history, state.at + 1)
		elseif msg.visit and msg.visit ~= state.history[state.at] then
			-- Visiting drops everything after the current page, as a browser does.
			for index = #state.history, state.at + 1, -1 do
				table.remove(state.history, index)
			end
			table.insert(state.history, msg.visit)
			state.at = #state.history
		end
	end,

	view = function(state)
		local links = {}
		for _, page in PAGES do
			table.insert(links, ui.item { page, selected = page == state.history[state.at], msg = { visit = page } })
		end
		return ui.column {
			gap = "normal",
			ui.row {
				gap = "normal",
				align = "center",
				nav_buttons { can_back = state.at > 1, can_forward = state.at < #state.history },
				ui.text { state.history[state.at], weight = "medium" },
				ui.text { `{state.at} of {#state.history}`, tone = "muted", size = "small" },
			},
			ui.column { gap = "none", width = 280, table.unpack(links) },
		}
	end,
}
