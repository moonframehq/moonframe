-- A first-run tour: Next / Back (or ← →) move, a dot jumps to its page,
-- Get started or × closes it, Show tour reopens it at page 1.
local onboarding = use "onboarding"

local PAGES = {
	{ icon = "terminal", title = "Terminals that remember", text = "Every workspace keeps its panes, tabs and scrollback across restarts." },
	{ icon = "bot", title = "Run agents side by side", text = "Start Claude, Codex or Gemini in their own worktrees and watch them from the sidebar." },
	{ icon = "globe", title = "Your machines, one window", text = "Add SSH hosts and cloud devices; Moonframe reconnects when they drop." },
}

return mod {
	init = function(previous)
		return previous or { open = true, current = 1 }
	end,

	update = function(state, msg)
		if msg == "next" then
			state.current = math.min(state.current + 1, #PAGES)
		elseif msg == "back" then
			state.current = math.max(state.current - 1, 1)
		elseif msg == "done" then
			state.open = false
		elseif msg == "show" then
			state.open = true
			state.current = 1
		elseif type(msg) == "table" and msg.page then
			state.current = msg.page
		end
	end,

	view = function(state)
		return ui.column {
			gap = "normal",
			ui.row { ui.button { "Show tour", icon = "sparkles", small = true, variant = "outline", msg = "show" } },
			onboarding {
				open = state.open,
				title = "Welcome to Moonframe",
				icon = "sparkles",
				badge = "BETA",
				current = state.current,
				pages = PAGES,
			},
		}
	end,
}
