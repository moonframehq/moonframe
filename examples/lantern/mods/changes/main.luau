-- A changes pane: the checkout (a menu of the repository's worktrees), the
-- working tree or what is staged, the changed files grouped by kind, and
-- one file's diff open under its row. From the built-in `git` provider, so
-- it shows the repository the app runs from.

local icon_button = use "icon-button"
local menu = use "menu"
local diff_view = use "diff-view"

-- `shut` is set once the user folds the open file, so none opens by itself.
type State = { open: string?, staged: boolean, picking: boolean, shut: boolean? }

local GROUPS = {
	{ status = "?", title = "Untracked", tone = "success" },
	{ status = "U", title = "Conflicts", tone = "danger" },
	{ status = "M", title = "Modified", tone = "warning" },
	{ status = "A", title = "Added", tone = "success" },
	{ status = "R", title = "Renamed", tone = "accent" },
	{ status = "D", title = "Deleted", tone = "danger" },
}

-- The first changed file a reader would open: a file, not a new folder.
local function first(files: { any }): string?
	for _, group in GROUPS do
		for _, file in files do
			if file.status == group.status and string.sub(file.path, -1) ~= "/" then
				return file.path
			end
		end
	end
	return nil
end

return mod {
	title = "Changes",
	subscribe = { "provider/git" },

	styles = {
		[".chg-bar"] = { padding = { 8, 10 }, gap = 8, border = "bottom", border_color = "border" },
		[".chg-pick"] = { background = "field", radius = 16, padding = { 0, 12 }, gap = 8 },
		[".chg-pick:hover"] = { background = "row-selected" },
		[".chg-scope"] = { background = "field", radius = 16, padding = { 0, 12 } },
		[".chg-scope:hover"] = { background = "row-selected" },
		[".chg-group"] = { padding = { 12, 10, 6, 10 }, color = "muted-foreground" },
		[".chg-group-text"] = { size = "small", weight = "medium" },
		[".chg-card"] = { margin = { 0, 8, 6, 8 }, border = true, border_color = "border", radius = 6, gap = 0 },
		[".chg-file"] = { padding = { 0, 8 }, gap = 6 },
		[".chg-file:hover"] = { background = "list-hover" },
		[".chg-dir"] = { color = "muted-foreground" },
		[".chg-name"] = { weight = "medium" },
		[".chg-status"] = { font = "mono", weight = "bold" },
		[".chg-empty"] = { padding = { 16, 12 } },
		[".chg-menu"] = { padding = 0 },
	},

	init = function(previous: State?)
		local state: State = previous or { open = nil, staged = false, picking = false }
		state.picking = false
		if state.open then
			return state, { { to = "provider:git", msg = { diff = state.open } } }
		end
		return state
	end,

	update = function(state: State, msg)
		if type(msg) == "table" and msg.topic == "provider/git" then
			-- Open the first change by itself, until the user folds one.
			local git = msg.value
			if state.open == nil and not state.shut and git and git.files then
				local path = first(git.files)
				if path then
					state.open = path
					return state, { { to = "provider:git", msg = { diff = path } } }
				end
			end
			return
		elseif msg == "pick" then
			state.picking = not state.picking
		elseif msg == "dismiss" then
			state.picking = false
		elseif msg == "scope" then
			state.staged = not state.staged
		elseif msg == "refresh" then
			return state, { { to = "provider:git", msg = { refresh = true } } }
		elseif type(msg) == "table" and msg.pick then
			-- Only this checkout's changes are read; the others just show.
			state.picking = false
		elseif type(msg) == "table" and msg.file then
			if state.open == msg.file then
				state.open = nil
				state.shut = true
				return state, { { to = "provider:git", msg = { diff = false } } }
			end
			state.open = msg.file
			return state, { { to = "provider:git", msg = { diff = msg.file } } }
		end
	end,

	view = function(state: State, app)
		local git = app.git
		if not git or not git.available or not git.repository then
			return ui.column {
				grow = true,
				align = "center",
				justify = "center",
				ui.text { if git then (git.reason or "Not a git repository") else "Reading git…", tone = "muted" },
			}
		end

		-- The checkout menu: every worktree, the current one ticked.
		local items: { any } = {}
		local label = "Main checkout"
		local worktrees: { any } = git.worktrees or {}
		for index, worktree in worktrees do
			local name = if index == 1 then "Main checkout" else (worktree.branch or string.match(worktree.path, "([^/]+)$") or worktree.path)
			if worktree.current then
				label = name
			end
			table.insert(items, {
				id = worktree.path,
				label = name,
				detail = `{worktree.changed}`,
				icon = if index == 1 then "folder" else "git-branch",
				checked = worktree.current,
				disabled = not worktree.current,
			})
		end

		local files: { any } = {}
		for _, file in git.files do
			if (state.staged and file.staged) or (not state.staged and (file.unstaged or file.status == "?")) then
				table.insert(files, file)
			end
		end

		local bar = ui.row {
			class = "chg-bar",
			align = "center",
			shrink = false,
			ui.popover {
				open = state.picking,
				dismiss = "dismiss",
				width = 260,
				content = ui.column { class = "chg-menu", menu { items = items } },
				ui.row {
					class = "chg-pick",
					height = 30,
					max_width = 180,
					align = "center",
					msg = "pick",
					tooltip = git.root,
					ui.text { `{label} · {git.changed}`, truncate = true },
					ui.icon { name = "chevron-down", size = "small", tone = "muted" },
				},
			},
			icon_button { icon = "git-compare", tooltip = "Look again", size = "small", msg = "refresh" },
			ui.row {
				class = "chg-scope",
				height = 30,
				align = "center",
				msg = "scope",
				tooltip = "Switch between the working tree and what is staged",
				ui.text { if state.staged then "Staged" else "Working tree" },
			},
		}

		local body: { any } = {}
		for _, group in GROUPS do
			local members: { any } = {}
			for _, file in files do
				if file.status == group.status then
					table.insert(members, file)
				end
			end
			if #members > 0 then
				table.insert(body, ui.row {
					class = "chg-group",
					align = "center",
					ui.text { string.upper(group.title), class = "chg-group-text" },
					ui.spacer(),
					ui.text { tostring(#members), class = "chg-group-text" },
				})
				for _, file in members do
					local open = state.open == file.path
					local dir, name = string.match(file.path, "^(.*/)([^/]+)$")
					local diff = git.diff
					table.insert(body, ui.column {
						class = "chg-card",
						shrink = false,
						ui.row {
							class = "chg-file",
							height = 30,
							align = "center",
							msg = { file = file.path },
							tooltip = file.path,
							ui.icon { name = if open then "chevron-down" else "chevron-right", size = "small", tone = "muted" },
							ui.text { file.status, class = "chg-status", tone = group.tone :: any },
							dir ~= nil and ui.text { dir, class = "chg-dir", truncate = true },
							ui.text { name or file.path, class = "chg-name", truncate = true },
						},
						open and (if diff and diff.path == file.path
							then (if diff.binary
								then ui.text { "Binary file", tone = "muted", style = { padding = { 6, 10 } } }
								else diff_view { header = false, hunks = diff.hunks })
							else ui.text { "Reading the diff…", tone = "muted", style = { padding = { 6, 10 } } }),
					})
				end
			end
		end
		if #body == 0 then
			table.insert(body, ui.column {
				class = "chg-empty",
				align = "center",
				ui.text { if state.staged then "Nothing staged" else "Nothing changed", tone = "muted" },
			})
		end

		return ui.column {
			grow = true,
			gap = "none",
			bar,
			ui.column { grow = true, scroll = true, gap = "none", ui.column { gap = "none", shrink = false, table.unpack(body) } },
		}
	end,
}
