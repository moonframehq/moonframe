-- The worktree's files, changes and checks. Files is its file tree with the
-- changed files marked; Changes lists what changed in its last commit, its
-- uncommitted work, or its whole branch, as a tree of folders; Checks is
-- its pull requests and workflow runs, from `gh`. Picking a file shows it
-- in a sheet over the window.

local icon_button = use "icon-button"
local segmented = use "segmented"
local diff_counts = use "diff-counts"
local change_tree = use "change-tree"
local file_tree = use "file-tree"
local code = use "code"

type State = {
	tab: string,
	scope: string,
	folded: { [string]: boolean? },
	open: { [string]: boolean? },
	file: string?,
	limit: number,
	branch: string?,
}

local function open_list(open: { [string]: boolean? }): string
	local paths = {}
	for path in open do
		table.insert(paths, path)
	end
	table.sort(paths)
	return table.concat(paths, "\n")
end

-- A run's or pull request's look: its icon and tone.
local function run_mark(run: any): (string, string)
	if run.status ~= "completed" then
		return "loader", "warning"
	elseif run.conclusion == "success" then
		return "circle-check", "success"
	elseif run.conclusion == "skipped" or run.conclusion == "cancelled" then
		return "circle", "muted"
	end
	return "circle-alert", "danger"
end

local function checks_view(state: State, app: any): any
	local forge = app.projects and app.projects.forge
	if not forge then
		return ui.text { "Asking GitHub…", tone = "muted", style = { padding = { 8, 12 } } }
	elseif forge.error then
		return ui.text { forge.error, tone = "muted", wrap = true, style = { padding = { 8, 12 } } }
	end
	local branch = state.branch
	local rows = {}
	local prs = {}
	for _, pr in forge.prs or {} do
		if not branch or pr.branch == branch then
			table.insert(prs, pr)
		end
	end
	if #prs > 0 then
		table.insert(rows, ui.text { "Pull request", class = "checks-heading" })
		for _, pr in prs do
			table.insert(rows, ui.row { class = "checks-row", align = "center",
				ui.icon { name = "git-pull-request", size = "small", tone = if pr.draft then "muted" else "success" },
				ui.text { `#{pr.number} {pr.title}`, truncate = true },
			})
		end
	end
	local runs = {}
	for _, run in forge.runs or {} do
		if not branch or run.branch == branch then
			table.insert(runs, run)
		end
	end
	table.insert(rows, ui.text { if branch then `Workflow runs on {branch}` else "Workflow runs", class = "checks-heading" })
	if #runs == 0 then
		table.insert(rows, ui.text { "No runs yet", tone = "muted", style = { padding = { 2, 8 } } })
	end
	for _, run in runs do
		local icon, tone = run_mark(run)
		table.insert(rows, ui.row { class = "checks-row", align = "center",
			ui.icon { name = icon :: any, size = "small", tone = tone :: any, spin = icon == "loader" },
			ui.column { gap = "none", grow = true,
				ui.text { run.title, truncate = true },
				ui.text { run.workflow, class = "changes-count", size = "small", truncate = true },
			},
		})
	end
	return ui.column { class = "changes-body", grow = true, scroll = true, table.unpack(rows) }
end

return mod {
	title = "Inspector",
	subscribe = { "switchyard/workspace" },

	styles = {
		[".changes-top"] = { padding = { 0, 10, 0, 12 }, gap = 4 },
		[".changes-tab"] = { padding = { 5, 12 }, radius = 9, color = "muted-foreground" },
		[".changes-tab:hover"] = { background = "list-hover" },
		[".changes-tab:selected"] = { background = "card", color = "foreground", shadow = "0 1px 2px #00000012" },
		[".changes-tab-label"] = { weight = "medium" },
		[".changes-body"] = { padding = { 0, 10, 10, 10 }, gap = 8 },
		[".changes-summary"] = { padding = { 2, 6 }, gap = 8 },
		[".changes-count"] = { color = "muted-foreground" },
		[".changes-path"] = { color = "muted-foreground", padding = { 2, 6 } },
		[".changes-sheet-header"] = { padding = { 12, 16 }, gap = 8, border = "bottom", border_color = "border" },
		[".changes-sheet"] = { background = "card", radius = 14, shadow = "large" },
		[".checks-heading"] = { color = "muted-foreground", size = "small", weight = "medium", padding = { 8, 6, 2, 6 } },
		[".checks-row"] = { padding = { 6, 8 }, radius = 8, gap = 8 },
		[".checks-row:hover"] = { background = "list-hover" },
	},

	init = function(previous: State?): State
		return previous or { tab = "files", scope = "uncommitted", folded = {}, open = {}, file = nil, limit = 400 }
	end,

	update = function(state: State, msg)
		if type(msg) ~= "table" then
			if msg == "close" then
				state.file = nil
				return state, { { to = "provider:projects", msg = { read = false } } }
			elseif msg == "more" then
				state.limit += 1000
			end
			return
		end
		if msg.topic == "switchyard/workspace" then
			-- Another worktree: its folders start shut.
			state.folded = {}
			state.open = {}
			state.file = nil
			state.branch = msg.value and msg.value.branch
		elseif msg.tab then
			state.tab = msg.tab
			return state, { { to = "provider:projects", msg = { forge = msg.tab == "checks" } } }
		elseif msg.scope then
			state.scope = msg.scope
			return state, { { to = "provider:projects", msg = { scope = msg.scope } } }
		elseif msg.fold then
			state.folded[msg.fold] = if state.folded[msg.fold] then nil else true
		elseif msg.toggle then
			state.open[msg.toggle] = if state.open[msg.toggle] then nil else true
			return state, { { to = "provider:projects", msg = { open = open_list(state.open) } } }
		elseif msg.pick then
			state.file = msg.pick
			state.limit = 400
			return state, { { to = "provider:projects", msg = { read = msg.pick } } }
		end
	end,

	view = function(state: State, app)
		local projects = app.projects
		local current = projects and projects.current

		local function tab(id: string, label: string)
			return ui.row {
				class = "changes-tab",
				selected = state.tab == id,
				msg = { tab = id },
				ui.text { label, class = "changes-tab-label" },
			}
		end

		local top = ui.row {
			class = "changes-top",
			drag = true,
			height = 52,
			align = "center",
			tab("files", "Files"),
			tab("changes", if current and current.count > 0 then `Changes {current.count}` else "Changes"),
			tab("checks", "Checks"),
			ui.spacer(),
			icon_button { icon = "panel-right", tooltip = "Hide the inspector", msg = "toggle-right", to = "shell" },
		}

		local body
		if not current then
			body = ui.text { if projects then "No worktree open" else "Loading…", tone = "muted", style = { padding = { 8, 12 } } }
		elseif state.tab == "checks" then
			body = checks_view(state, app)
		elseif state.tab == "changes" then
			local paths = {}
			for _, line in current.files do
				table.insert(paths, (string.match(line, "^%a\t%d+\t%d+\t(.+)$")))
			end
			body = ui.column {
				class = "changes-body",
				grow = true,
				ui.row { segmented {
					grow = true,
					event = "scope",
					selected = current.scope,
					options = {
						{ id = "last", label = "Last Commit" },
						{ id = "uncommitted", label = "Uncommitted" },
						{ id = "branch", label = "Branch" },
					},
				} },
				ui.row {
					class = "changes-summary",
					align = "center",
					ui.text { `{current.count} {if current.count == 1 then "file" else "files"}`, class = "changes-count" },
					diff_counts { added = current.added, removed = current.deleted, size = "normal" },
					ui.spacer(),
					ui.button {
						icon = "copy",
						variant = "ghost",
						small = true,
						tooltip = "Copy the list of files",
						copy = table.concat(paths, "\n"),
					},
				},
				if current.count == 0
					then ui.text { "Nothing changed", tone = "muted", style = { padding = { 4, 6 } } }
					else ui.column {
						grow = true,
						scroll = true,
						gap = "none",
						change_tree { files = current.files, folded = state.folded, selected = state.file },
						current.truncated and ui.text { `and {current.count - #current.files} more`, tone = "muted", size = "small" },
					},
			}
		else
			local marks = {}
			for _, line in current.files do
				local status, path = string.match(line, "^(%a)\t%d+\t%d+\t(.+)$")
				if path then
					marks[path] = status
				end
			end
			body = ui.column {
				class = "changes-body",
				grow = true,
				ui.text { `{current.home}/{current.name}`, class = "changes-path", truncate = true },
				ui.column {
					grow = true,
					scroll = true,
					gap = "none",
					file_tree { dirs = projects.dirs, open = state.open, selected = state.file, marks = marks },
				},
			}
		end

		local file = projects and projects.file
		local sheet = ui.overlay {
			open = state.file ~= nil,
			dismiss = "close",
			place = "right",
			width = 760,
			backdrop = true,
			class = "changes-sheet",
			content = ui.column {
				grow = true,
				gap = "none",
				ui.row {
					class = "changes-sheet-header",
					align = "center",
					ui.icon { name = "file", tone = "muted" },
					ui.text { state.file or "", weight = "medium", truncate = true },
					ui.spacer(),
					icon_button { icon = "close", tooltip = "Close", msg = "close" },
				},
				if not file or file.path ~= state.file
					then ui.text { "Reading…", tone = "muted", style = { padding = 16 } }
					elseif file.missing then ui.text { "The file is gone", tone = "muted", style = { padding = 16 } }
					elseif file.binary then ui.text { "A binary file", tone = "muted", style = { padding = 16 } }
					else code { lines = file.lines, path = file.path, limit = state.limit, total = file.total, more = "more" },
			},
		}

		return ui.column { grow = true, gap = "none", top, body, sheet }
	end,
}
