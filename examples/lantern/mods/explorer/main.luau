-- A files pane: the open project's folder path, collapse and refresh, and
-- its tree (the `file-tree` component over `app.projects.dirs`). Mounted by
-- the workspace once per pane that shows files. The provider lists one set
-- of open folders, so two files panes share what is unfolded.

local icon_button = use "icon-button"
local file_tree = use "file-tree"

type State = { open: { [string]: boolean? }, selected: string? }

local function open_list(open: { [string]: boolean? }): string
	local paths = {}
	for path in open do
		table.insert(paths, path)
	end
	table.sort(paths)
	return table.concat(paths, "\n")
end

return mod {
	title = "Files",

	styles = {
		[".explorer-path"] = { padding = { 4, 4, 4, 10 }, gap = 2, border = "bottom", border_color = "border" },
		[".explorer-path-text"] = { color = "muted-foreground" },
		[".explorer-body"] = { padding = { 6, 6 } },
	},

	init = function(previous: State?): State
		return previous or { open = {}, selected = nil }
	end,

	update = function(state: State, msg)
		if msg == "collapse" then
			state.open = {}
			return state, { { to = "provider:projects", msg = { open = "" } } }
		elseif msg == "refresh" then
			return state, { { to = "provider:projects", msg = { refresh = true, open = open_list(state.open) } } }
		elseif type(msg) == "table" and msg.toggle then
			state.open[msg.toggle] = if state.open[msg.toggle] then nil else true
			return state, { { to = "provider:projects", msg = { open = open_list(state.open) } } }
		elseif type(msg) == "table" and msg.pick then
			state.selected = msg.pick
		end
	end,

	view = function(state: State, app)
		local projects = app.projects
		local current = projects and projects.current
		if not current then
			return ui.column {
				grow = true,
				align = "center",
				justify = "center",
				ui.text { if projects then "No project open" else "Looking for projects…", tone = "muted" },
			}
		end
		local marks = {}
		for _, line in current.files do
			local status, path = string.match(line, "^(%a)\t%d+\t%d+\t(.+)$")
			if path then
				marks[path] = status
			end
		end
		return ui.column {
			grow = true,
			gap = "none",
			ui.row {
				class = "explorer-path",
				height = 34,
				align = "center",
				shrink = false,
				ui.text { current.path, class = "explorer-path-text", truncate = true, tooltip = current.path },
				ui.spacer(),
				ui.row {
					gap = "none",
					shrink = false,
					icon_button { icon = "chevrons-up-down", tooltip = "Collapse all", size = "small", msg = "collapse" },
					icon_button { icon = "refresh-cw", tooltip = "Look again", size = "small", msg = "refresh" },
				},
			},
			ui.column {
				class = "explorer-body",
				grow = true,
				scroll = true,
				file_tree { dirs = projects.dirs, open = state.open, selected = state.selected, marks = marks },
			},
		}
	end,
}
