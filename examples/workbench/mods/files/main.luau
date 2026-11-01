-- The project's changes and files, beside the cards. Files is the whole
-- tree with changed files marked; Changes lists only what is uncommitted,
-- as a tree of folders. A file opens in the editor.

local icon_button = use "icon-button"
local diff_counts = use "diff-counts"
local change_tree = use "change-tree"
local file_tree = use "file-tree"

type State = {
	tab: string,
	open: { [string]: boolean? },
	folded: { [string]: boolean? },
	selected: string?,
}

local function open_list(open: { [string]: boolean? }): string
	local paths = {}
	for path in open do
		table.insert(paths, path)
	end
	table.sort(paths)
	return table.concat(paths, "\n")
end

return mod {
	region = "right",
	title = "Files",

	styles = {
		[".files-top"] = { padding = { 0, 10, 0, 12 }, gap = 4 },
		[".files-tab"] = { padding = { 5, 12 }, radius = 9, color = "muted-foreground" },
		[".files-tab:hover"] = { background = "list-hover" },
		[".files-tab:selected"] = { background = "card", color = "foreground", shadow = "0 1px 2px #00000012" },
		[".files-tab-label"] = { weight = "medium" },
		[".files-path"] = { padding = { 2, 8, 6, 18 }, gap = 2 },
		[".files-path-text"] = { color = "muted-foreground" },
		[".files-body"] = { padding = { 0, 8, 10, 8 } },
		[".files-summary"] = { padding = { 2, 10, 6, 18 }, gap = 8 },
	},

	init = function(previous: State?): State
		return previous or { tab = "files", open = {}, folded = {}, selected = nil }
	end,

	update = function(state: State, msg)
		if type(msg) ~= "table" then
			if msg == "collapse" then
				state.open = {}
				state.folded = {}
				return state, { { to = "provider:projects", msg = { open = "" } } }
			elseif msg == "refresh" then
				return state, { { to = "provider:projects", msg = { refresh = true } } }
			end
			return
		end
		if msg.tab then
			state.tab = msg.tab
		elseif msg.toggle then
			state.open[msg.toggle] = if state.open[msg.toggle] then nil else true
			return state, { { to = "provider:projects", msg = { open = open_list(state.open) } } }
		elseif msg.fold then
			state.folded[msg.fold] = if state.folded[msg.fold] then nil else true
		elseif msg.pick then
			state.selected = msg.pick
			return state, { { to = "editor", msg = { open = msg.pick } } }
		elseif msg.project then
			state.open = {}
			state.folded = {}
			state.selected = nil
		end
	end,

	view = function(state: State, app)
		local projects = app.projects
		local current = projects and projects.current

		local function tab(id: string, label: string)
			return ui.row {
				class = "files-tab",
				selected = state.tab == id,
				msg = { tab = id },
				ui.text { label, class = "files-tab-label" },
			}
		end

		local top = ui.row {
			class = "files-top",
			drag = true,
			height = 52,
			align = "center",
			tab("changes", "Changes"),
			tab("files", "Files"),
			ui.spacer(),
			icon_button { icon = "panel-right", tooltip = "Hide files", msg = "toggle-right", to = "shell" },
		}

		local body
		if not current then
			body = ui.text { if projects then "No project open" else "Loading…", tone = "muted", style = { padding = { 8, 18 } } }
		else
			local marks = {}
			for _, line in current.files do
				local status, path = string.match(line, "^(%a)\t%d+\t%d+\t(.+)$")
				if path then
					marks[path] = status
				end
			end
			local header = ui.row {
				class = "files-path",
				align = "center",
				ui.text { `{current.home}/{current.name}`, class = "files-path-text", truncate = true },
				ui.spacer(),
				icon_button { icon = "refresh-cw", tooltip = "Look again", size = "small", msg = "refresh" },
				icon_button { icon = "chevrons-up-down", tooltip = "Collapse all", size = "small", msg = "collapse" },
			}
			if state.tab == "files" then
				body = ui.column {
					grow = true,
					gap = "none",
					header,
					ui.column {
						class = "files-body",
						grow = true,
						scroll = true,
						file_tree { dirs = projects.dirs, open = state.open, selected = state.selected, marks = marks },
					},
				}
			else
				body = ui.column {
					grow = true,
					gap = "none",
					header,
					ui.row {
						class = "files-summary",
						align = "center",
						ui.text { `{current.count} {if current.count == 1 then "file" else "files"}`, tone = "muted" },
						diff_counts { added = current.added, removed = current.deleted, size = "normal" },
					},
					ui.column {
						class = "files-body",
						grow = true,
						scroll = true,
						if current.count == 0
							then ui.text { "Nothing uncommitted", tone = "muted", style = { padding = { 4, 10 } } }
							else change_tree { files = current.files, folded = state.folded, selected = state.selected, pick = "pick" },
					},
				}
			end
		end

		return ui.column { grow = true, gap = "none", top, body }
	end,
}
