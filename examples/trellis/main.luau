-- Trellis's shell: projects and their work trees on the left, from the
-- traffic lights to the window's foot; the chosen work tree's tabs of
-- split terminals in the middle, its source control on the right, and a
-- status bar under both. Each work tree is an instance of the `panes` mod,
-- so its tabs, splits and shells stay as they were while you visit
-- another.

local MENUS: { { title: string, items: { any } } } = {
	{ title = "Shell", items = {
		{ label = "New Tab", msg = { panes = "new-tab" }, shortcut = "secondary-t" },
		{ label = "Split Right", msg = { panes = { split = "row" } }, shortcut = "secondary-d" },
		{ label = "Split Down", msg = { panes = { split = "column" } }, shortcut = "secondary-shift-d" },
		"-",
		{ label = "Zoom Pane", msg = { panes = "zoom" }, shortcut = "secondary-shift-enter" },
		{ label = "Close Pane", msg = { panes = "close" }, shortcut = "secondary-w" },
	} },
	{ title = "View", items = {
		{ label = "Show or Hide Projects", msg = "toggle-left", shortcut = "secondary-b" },
		{ label = "Show or Hide Source Control", msg = "toggle-right", shortcut = "secondary-shift-g" },
		"-",
		{ label = "Branch", msg = { tab = "branch" }, to = "source-control", shortcut = "secondary-1" },
		{ label = "Pull Requests", msg = { tab = "prs" }, to = "source-control", shortcut = "secondary-2" },
		{ label = "Actions", msg = { tab = "actions" }, to = "source-control", shortcut = "secondary-3" },
		{ label = "History", msg = { tab = "history" }, to = "source-control", shortcut = "secondary-4" },
	} },
	{ title = "Window", items = {
		{ label = "Minimize", command = "minimize", shortcut = "secondary-m" },
		{ label = "Zoom", command = "maximize" },
	} },
}

type Worktree = { path: string, name: string, repo: string?, branch: string? }

type State = {
	left: boolean,
	right: boolean,
	worktree: Worktree?,
}

-- An instance id for a work tree's folder: the same folder, the same tabs.
local function panes_id(path: string): string
	local hash = 5381
	for index = 1, #path do
		hash = (hash * 33 + string.byte(path, index)) % 2147483647
	end
	return `w{hash}`
end

return mod {
	window = { titlebar = "hidden", traffic_lights = { x = 16, y = 16 }, size = { width = 1510, height = 900 } },

	about = {
		version = "0.1",
		description = "Terminals organised around projects and their work trees, with splits, tabs and git.",
		website = "https://github.com/penso/moonframe",
		copyright = "MIT licensed",
	},

	menus = MENUS,

	subscribe = { "trellis/worktree" },

	init = function(previous: State?): State
		return previous or { left = true, right = true, worktree = nil }
	end,

	update = function(state: State, msg)
		if msg == "toggle-left" then
			state.left = not state.left
		elseif msg == "toggle-right" then
			state.right = not state.right
		elseif type(msg) == "table" and msg.topic == "trellis/worktree" then
			state.worktree = msg.value
		elseif type(msg) == "table" and msg.panes ~= nil and state.worktree then
			-- The menus act on the work tree shown.
			return state, { { to = `panes#{panes_id(state.worktree.path)}`, msg = msg.panes } }
		end
	end,

	view = function(state: State)
		local worktree = state.worktree
		local center = if worktree
			then ui.mount {
				"panes",
				id = panes_id(worktree.path),
				props = { cwd = worktree.path, name = worktree.name, sidebar = state.left },
			}
			else ui.column {
				grow = true,
				align = "center",
				justify = "center",
				ui.text { "Pick a work tree", tone = "muted" },
			}
		return ui.hsplit {
			id = "body",
			divider = false,
			ui.pane { size = 256, min = 200, max = 420, fixed = true, visible = state.left, class = "trellis-side", ui.region "left" },
			ui.pane {
				class = "trellis-work",
				ui.column {
					grow = true,
					gap = "none",
					ui.hsplit {
						id = "work",
						divider = false,
						ui.pane { min = 320, class = "trellis-center", center },
						ui.pane { size = 392, min = 280, max = 640, fixed = true, visible = state.right, class = "trellis-right", ui.region "right" },
					},
					ui.column { height = 28, shrink = false, gap = "none", ui.mount "status" },
				},
			},
		}
	end,
}
