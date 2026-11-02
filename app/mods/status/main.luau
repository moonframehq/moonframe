-- The status bar along the bottom. The branch opens a sheet to commit and
-- push, which the git provider (providers/git, built into the app) does and
-- answers here. Each asks for the repository the sheet shows (`root`): the
-- provider follows the terminal's project, and refuses one that moved.
type State = {
	open: boolean,
	message: string,
	-- The action waiting for the provider's answer, if any.
	running: string?,
	-- The last answer: { ok, text }.
	result: { ok: boolean, text: string }?,
}

return mod {
	region = "status-right",

	init = function(previous: any): State
		local state = previous or {}
		return {
			open = false,
			message = state.message or "",
			running = nil,
			result = nil,
		}
	end,

	update = function(state: State, msg)
		if msg == "toggle" then
			state.open = not state.open
			state.result = nil
		elseif msg == "close" then
			state.open = false
		elseif type(msg) == "table" and msg.msg == "message" then
			state.message = msg.value
		elseif type(msg) == "table" and msg.commit == true then
			if state.running or state.message:match("^%s*$") then
				return nil
			end
			state.running = "commit"
			state.result = nil
			return state,
				{ { to = "provider:git", msg = { commit = state.message, root = msg.root, id = "status" } } }
		elseif type(msg) == "table" and msg.push == true then
			if state.running then
				return nil
			end
			state.running = "push"
			state.result = nil
			return state, { { to = "provider:git", msg = { push = true, root = msg.root, id = "status" } } }
		elseif type(msg) == "table" and msg.reply == "status" then
			-- The git provider's answer to a commit or push from here.
			state.running = nil
			state.result = { ok = msg.ok, text = msg.message or msg.error or "" }
			if msg.ok and msg.action == "commit" then
				state.message = ""
			end
		end
		return nil
	end,

	view = function(state: State, app)
		local git = app.git
		local branch: any = false
		if git and git.repository then
			local extra = ""
			if git.changed > 0 then
				extra ..= ` +{git.changed}`
			end
			if git.ahead > 0 or git.behind > 0 then
				extra ..= ` ↑{git.ahead} ↓{git.behind}`
			end
			local result: any = false
			if state.running then
				result = ui.row {
					gap = "small",
					align = "center",
					ui.icon { name = "loader-circle", spin = true },
					ui.text { state.running == "push" and "Pushing…" or "Committing…", size = "small", tone = "muted" },
				}
			elseif state.result then
				result = ui.text {
					state.result.text,
					size = "small",
					tone = (if state.result.ok then "success" else "danger") :: any,
				}
			end
			local nothing = git.changed == 0
			branch = ui.popover {
				open = state.open,
				dismiss = "close",
				side = "top",
				align = "end",
				width = 340,
				content = ui.column {
					gap = "small",
					padding = "normal",
					ui.text { `⎇ {git.branch}`, weight = "medium" },
					ui.text {
						if nothing
							then "No changes to commit"
							else `{git.changed} changed · +{git.added} −{git.deleted}`,
						size = "small",
						tone = "muted",
					},
					ui.input {
						id = "commit-message",
						value = state.message,
						placeholder = "Commit message",
						multiline = true,
						rows = 3,
						change = "message",
						submit = { commit = true, root = git.root },
						focus = state.open,
					},
					ui.row {
						gap = "small",
						ui.button {
							"Commit",
							icon = "git-commit-horizontal",
							small = true,
							disabled = nothing or state.running ~= nil or state.message:match("^%s*$") ~= nil,
							msg = { commit = true, root = git.root },
						},
						ui.button {
							"Push",
							icon = "upload",
							small = true,
							variant = "outline",
							disabled = state.running ~= nil,
							msg = { push = true, root = git.root },
						},
						ui.spacer(),
						ui.button { icon = "refresh-cw", small = true, variant = "ghost", msg = "refresh", to = "provider:git" },
					},
					result,
				},
				ui.row {
					class = "usage-widget",
					align = "center",
					msg = "toggle",
					ui.text { `⎇ {git.branch}{extra}`, class = "status-text" },
				},
			}
		end
		return ui.row { gap = "large", align = "center", branch }
	end,
}
