-- A tab showing the repository's commits as a graph. It asks git for them
-- as it opens; git reads them again when the branches move.
local commit_graph = use "commit-graph"
local empty_state = use "empty-state"

local COMMITS = 200

return mod {
	title = "History",
	tab = { title = "History", icon = "git-branch" },

	init = function(previous: any): (any, { any })
		return {}, { { to = "provider:git", msg = { log = COMMITS } } }
	end,

	view = function(state: any, app)
		local log = app.git and app.git.log
		if not log or #log == 0 then
			return empty_state {
				icon = "git-branch",
				title = "No commits",
				description = "Commits show here once the repository has some.",
			}
		end
		return ui.column {
			grow = true,
			scroll = true,
			padding = "small",
			commit_graph { commits = log },
		}
	end,
}
