-- The right column: what a thread touches. Changes to review, stage and
-- commit; the files, to read and edit; terminals in the checkout; and its
-- worktrees. Each thread keeps its own open tab.

local checkbox = use "checkbox"
local diff_view = use "diff-view"
local code_view = use "code-view"
local empty_state = use "empty-state"
local dropdown = use "dropdown"
local dialog = use "dialog"
local file_icon = use "file-icon"
local terminal = use "terminal"

type Term = { id: string, cwd: string, title: string }
type State = {
	at: string?,
	thread: string?,
	tab: string,
	tabs: { [string]: string },
	reviewed: { [string]: { [string]: boolean } },
	compare: string,
	range: { from: string, to: string }?,
	compare_open: boolean,
	commit: string,
	terminals: { Term },
	terminal: string?,
	serial: number,
	editing: boolean,
	-- The file the edit is of: what's open can change while it's going on.
	edit: { path: string, version: string? }?,
	blocked: boolean,
	text: string,
	split: boolean,
	removing: string?,
}

local MAX_TERMINALS = 8
local STATUS_WORDS: { [string]: string } = { M = "modified", A = "added", D = "deleted", R = "renamed", ["?"] = "new", U = "conflict" }
local STATUS_CLASS: { [string]: string } = { M = "mark-modified", A = "mark-added", D = "mark-deleted", ["?"] = "mark-added", U = "mark-deleted" }

-- What the view last read, for `update`, which has no `app`.
local latest_repo: any = nil

local function reviewed_count(state: State, repo: any): number
	local marks = state.reviewed[repo.at] or {}
	local count = 0
	for _, file in repo.files do
		if marks[file.path] then
			count += 1
		end
	end
	return count
end

local function tab(id: string, label: string, icon: string, state: State): any
	return ui.row {
		class = "bench-tab",
		selected = state.tab == id,
		align = "center",
		msg = { tab = id },
		ui.icon { icon :: any },
		ui.text { label, wrap = false },
	}
end

local function file_row(file: any, state: State, repo: any): any
	local marks = state.reviewed[repo.at] or {}
	local uncommitted = repo.compare.mode == "uncommitted"
	return ui.row {
		class = "file-row",
		selected = repo.picked == file.path,
		align = "center",
		msg = { pick = file.path },
		checkbox { checked = marks[file.path] == true, msg = { reviewed = file.path } },
		ui.row { class = STATUS_CLASS[file.status] or "mark-modified", shrink = false },
		ui.text { file.path, truncate = true, class = "file-path" },
		ui.spacer(),
		(file.added > 0 or file.deleted > 0) and ui.text { `+{file.added} −{file.deleted}`, class = "file-word", wrap = false },
		ui.text { STATUS_WORDS[file.status] or file.status, class = "file-word", wrap = false },
		uncommitted and ui.button {
			if file.staged and not file.unstaged then "Unstage" else "Stage",
			small = true,
			msg = if file.staged and not file.unstaged then { unstage = file.path } else { stage = file.path },
			to = "provider:repo",
		},
	}
end

local function changes(state: State, repo: any): any
	if not repo.git then
		return empty_state { icon = "git-compare", title = "Not a git repository", message = "Changes show for a folder under git." }
	end
	local files: { any } = {}
	for _, file in repo.files do
		table.insert(files, file_row(file, state, repo))
	end
	local label = if repo.compare.mode == "branch" then `Branch vs {repo.base_name or "base"}`
		elseif repo.compare.mode == "range" then "This turn"
		else "Uncommitted"
	local options: { any } = { { id = "uncommitted", label = "Uncommitted", detail = "Working tree" } }
	if repo.base then
		table.insert(options, { id = "branch", label = `Branch vs {repo.base_name}` })
	end
	if state.range then
		table.insert(options, { id = "range", label = "This turn" })
	end
	local diff = repo.diff
	local uncommitted = repo.compare.mode == "uncommitted"
	return ui.column {
		grow = true,
		gap = "none",
		ui.row {
			class = "bench-head",
			align = "center",
			shrink = false,
			ui.text { "Changes", weight = "semibold", size = "large" },
			ui.spacer(),
			ui.text { `Reviewed {reviewed_count(state, repo)} of {#repo.files}`, tone = "muted", wrap = false },
			ui.row { class = "quiet-button", msg = { refresh = true }, to = "provider:repo", tooltip = "Refresh", ui.icon { "refresh-cw" } },
		},
		ui.row {
			class = "bench-tools",
			align = "center",
			shrink = false,
			dropdown { label = "Compare", value = label, open = state.compare_open, toggle = "toggle-compare", dismiss = "close-compare", selected = repo.compare.mode, options = options, event = "compare_pick", width = 240 },
			ui.spacer(),
			repo.branch and ui.text { repo.branch, class = "file-word", wrap = false },
			repo.head and ui.text { repo.head, class = "file-sha" },
		},
		repo.error and ui.row { class = "bench-error", ui.text { repo.error, wrap = true } },
		ui.vsplit {
			id = "changes",
			divider = true,
			ui.pane {
				size = 220,
				min = 90,
				fixed = true,
				ui.column {
					grow = true,
					gap = "none",
					ui.row {
						class = "files-head",
						align = "center",
						shrink = false,
						ui.text { "CHANGED FILES", class = "files-title" },
						ui.spacer(),
						ui.text { tostring(#repo.files), class = "files-title" },
					},
					ui.column {
						grow = true,
						scroll = true,
						gap = "none",
						ui.column {
							shrink = false,
							gap = "none",
							#files == 0 and ui.text { "Nothing has changed.", class = "bench-quiet" },
							table.unpack(files),
						},
					},
				},
			},
			ui.pane {
				min = 120,
				if diff
					then ui.column {
						grow = true,
						gap = "none",
						ui.row {
							class = "diff-head",
							align = "center",
							shrink = false,
							ui.text { diff.path, truncate = true, class = "file-path" },
							ui.spacer(),
							ui.button { "Open in Files", small = true, variant = "ghost", msg = { open_file = diff.path } },
							ui.button { if state.split then "Unified" else "Split", small = true, variant = "ghost", msg = "toggle-split" },
						},
						if diff.binary
							then ui.text { "A binary file.", class = "bench-quiet" }
							else diff_view { path = diff.path, status = diff.status, hunks = diff.hunks, split = state.split, header = false, grow = true },
					}
					else ui.column { grow = true, justify = "center", ui.text { "Pick a file to see its changes.", class = "bench-quiet" } },
			},
		},
		uncommitted and #repo.files > 0 and ui.row {
			class = "commit-row",
			align = "center",
			shrink = false,
			ui.row {
				class = "page-field",
				grow = true,
				ui.input { id = "commit", grow = true, value = state.commit, placeholder = "Commit message", change = "commit-text", submit = "commit" },
			},
			ui.button { if repo.busy == "commit" then "Committing…" else "Commit", variant = "primary", disabled = state.commit == "" or repo.busy ~= nil, msg = "commit" },
		},
	}
end

local function tree_rows(repo: any, path: string, depth: number, rows: { any })
	for _, entry in repo.tree[path] or {} do
		local full = if path == "" then entry.name else `{path}/{entry.name}`
		local open = repo.tree[full] ~= nil
		table.insert(rows, ui.row {
			class = "tree-entry",
			selected = repo.file ~= nil and repo.file.path == full,
			align = "center",
			msg = if entry.dir then { open_dir = full } else { open_file = full },
			to = if entry.dir then "provider:repo" else nil,
			ui.row { width = depth * 14, shrink = false },
			if entry.dir then ui.icon { (if open then "chevron-down" else "chevron-right") :: any, class = "tree-chevron" } else ui.row { width = 14, shrink = false },
			file_icon { name = entry.name, dir = entry.dir, open = open },
			ui.text { entry.name, truncate = true },
		})
		if entry.dir and open and #rows < 800 then
			tree_rows(repo, full, depth + 1, rows)
		end
	end
end

local function file_body(state: State, repo: any): any
	local file = repo.file
	if not file then
		return ui.column { grow = true, justify = "center", ui.text { "Pick a file.", class = "bench-quiet" } }
	end
	local notice = if file.missing then file.reason elseif file.binary then "A binary file." elseif file.tooLarge then "Over 1 MB, too large to open here." else nil
	return ui.column {
		grow = true,
		gap = "none",
		ui.row {
			class = "diff-head",
			align = "center",
			shrink = false,
			ui.text { file.path, truncate = true, class = "file-path" },
			ui.spacer(),
			file.conflict and ui.text { "Changed on disk", tone = "warning", size = "small", wrap = false },
			state.blocked and ui.text { "Save or cancel this edit first", tone = "warning", size = "small", wrap = false },
			file.text and not state.editing and ui.button { "Edit", small = true, variant = "ghost", msg = "edit" },
			state.editing and ui.button { "Cancel", small = true, variant = "ghost", msg = "cancel-edit" },
			state.editing and ui.button { "Save", small = true, variant = "primary", msg = "save-file" },
		},
		if notice
			then ui.text { notice, class = "bench-quiet" }
			elseif state.editing then ui.column {
				grow = true,
				class = "file-editor",
				ui.input { id = "file-text", class = "file-editor-input", multiline = true, grow = true, mono = true, focus = true, value = state.text, change = "file-text", keys = { ["secondary-s"] = "save-file" } },
			}
			else code_view { id = "file", text = file.text, language = string.match(file.path, "%.(%w+)$"), grow = true },
	}
end

local function files(state: State, repo: any): any
	local rows: { any } = {}
	tree_rows(repo, "", 0, rows)
	return ui.vsplit {
		id = "files",
		divider = true,
		ui.pane {
			size = 260,
			min = 100,
			fixed = true,
			ui.column { grow = true, scroll = true, gap = "none", ui.column { shrink = false, gap = "none", style = { padding = { 6, 0 } }, table.unpack(rows) } },
		},
		ui.pane { min = 120, file_body(state, repo) },
	}
end

local function terminals(state: State): any
	local bar: { any } = {}
	local current: Term? = nil
	for index, term in state.terminals do
		if term.id == state.terminal then
			current = term
		end
		table.insert(bar, ui.row {
			class = "term-tab",
			selected = term.id == state.terminal,
			align = "center",
			msg = { terminal = term.id },
			ui.row { class = "term-dot" },
			ui.text { `Terminal {index}`, wrap = false },
			ui.row { class = "quiet-button", msg = { close_terminal = term.id }, ui.icon { "close" } },
		})
	end
	return ui.column {
		grow = true,
		gap = "none",
		ui.row {
			class = "term-bar",
			align = "center",
			shrink = false,
			ui.row { gap = "small", table.unpack(bar) },
			ui.spacer(),
			#state.terminals < MAX_TERMINALS and ui.row { class = "quiet-button", msg = "new-terminal", tooltip = "New terminal", ui.icon { "plus" } },
		},
		if current
			then ui.column { grow = true, terminal { id = current.id, cwd = current.cwd, exit = { terminal_exit = current.id } } }
			else ui.column { grow = true, justify = "center", empty_state { icon = "square-terminal", title = "No terminal", action = "New terminal", msg = "new-terminal" } },
	}
end

local function worktrees(repo: any): any
	if not repo.git then
		return empty_state { icon = "folder-git-2", title = "Not a git repository" }
	end
	local rows: { any } = {}
	for index, worktree in repo.worktrees do
		table.insert(rows, ui.row {
			class = "worktree-row",
			align = "center",
			ui.icon { "folder-git-2", class = "tree-chevron" },
			ui.column {
				grow = true,
				gap = "none",
				ui.row { align = "center", gap = "small", ui.text { worktree.branch or "detached", weight = "medium", truncate = true }, worktree.current and ui.text { "here", class = "scope-chip" } },
				ui.text { worktree.path, class = "file-word", truncate = true },
			},
			index > 1 and not worktree.current and ui.button { "Remove", small = true, variant = "ghost", msg = { removing = worktree.path } },
		})
	end
	return ui.column {
		grow = true,
		gap = "none",
		ui.row { class = "bench-head", align = "center", shrink = false, ui.text { "Worktrees", weight = "semibold", size = "large" } },
		ui.column { grow = true, scroll = true, gap = "none", ui.column { shrink = false, gap = "none", table.unpack(rows) } },
	}
end

local function new_terminal(state: State): (State, { any })
	if #state.terminals >= MAX_TERMINALS then
		return state, {}
	end
	state.serial += 1
	local term = { id = `term-{state.serial}`, cwd = state.at or "~", title = "Terminal" }
	table.insert(state.terminals, term)
	state.terminal = term.id
	return state, {}
end

local function drop_terminal(state: State, id: string)
	for index, term in state.terminals do
		if term.id == id then
			table.remove(state.terminals, index)
			break
		end
	end
	if state.terminal == id then
		local last = state.terminals[#state.terminals]
		state.terminal = if last then last.id else nil
	end
end

local function show_tab(state: State, id: string)
	state.tab = id
	if state.thread then
		state.tabs[state.thread] = id
	end
end

return mod {
	subscribe = { "pilot/thread" },

	styles = {
		[".bench-bar"] = { padding = { 0, 8, 0, 12 }, border = "bottom", gap = 4 },
		[".bench-tab"] = { padding = { 6, 10 }, radius = 8, gap = 7, color = "muted-foreground" },
		[".bench-tab:hover"] = { background = "list-hover" },
		[".bench-tab:selected"] = { background = "list-active", color = "foreground" },
		[".bench-head"] = { padding = { 12, 16 }, gap = 8, border = "bottom" },
		[".bench-tools"] = { padding = { 8, 16 }, gap = 8, border = "bottom" },
		[".bench-error"] = { padding = { 8, 16 }, background = "danger/10", size = "small" },
		[".bench-quiet"] = { padding = { 16, 16 }, color = "muted-foreground" },
		[".files-head"] = { padding = { 8, 16 } },
		[".files-title"] = { size = "small", weight = "semibold", color = "muted-foreground" },
		[".file-row"] = { padding = { 7, 12, 7, 16 }, gap = 10 },
		[".file-row:hover"] = { background = "list-hover" },
		[".file-row:selected"] = { background = "list-hover" },
		[".file-path"] = { font = "mono", size = "small" },
		[".file-word"] = { size = "small", color = "muted-foreground" },
		[".file-sha"] = { font = "mono", size = "small", color = "muted-foreground" },
		[".mark-modified"] = { width = 8, height = 8, radius = 4, background = "warning" },
		[".mark-added"] = { width = 8, height = 8, radius = 4, background = "success" },
		[".mark-deleted"] = { width = 8, height = 8, radius = 4, background = "danger" },
		[".diff-head"] = { padding = { 7, 16 }, gap = 6, background = "secondary", border = "bottom" },
		[".commit-row"] = { padding = { 10, 16 }, gap = 8, border = "top" },
		[".tree-entry"] = { padding = { 3, 8 }, gap = 6, radius = 6, margin = { 0, 6 } },
		[".tree-entry:hover"] = { background = "list-hover" },
		[".tree-entry:selected"] = { background = "list-active" },
		[".tree-chevron"] = { color = "muted-foreground" },
		[".file-editor"] = { padding = { 8, 12 } },
		[".term-bar"] = { padding = { 6, 10 }, border = "bottom", gap = 6 },
		[".term-tab"] = { padding = { 3, 4, 3, 10 }, radius = 7, gap = 6, color = "muted-foreground" },
		[".term-tab:hover"] = { background = "list-hover" },
		[".term-tab:selected"] = { border = true, color = "foreground" },
		[".term-dot"] = { width = 7, height = 7, radius = 4, background = "success" },
		[".worktree-row"] = { padding = { 10, 16 }, gap = 10, border = "bottom" },
	},

	init = function(previous: State?): State
		local state: State = previous or {
			tab = "changes",
			tabs = {},
			reviewed = {},
			compare = "uncommitted",
			compare_open = false,
			commit = "",
			terminals = {},
			serial = 0,
			editing = false,
			blocked = false,
			text = "",
			split = false,
		}
		state.compare_open = false
		state.editing = false
		state.edit = nil
		state.blocked = false
		state.at = nil
		state.range = nil
		state.removing = nil
		return state
	end,

	update = function(state: State, msg: any)
		if msg == "toggle-compare" then
			state.compare_open = not state.compare_open
		elseif msg == "close-compare" then
			state.compare_open = false
		elseif msg == "toggle-split" then
			state.split = not state.split
		elseif msg == "new-terminal" then
			return new_terminal(state)
		elseif msg == "cancel-remove" then
			state.removing = nil
		elseif msg == "confirm-remove" then
			local path = state.removing
			state.removing = nil
			if path then
				return state, { { to = "provider:repo", msg = { remove_worktree = path } } }
			end
		elseif msg == "commit" then
			local text = state.commit
			state.commit = ""
			if text ~= "" then
				return state, { { to = "provider:repo", msg = { commit = text } } }
			end
		elseif msg == "edit" then
			local file = latest_repo and latest_repo.file
			if file then
				state.editing = true
				state.edit = { path = file.path, version = file.version }
				-- After a conflict, the words the save didn't write.
				if not file.conflict then
					state.text = file.text or ""
				end
			end
		elseif msg == "cancel-edit" then
			state.editing = false
			state.edit = nil
			state.blocked = false
		elseif msg == "save-file" or (type(msg) == "table" and msg.msg == "save-file") then
			local edit = state.edit
			state.editing = false
			state.edit = nil
			state.blocked = false
			if edit then
				return state, { { to = "provider:repo", msg = { write = edit.path, text = state.text, version = edit.version } } }
			end
		elseif type(msg) ~= "table" then
			return
		elseif msg.topic == "pilot/thread" then
			local thread = msg.value
			if not thread or not thread.id then
				return
			end
			local sends: { any } = {}
			if thread.id ~= state.thread then
				state.thread = thread.id
				state.tab = state.tabs[thread.id] or state.tab
				state.range = nil
				state.editing = false
				state.edit = nil
				state.blocked = false
			end
			if thread.cwd and thread.cwd ~= state.at then
				state.at = thread.cwd
				table.insert(sends, { to = "provider:repo", msg = { at = thread.cwd } })
			end
			return state, sends
		elseif msg.tab then
			show_tab(state, msg.tab)
			if msg.tab == "terminal" and #state.terminals == 0 then
				return new_terminal(state)
			end
		elseif msg.toggle_tab then
			if state.tab == msg.toggle_tab then
				return state, { { to = "shell", msg = "toggle-workbench" } }
			end
			show_tab(state, msg.toggle_tab)
			if msg.toggle_tab == "terminal" and #state.terminals == 0 then
				new_terminal(state)
			end
			return state, { { to = "shell", msg = "show-workbench" } }
		elseif msg.pick then
			return state, { { to = "provider:repo", msg = { pick = msg.pick } } }
		elseif msg.reviewed then
			local at = state.at or ""
			state.reviewed[at] = state.reviewed[at] or {}
			state.reviewed[at][msg.reviewed] = not state.reviewed[at][msg.reviewed]
		elseif msg.compare_pick then
			state.compare_open = false
			if msg.compare_pick == "range" and state.range then
				return state, { { to = "provider:repo", msg = { compare = "range", from = state.range.from, to = state.range.to } } }
			end
			return state, { { to = "provider:repo", msg = { compare = msg.compare_pick } } }
		elseif msg.review then
			-- A run's changes, from its card in the timeline.
			show_tab(state, "changes")
			state.range = { from = msg.review.from, to = msg.review.to }
			local sends: { any } = {
				{ to = "shell", msg = "show-workbench" },
				{ to = "provider:repo", msg = { compare = "range", from = msg.review.from, to = msg.review.to, pick = msg.review.path } },
			}
			return state, sends
		elseif msg.review_tool then
			show_tab(state, "changes")
			local path = string.match(msg.review_tool, "^%a+ (.+)$")
			local sends: { any } = { { to = "shell", msg = "show-workbench" }, { to = "provider:repo", msg = { compare = "uncommitted", pick = path } } }
			return state, sends
		elseif msg.open_file then
			show_tab(state, "files")
			if state.edit and state.edit.path ~= msg.open_file then
				state.blocked = true
				return state, { { to = "shell", msg = "show-workbench" } }
			end
			local sends: { any } = { { to = "shell", msg = "show-workbench" }, { to = "provider:repo", msg = { read = msg.open_file } } }
			-- Open the folders down to it.
			local dir = ""
			for part in string.gmatch(msg.open_file, "([^/]+)/") do
				dir = if dir == "" then part else `{dir}/{part}`
				if not (latest_repo and latest_repo.tree[dir]) then
					table.insert(sends, { to = "provider:repo", msg = { open_dir = dir } })
				end
			end
			return state, sends
		elseif msg.msg == "commit-text" then
			state.commit = msg.value
		elseif msg.msg == "file-text" then
			state.text = msg.value
		elseif msg.removing then
			state.removing = msg.removing
		elseif msg.terminal then
			state.terminal = msg.terminal
		elseif msg.close_terminal then
			drop_terminal(state, msg.close_terminal)
		elseif msg.terminal_exit then
			drop_terminal(state, msg.terminal_exit)
		end
	end,

	view = function(state: State, app: any)
		local repo = app.repo
		latest_repo = repo
		local body: any = nil
		if state.tab == "terminal" then
			body = terminals(state)
		elseif not repo or not repo.at then
			body = ui.column { grow = true }
		elseif state.tab == "files" then
			body = files(state, repo)
		elseif state.tab == "worktrees" then
			body = worktrees(repo)
		else
			body = changes(state, repo)
		end
		return ui.column {
			grow = true,
			gap = "none",
			ui.row {
				class = "bench-bar",
				drag = true,
				height = 52,
				shrink = false,
				align = "center",
				tab("changes", "Changes", "git-compare", state),
				tab("files", "Files", "folder", state),
				tab("terminal", "Terminal", "square-terminal", state),
				tab("worktrees", "Worktrees", "folder-git-2", state),
				ui.spacer(),
				ui.row { class = "quiet-button", msg = "toggle-workbench", to = "shell", tooltip = "Hide workbench", ui.icon { "panel-right" } },
			},
			body,
			dialog {
				open = state.removing ~= nil,
				dismiss = "cancel-remove",
				title = "Remove this worktree?",
				size = "small",
				plain = true,
				buttons = { { label = "Cancel", msg = "cancel-remove", variant = "ghost" }, { label = "Remove", msg = "confirm-remove", variant = "danger" } },
				ui.text { "Its folder goes, with any changes not committed there. Its branch stays.", wrap = true },
			},
		}
	end,
}
