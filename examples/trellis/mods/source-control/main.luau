-- Source control for the work tree shown: its branch, then Branch (a
-- commit message, Commit, pull and push, and the changed files as a tree
-- with their counts), PRs and Actions (from `gh`, through the projects
-- provider), and History (its last commits). Commits go through the
-- projects provider, which lets this mod write (`source-control` is in its
-- writers) and answers on `app.projects.action`.

local icon_button = use "icon-button"
local change_tree = use "change-tree"
local relative_time = use "relative-time"
local empty_state = use "empty-state"
local loading = use "loading"
local ahead_behind = use "ahead-behind"

type State = {
	tab: string,
	message: string,
	folded: { [string]: boolean? },
	changes_open: boolean,
	pending: string?,
	result: { ok: boolean, text: string }?,
	made: number,
	focus: boolean,
}

local TABS = {
	{ id = "branch", label = "Branch", icon = "git-branch" },
	{ id = "prs", label = "PRs", icon = "git-pull-request" },
	{ id = "actions", label = "Actions", icon = "play" },
	{ id = "history", label = "History", icon = "history" },
}

local RUN_ICONS = { success = "circle-check", failure = "circle-x", cancelled = "circle-stop", skipped = "circle-dashed" }
local RUN_TONES = { success = "success", failure = "danger", cancelled = "muted", skipped = "muted" }

return mod {
	region = "right",
	title = "Source Control",
	subscribe = { "provider/projects" },

	styles = {
		[".sc-top"] = { padding = { 0, 8 }, gap = 2, border = "bottom", border_color = "border" },
		[".sc-head"] = { padding = { 0, 6, 0, 12 }, gap = 6, border = "bottom", border_color = "border" },
		[".sc-title"] = { weight = "bold" },
		[".sc-branch"] = { padding = { 0, 12 }, gap = 8, border = "bottom", border_color = "border" },
		[".sc-branch-name"] = { weight = "bold", size = "large" },
		[".sc-tabs"] = { padding = { 0, 6 }, gap = 0, border = "bottom", border_color = "border" },
		[".sc-tab"] = { padding = { 0, 8 }, gap = 6, color = "muted-foreground", border = "bottom", border_color = "background" },
		[".sc-tab:hover"] = { color = "foreground" },
		[".sc-tab:selected"] = { color = "foreground", border_color = "accent" },
		[".sc-tab-label"] = { size = "large" },
		[".sc-badge"] = { background = "face", radius = 999, padding = { 0, 6 }, size = "small", weight = "bold", color = "muted-foreground" },
		[".sc-compose"] = { padding = { 12, 12 }, gap = 8, border = "bottom", border_color = "border" },
		[".sc-message"] = { background = "well", border = true, border_color = "face", radius = 8, padding = { 8, 12 } },
		[".sc-commit"] = { background = "well", radius = 7, gap = 6, color = "muted-foreground" },
		[".sc-commit:hover"] = { background = "face", color = "foreground" },
		[".sc-commit-ready"] = { background = "accent", color = "#ffffff" },
		[".sc-commit-ready:hover"] = { background = "accent", color = "#ffffff" },
		[".sc-square"] = { background = "face", radius = 7, color = "foreground" },
		[".sc-square:hover"] = { background = "raised" },
		[".sc-result"] = { size = "small", padding = { 0, 2 } },
		[".sc-changes"] = { padding = { 0, 12 }, gap = 6 },
		[".sc-changes-label"] = { weight = "medium", color = "muted-foreground" },
		[".sc-tree"] = { padding = { 0, 6, 8, 6 } },
		[".sc-row"] = { padding = { 6, 12 }, gap = 2, border = "bottom", border_color = "border" },
		[".sc-row:hover"] = { background = "list-hover" },
		[".sc-row-title"] = { weight = "medium" },
		[".sc-row-meta"] = { size = "small", color = "muted-foreground" },
		[".sc-sha"] = { size = "small", font = "mono", color = "accent" },
	},

	init = function(previous: State?)
		local state: State = previous
			or { tab = "branch", message = "", folded = {}, changes_open = true, pending = nil, result = nil, made = 0, focus = false }
		state.pending, state.result, state.focus = nil, nil, false
		-- Back on PRs or Actions: ask `gh` again.
		if state.tab == "prs" or state.tab == "actions" then
			return state, { { to = "provider:projects", msg = { forge = true } } }
		end
		return state
	end,

	update = function(state: State, msg)
		if msg == "toggle-changes" then
			state.changes_open = not state.changes_open
		elseif msg == "commit" or (type(msg) == "table" and msg.msg == "commit") then
			if type(msg) == "table" then
				state.message = msg.value
			end
			if state.pending or not string.find(state.message, "%S") then
				return
			end
			state.made += 1
			state.pending = `c{state.made}`
			state.result = nil
			return state, { { to = "provider:projects", msg = { commit = state.message, id = state.pending } } }
		elseif msg == "pull" or msg == "push" then
			if state.pending then
				return
			end
			state.made += 1
			state.pending = `{msg}{state.made}`
			state.result = nil
			return state, { { to = "provider:projects", msg = { [msg] = true, id = state.pending } } }
		elseif msg == "refresh" then
			return state, { { to = "provider:projects", msg = { refresh = true, forge = state.tab == "prs" or state.tab == "actions" } } }
		elseif msg == "focus-message" then
			state.tab = "branch"
			state.focus = true
		elseif type(msg) == "table" then
			if msg.topic == "provider/projects" then
				local action = msg.value and msg.value.action
				if state.pending and action and action.id == state.pending then
					state.pending = nil
					state.result = { ok = action.ok, text = action.message or action.error or "" }
					if action.ok then
						state.message = ""
					end
				end
				return
			elseif msg.msg == "message" then
				state.message = msg.value
				state.focus = false
				return
			elseif msg.tab then
				state.tab = msg.tab
				state.focus = false
				local forge = msg.tab == "prs" or msg.tab == "actions"
				return state, { { to = "provider:projects", msg = { forge = forge } } }
			elseif msg.fold then
				state.folded[msg.fold] = if state.folded[msg.fold] then nil else true
			end
		end
	end,

	view = function(state: State, app)
		local projects = app.projects
		local current = projects and projects.current
		local summary
		for _, project in (projects and projects.projects) or {} do
			if current and project.path == current.path then
				summary = project
			end
		end

		local top = ui.row {
			class = "sc-top",
			drag = true,
			height = 32,
			shrink = false,
			align = "center",
			ui.spacer(),
			current and ui.row {
				class = "icon-button icon-button-small",
				width = 22,
				height = 22,
				align = "center",
				justify = "center",
				tooltip = `Copy {current.path}`,
				copy = current.path,
				ui.icon { name = "folder", size = "small" },
			},
			icon_button { icon = "git-merge", size = "small", tooltip = "Source control (⇧⌘G)", msg = "toggle-right", to = "shell", active = true },
			icon_button { icon = "settings", size = "small", tooltip = "Themes", command = "show-themes" },
		}

		local head = ui.row {
			class = "sc-head",
			height = 34,
			shrink = false,
			align = "center",
			ui.icon { name = "git-merge", size = "small" },
			ui.text { "Source Control", class = "sc-title" },
			ui.spacer(),
			icon_button { icon = "refresh-cw", size = "small", tooltip = "Look again", msg = "refresh" },
			icon_button { icon = "close", size = "small", tooltip = "Hide source control", msg = "toggle-right", to = "shell" },
		}

		if not current then
			return ui.column {
				grow = true,
				gap = "none",
				top,
				head,
				if projects
					then empty_state { icon = "git-branch", title = "Not a git work tree", message = "Pick a work tree of a project to see its changes." }
					else loading { block = true, text = "Looking for projects…" },
			}
		end

		local branch = ui.row {
			class = "sc-branch",
			height = 38,
			shrink = false,
			align = "center",
			ui.icon { name = "git-branch", tone = "muted" },
			ui.text { current.branch, class = "sc-branch-name", truncate = true },
			ui.spacer(),
			summary and ahead_behind { ahead = summary.ahead, behind = summary.behind, synced = true },
		}

		local forge = projects.forge
		local counts = { branch = current.count, prs = forge and not forge.error and #forge.prs or nil }
		local tab_row = {}
		for _, each in TABS do
			local count = counts[each.id]
			table.insert(tab_row, ui.row {
				class = "sc-tab",
				selected = state.tab == each.id,
				grow = true,
				justify = "center",
				align = "center",
				msg = { tab = each.id },
				ui.icon { name = each.icon :: any, size = "small" },
				ui.text { each.label, class = "sc-tab-label" },
				count and count > 0 and ui.text { tostring(count), class = "sc-badge" },
			})
		end
		local tabs = ui.row { class = "sc-tabs", height = 40, shrink = false, align = "stretch", table.unpack(tab_row) }

		local body
		if state.tab == "branch" then
			local ready = string.find(state.message, "%S") ~= nil and current.count > 0 and not state.pending
			body = ui.column {
				grow = true,
				gap = "none",
				ui.column {
					class = "sc-compose",
					shrink = false,
					ui.input {
						id = "commit-message",
						class = "sc-message",
						multiline = true,
						rows = 2,
						value = state.message,
						placeholder = `Commit message (⌘↵ to commit on {current.branch})`,
						change = "message",
						submit = "commit",
						focus = state.focus,
					},
					ui.row {
						height = 32,
						gap = "normal",
						ui.row {
							class = if ready then { "sc-commit", "sc-commit-ready" } else "sc-commit",
							grow = true,
							align = "center",
							justify = "center",
							msg = "commit",
							tooltip = if current.count == 0 then "Nothing to commit" else "Stage everything and commit",
							if state.pending
								then ui.icon { name = "loader-circle", spin = true, size = "small" }
								else ui.icon { name = "check", size = "small" },
							ui.text { if state.pending then "Working…" else "Commit" },
						},
						ui.row {
							class = "sc-square",
							width = 32,
							align = "center",
							justify = "center",
							tooltip = if summary then `{summary.behind} to pull` else "Pull",
							msg = "pull",
							ui.icon { name = "arrow-down", size = "small" },
						},
						ui.row {
							class = "sc-square",
							width = 32,
							align = "center",
							justify = "center",
							tooltip = if summary then `{summary.ahead} to push` else "Push",
							msg = "push",
							ui.icon { name = "arrow-up", size = "small" },
						},
					},
					state.result and ui.text {
						state.result.text,
						class = "sc-result",
						tone = if state.result.ok then "success" else "danger",
						wrap = true,
					},
				},
				ui.row {
					class = "sc-changes",
					height = 34,
					shrink = false,
					align = "center",
					msg = "toggle-changes",
					ui.icon { name = if state.changes_open then "chevron-down" else "chevron-right", size = "small", tone = "muted" },
					ui.text { "Changes", class = "sc-changes-label" },
					ui.text { tostring(current.count), class = "sc-badge" },
					ui.spacer(),
					ui.text { `+{current.added}`, class = "change-tree-added" },
					ui.text { `−{current.deleted}`, class = "change-tree-deleted" },
				},
				state.changes_open and ui.column {
					grow = true,
					scroll = true,
					gap = "none",
					if current.count == 0
						then ui.text { "Nothing changed", tone = "muted", style = { padding = { 4, 16 } } }
						else ui.column {
							class = "sc-tree",
							shrink = false,
							change_tree { files = current.files, folded = state.folded, counts = true, mark = "start", plain = true, indent = 12 },
							current.truncated and ui.text { `and {current.count - #current.files} more`, tone = "muted", size = "small" },
						},
				},
			}
		elseif state.tab == "history" then
			local rows = {}
			for _, commit in current.log or {} do
				table.insert(rows, ui.column {
					class = "sc-row",
					copy = commit.sha,
					tooltip = "Copy the commit's hash",
					ui.text { commit.subject, class = "sc-row-title", truncate = true },
					ui.row {
						align = "center",
						gap = "small",
						ui.text { commit.sha, class = "sc-sha" },
						ui.text { commit.author, class = "sc-row-meta", truncate = true },
						ui.spacer(),
						relative_time { seconds = math.max(0, (projects.now or 0) - commit.time), compact = true },
					},
				})
			end
			body = if #rows == 0
				then empty_state { icon = "history", title = "No commits yet" }
				else ui.column { grow = true, scroll = true, gap = "none", ui.column { shrink = false, gap = "none", table.unpack(rows) } }
		else
			local rows = {}
			if forge and forge.path == current.path and not forge.error then
				if state.tab == "prs" then
					for _, pr in forge.prs do
						table.insert(rows, ui.column {
							class = "sc-row",
							ui.row {
								align = "center",
								gap = "small",
								ui.icon {
									name = if pr.draft then "git-pull-request-draft" else "git-pull-request",
									size = "small",
									tone = if pr.draft then "muted" else "success",
								},
								ui.text { pr.title, class = "sc-row-title", truncate = true },
							},
							ui.row {
								align = "center",
								gap = "small",
								ui.text { `#{pr.number}`, class = "sc-sha" },
								ui.text { `{pr.branch} · {pr.author}`, class = "sc-row-meta", truncate = true },
								ui.spacer(),
								relative_time { seconds = math.max(0, (projects.now or 0) - pr.updated), compact = true },
							},
						})
					end
				else
					for _, run in forge.runs do
						local outcome = if run.status == "completed" then run.conclusion else "running"
						table.insert(rows, ui.column {
							class = "sc-row",
							ui.row {
								align = "center",
								gap = "small",
								if outcome == "running"
									then ui.icon { name = "loader-circle", spin = true, size = "small", tone = "warning" }
									else ui.icon {
										name = (RUN_ICONS[outcome] or "circle-dot") :: any,
										size = "small",
										tone = (RUN_TONES[outcome] or "muted") :: any,
									},
								ui.text { run.title, class = "sc-row-title", truncate = true },
							},
							ui.row {
								align = "center",
								gap = "small",
								ui.text { `{run.workflow} · {run.branch}`, class = "sc-row-meta", truncate = true },
								ui.spacer(),
								relative_time { seconds = math.max(0, (projects.now or 0) - run.created), compact = true },
							},
						})
					end
				end
			end
			if not forge or forge.path ~= current.path then
				body = loading { block = true, text = "Asking GitHub…", detail = "gh, in this work tree" }
			elseif forge.error then
				body = empty_state { icon = "github", title = "No GitHub answer", message = forge.error }
			elseif #rows == 0 then
				body = empty_state {
					icon = if state.tab == "prs" then "git-pull-request" else "play",
					title = if state.tab == "prs" then "No open pull requests" else "No workflow runs",
				}
			else
				body = ui.column { grow = true, scroll = true, gap = "none", ui.column { shrink = false, gap = "none", table.unpack(rows) } }
			end
		end

		return ui.column { grow = true, gap = "none", top, head, branch, tabs, body }
	end,
}
