-- An onboarding flow: Next and Back move through the steps, or click a
-- dot to jump. Next says "Get started" on the last page and starts over.
local pager = use "pager"

local PAGES = {
	{ title = "Welcome to Moonframe", body = "Run many coding agents side by side." },
	{ title = "Worktrees", body = "Each agent gets its own checkout and branch." },
	{ title = "Review", body = "Read every change before it lands." },
	{ title = "You're set", body = "Open a repository to start your first agent." },
}

return mod {
	init = function(previous)
		return previous or { page = 2 }
	end,

	update = function(state, msg)
		if msg == "back" then
			state.page = math.max(1, state.page - 1)
		elseif msg == "next" then
			state.page = if state.page >= #PAGES then 1 else state.page + 1
		elseif msg.page then
			state.page = msg.page
		end
	end,

	view = function(state)
		local page = PAGES[state.page]
		return ui.column {
			gap = "large",
			ui.column {
				gap = "small",
				ui.text { page.title, size = "large", weight = "bold" },
				ui.text { page.body, tone = "muted" },
			},
			pager {
				count = #PAGES,
				page = state.page,
				back = "back",
				next = "next",
				next_label = if state.page == #PAGES then "Get started" else nil,
			},
		}
	end,
}
