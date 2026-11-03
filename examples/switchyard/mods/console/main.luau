-- The console under the chats: the worktree's setup and run scripts (one
-- command each, run through the `shell` provider, their last run shown as a
-- card) and a terminal in the worktree. Each worktree remembers its own
-- scripts and keeps its own shell.

local segmented = use "segmented"
local shell_card = use "shell-card"
local empty_state = use "empty-state"

type State = {
	tab: string,
	workspace: { path: string, name: string, branch: string }?,
	scripts: { [string]: { setup: string?, run: string? } },
	runs: { [string]: { setup: string?, run: string? } },
	editing: string?,
	text: string,
	pending: { [string]: { path: string, which: string } },
	serial: number,
}

local TITLES = { setup = "Setup", run = "Run" }

-- A terminal id for a worktree: the same folder always gets the same
-- shell (Moonframe keeps up to 8 a mod).
local function terminal_id(path: string): string
	local hash = 5381
	for at = 1, #path do
		hash = (hash * 33 + string.byte(path, at)) % 2147483647
	end
	return `t{hash}`
end

return mod {
	title = "Console",
	subscribe = { "switchyard/workspace" },

	styles = {
		[".console-top"] = { padding = { 6, 10 }, gap = 8 },
		[".console-body"] = { padding = { 4, 14, 12, 14 }, gap = 10 },
		[".console-script"] = { background = "secondary", radius = 10, padding = { 8, 12 }, gap = 10, border = true, border_color = "border" },
		[".console-command"] = { font = "mono" },
		[".console-terminal"] = { padding = { 0, 6, 6, 10 } },
	},

	init = function(previous: State?): State
		local state: State = previous or {
			tab = "terminal", workspace = nil, scripts = {}, runs = {}, editing = nil, text = "",
			pending = {}, serial = 0,
		}
		state.pending = {}
		state.editing = nil
		return state
	end,

	update = function(state: State, msg)
		local workspace = state.workspace
		if type(msg) ~= "table" then
			return
		end
		if msg.topic == "switchyard/workspace" then
			-- Its branch changing keeps the folder: an edit stays open.
			local moved = not workspace or not msg.value or workspace.path ~= msg.value.path
			state.workspace = msg.value
			if moved then
				state.editing = nil
			end
		elseif msg.select then
			state.tab = msg.select
			state.editing = nil
		elseif msg.show then
			state.tab = msg.show
			return state, { { to = "shell", msg = "show-bottom" } }
		elseif msg.edit and workspace then
			state.editing = msg.edit
			state.text = (state.scripts[workspace.path] or {})[msg.edit] or ""
		elseif msg.msg == "text" then
			state.text = msg.value
		elseif msg.msg == "save" and workspace and state.editing then
			local scripts = state.scripts[workspace.path] or {}
			local text = string.match(state.text, "^%s*(.-)%s*$") or ""
			scripts[state.editing] = if text == "" then nil else text
			state.scripts[workspace.path] = scripts
			state.editing = nil
		elseif msg.msg == "cancel" then
			state.editing = nil
		elseif msg.start and workspace then
			local command = (state.scripts[workspace.path] or {})[msg.start]
			if command then
				state.serial += 1
				local token = `k{state.serial}`
				state.pending[token] = { path = workspace.path, which = msg.start }
				return state, { { to = "provider:shell", msg = { run = command, cwd = workspace.path, id = token } } }
			end
		elseif msg.stop then
			return state, { { to = "provider:shell", msg = { stop = msg.stop } } }
		elseif msg.reply and msg.action == "run" then
			local pending = state.pending[msg.reply]
			state.pending[msg.reply] = nil
			if pending and msg.ok then
				local runs = state.runs[pending.path] or {}
				runs[pending.which] = msg.run
				state.runs[pending.path] = runs
			end
		end
	end,

	view = function(state: State, app)
		local workspace = state.workspace
		local by_id = {}
		for _, run in (app.shell and app.shell.runs) or {} do
			by_id[run.id] = run
		end

		local body
		if not workspace then
			body = ui.text { "No worktree yet", tone = "muted" }
		elseif state.tab == "terminal" then
			body = ui.column { grow = true, class = "console-terminal",
				ui.terminal { id = terminal_id(workspace.path), cwd = workspace.path },
			}
		else
			local which = state.tab
			local command = (state.scripts[workspace.path] or {})[which]
			local run_id = (state.runs[workspace.path] or {})[which]
			local run = run_id and by_id[run_id]
			if state.editing == which then
				body = ui.column { class = "console-body", grow = true,
					ui.text { `The {string.lower(TITLES[which])} command for {workspace.branch}, run in its folder:`, tone = "muted" },
					ui.input { id = `script-{which}`, value = state.text, change = "text", submit = "save", mono = true, focus = true,
						placeholder = if which == "setup" then "npm install" else "just run", keys = { escape = "cancel" } },
					ui.row { gap = "small", ui.button { "Save", variant = "primary", small = true, msg = { msg = "save" } },
						ui.button { "Cancel", variant = "ghost", small = true, msg = { msg = "cancel" } } },
				}
			elseif not command then
				body = ui.column { grow = true, align = "center", justify = "center",
					empty_state {
						icon = if which == "setup" then "settings" else "play",
						title = `No {string.lower(TITLES[which])} script configured`,
						message = `Give {workspace.branch} a command to {if which == "setup" then "prepare it" else "run it"}.`,
						action = `Configure {string.lower(TITLES[which])} script`,
						msg = { edit = which },
					},
				}
			else
				local running = run and run.status == "running"
				body = ui.column { class = "console-body", grow = true, scroll = true,
					ui.row { class = "console-script", align = "center",
						ui.text { command, class = "console-command", truncate = true },
						ui.spacer(),
						ui.button { "Edit", variant = "ghost", small = true, msg = { edit = which } },
						if running
							then ui.button { "Stop", small = true, msg = { stop = run.id } }
							else ui.button { TITLES[which], icon = "play", variant = "primary", small = true, msg = { start = which } },
					},
					run and shell_card { run = run, title = TITLES[which], stop = { stop = run.id }, rerun = { start = which } },
				}
			end
		end

		return ui.column { grow = true, gap = "none",
			ui.row { class = "console-top", align = "center", shrink = false,
				segmented {
					small = true,
					selected = state.tab,
					options = { { id = "setup", label = "Setup" }, { id = "run", label = "Run" }, { id = "terminal", label = "Terminal" } },
				},
				ui.spacer(),
				workspace and ui.text { workspace.path, tone = "muted", size = "small", truncate = true },
			},
			body,
		}
	end,
}
