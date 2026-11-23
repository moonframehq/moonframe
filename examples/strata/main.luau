-- Strata's shell: a title bar over the repositories (a pane that folds
-- away), the history of the open repository as a graph, and the commit
-- picked with its diff. History and Branches switch what the middle pane
-- lists. With no repository open, the repositories mod fills the window.

local icon_button = use "icon-button"
local segmented = use "segmented"
local title_bar = use "title-bar"

-- The menus after the app menu (About, Quit); "View" merges into the
-- host's. A typed local, so items with `msg` and `command` mix.
local MENUS: { { title: string, items: { any } } } = {
	{ title = "File", items = {
		{ label = "Open Repository…", msg = "open", to = "repositories", shortcut = "secondary-o" },
	} },
	{ title = "View", items = {
		{ label = "History", msg = { select = "history" }, shortcut = "secondary-1" },
		{ label = "Branches", msg = { select = "branches" }, shortcut = "secondary-2" },
		"-",
		{ label = "Find a Commit", msg = "find", to = "timeline", shortcut = "secondary-f" },
		{ label = "Show or Hide Repositories", msg = "toggle-repos", shortcut = "secondary-b" },
		{ label = "Unified or Split Diff", msg = "split", to = "commit", shortcut = "secondary-shift-d" },
	} },
	{ title = "Window", items = {
		{ label = "Minimize", command = "minimize", shortcut = "secondary-m" },
		{ label = "Zoom", command = "maximize" },
	} },
}

type State = {
	-- The repositories pane shows.
	repos: boolean,
	-- What the middle pane lists: "history" or "branches".
	tab: string,
}

local function name_of(path: string): string
	return string.match(path, "([^/\\]+)[/\\]*$") or path
end

return mod {
	window = { titlebar = "hidden", traffic_lights = { x = 18, y = 14 }, size = { width = 1360, height = 860 } },

	about = {
		version = "0.1",
		description = "Your repositories, their history as a graph, and each commit's diff.",
		website = "https://github.com/moonframehq/moonframe",
		copyright = "MIT licensed",
	},

	menus = MENUS,

	init = function(previous: State?): State
		return { repos = if previous then previous.repos else true, tab = "history" }
	end,

	update = function(state: State, msg: any)
		if msg == "toggle-repos" then
			state.repos = not state.repos
		elseif type(msg) == "table" and (msg.select == "history" or msg.select == "branches") then
			state.tab = msg.select
			return state, { { to = "timeline", msg = { tab = msg.select } } }
		else
			return
		end
		return state
	end,

	view = function(state: State, app)
		local git = app.git
		-- No repository (started from the Finder, in `/`): a way to open one.
		if git and not git.repository then
			return ui.column { grow = true, gap = "none", ui.mount "repositories" }
		end
		local branch = git and (if git.detached then "detached HEAD" else git.branch)
		local bar = title_bar {
			window = app.window,
			leading = {
				icon_button {
					icon = if state.repos then "panel-left-close" else "panel-left-open",
					tooltip = "Show or Hide Repositories",
					active = state.repos,
					msg = "toggle-repos",
				},
			},
			actions = {
				segmented {
					small = true,
					selected = state.tab,
					options = {
						{ id = "history", label = "History", icon = "history" },
						{ id = "branches", label = "Branches", icon = "git-branch", count = git and #(git.branches or {}) or nil },
					},
				},
			},
			ui.row {
				gap = "small",
				align = "center",
				ui.icon { "folder-git-2", tone = "muted" },
				ui.text { git and git.root and name_of(git.root) or "Strata", weight = "semibold" },
				branch and ui.button {
					branch,
					icon = "git-branch",
					small = true,
					variant = "outline",
					tooltip = "Branches",
					msg = { select = "branches" },
				} or nil,
			},
		}
		return ui.column {
			grow = true,
			gap = "none",
			bar,
			ui.hsplit {
				id = "strata",
				ui.pane { size = 240, min = 180, max = 400, visible = state.repos, class = "repos-pane", ui.mount "repositories" },
				ui.pane { size = 400, min = 280, max = 640, ui.mount "timeline" },
				ui.pane { min = 360, ui.mount "commit" },
			},
		}
	end,
}
