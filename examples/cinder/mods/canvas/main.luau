-- The canvas: a new session (Cinder on the import pill over an empty page,
-- the composer at the bottom), a CLI session's transcript, or a session
-- with a real coding agent.
--
-- Transcripts are your coding-agent CLI sessions, read-only, from the
-- history provider. A new session sends what you type to a coding agent
-- over the Agent Client Protocol (`provider:acp`, see providers/acp): in
-- the folder picked under the composer, with the reply, its tool calls and
-- its permission questions shown as they come. Reading is allowed once at
-- a time; every edit and command waits for your answer here. Stop stops it.
-- A session is its first run and the follow-ups typed under it, each a run
-- of its own told what was said before. The shell opens one as
-- "run:<first run's id>".

local sprite = use "cinder-sprite"
local menu = use "menu"
local reply = use "cinder-reply"

type State = {
	mode: string,
	open: string,
	draft: string,
	unfolded: { [string]: boolean },
	popup: string,
	-- The folder a new session starts in; "" for the newest repository.
	folder: string,
	-- The agent picked under the composer; nil for the provider's.
	agent: string?,
	-- Why the last start was refused.
	error: string?,
	-- What `update` needs of the providers, which only `view` is handed:
	-- the agents and our runs (without their replies) from `acp`, and the
	-- newest repository a CLI session ran in from `history`.
	acp: any?,
	newest: string,
}

local RUN = "run:"

-- What ACP calls a tool's kind, as a verb.
local TOOL_NAMES = {
	read = "Read",
	edit = "Edit",
	delete = "Delete",
	move = "Move",
	search = "Search",
	execute = "Run",
	think = "Think",
	fetch = "Fetch",
}

-- Told to each follow-up, whose agent starts afresh.
local FOLLOW_UP = "This continues a conversation: `conversation` in the context has the user's earlier requests and your answers, oldest first. Read the files as they are now rather than trusting the earlier answers."

local function going(run: any): boolean
	return run.status == "starting" or run.status == "running" or run.status == "waiting"
end

-- The runs of the session `thread`, oldest first.
local function session_runs(acp: any, thread: string): { any }
	local runs = {}
	local all = (acp and acp.runs) or {}
	for index = #all, 1, -1 do
		local run = all[index]
		if run.owner == "canvas" and (run.id == thread or run.thread == thread) then
			table.insert(runs, run)
		end
	end
	return runs
end

-- A run's parts in the reply component's shape; a run saved before parts
-- has only its last message.
local function run_parts(run: any): { any }
	local parts: { any } = {}
	for _, part in run.parts or {} do
		if part.kind == "tools" then
			local tools = {}
			for _, tool in part.tools or {} do
				table.insert(tools, { name = TOOL_NAMES[tool.kind] or "Tool", detail = tool.title, status = tool.status })
			end
			table.insert(parts, { kind = "tools", tools = tools })
		else
			table.insert(parts, { kind = "text", text = part.text })
		end
	end
	if #parts == 0 and run.message then
		table.insert(parts, { kind = "text", text = run.message })
	end
	return parts
end

-- The agent as it named itself, else as Cinder knows it.
local function agent_name(run: any): string
	local info = run.agent_info
	return (info and (info.title or info.name)) or run.agent_title or run.agent
end

-- Where a run worked, with the home folder as ~.
local function short(path: string?): string
	if not path or path == "" then
		return "?"
	end
	local home = string.match(path, "^(/Users/[^/]+)") or string.match(path, "^(/home/[^/]+)")
	return if home then "~" .. string.sub(path, #home + 1) else path
end

local function last_name(path: string): string
	return string.match(path, "([^/\\]+)[/\\]?$") or path
end

-- The agents that can run here, and the one a new session gets.
local function agents(state: State, acp: any): ({ any }, any?)
	local usable = {}
	local chosen = nil
	local wanted = state.agent or (acp and acp.agent) or "claude"
	for _, agent in (acp and acp.agents) or {} do
		if agent.installed or agent.fetched then
			table.insert(usable, agent)
			if agent.name == wanted then
				chosen = agent
			end
		end
	end
	return usable, chosen or usable[1]
end

-- The newest model `agent` said it uses.
local function model_of(acp: any, agent: string): string?
	for _, run in (acp and acp.runs) or {} do
		if run.agent == agent and run.model then
			return run.model
		end
	end
	return nil
end

-- The newest repository a CLI session ran in that is still there.
local function newest_folder(history: any): string
	if history and history.source == "cli" then
		for _, group in history.groups or {} do
			local here = false
			for _, session in group.sessions do
				here = here or not session.gone
			end
			if group.path ~= "" and here then
				return group.path
			end
		end
	end
	return ""
end

-- The folder a new session starts in: the one picked, else the newest.
local function start_folder(state: State, history: any): string
	return if state.folder ~= "" then state.folder else newest_folder(history)
end

-- What `update` keeps of `app.acp`: no replies, which it never reads.
local function slim(acp: any): any
	if acp == nil then
		return nil
	end
	local runs = {}
	for _, run in acp.runs or {} do
		if run.owner == "canvas" then
			table.insert(runs, {
				id = run.id,
				owner = run.owner,
				thread = run.thread,
				text = run.text,
				status = run.status,
				agent = run.agent,
				cwd = run.cwd,
				message = run.message,
				error = run.error,
			})
		end
	end
	return { agent = acp.agent, agents = acp.agents, runs = runs }
end

-- A run's turn: the user's words, the reply so far, then what it waits on.
local function render_run(state: State, run: any): { any }
	local items = {}
	table.insert(items, ui.row {
		justify = "end",
		ui.column { class = "em-bubble", max_width = 560, ui.text { run.text, class = "em-prose" } },
	})
	local parts = run_parts(run)
	if #parts > 0 then
		table.insert(items, reply { parts = parts, key = run.id, unfolded = state.unfolded })
	end
	if run.question then
		local buttons = {}
		for _, option in run.question.options do
			-- Allowing always is refused by the provider: once at most.
			if option.kind ~= "allow_always" then
				table.insert(buttons, ui.button {
					option.name,
					small = true,
					variant = if option.kind == "allow_once" then "primary" else "ghost",
					msg = { answer = run.id, option = option.id },
					to = "provider:acp",
				})
			end
		end
		table.insert(items, ui.column {
			class = "em-question",
			ui.row {
				align = "center",
				gap = "small",
				ui.icon { name = "circle-alert", class = "em-warn" },
				ui.text { `{agent_name(run)} asks to: {run.question.title}`, weight = "medium", wrap = true },
			},
			ui.row { gap = "small", wrap = true, table.unpack(buttons) },
		})
	elseif run.status == "starting" or run.status == "running" then
		table.insert(items, ui.row {
			class = "em-working",
			align = "center",
			ui.icon { name = "loader-circle", spin = true, size = "small" },
			ui.text { if run.status == "starting" then `Starting {run.agent_title}…` else "Working…", size = "small" },
		})
	elseif run.status == "failed" then
		table.insert(items, ui.row {
			class = "em-failed",
			align = "start",
			ui.icon { name = "circle-x", size = "small" },
			ui.row { grow = true, ui.text { run.error or "The agent stopped.", size = "small", wrap = true } },
		})
	elseif run.status == "cancelled" or run.status == "stopped" then
		table.insert(items, ui.text { if run.status == "cancelled" then "Stopped." else "Stopped when Cinder quit.", class = "em-hidden" })
	end
	if run.warning then
		table.insert(items, ui.text { run.warning, class = "em-hidden", wrap = true })
	end
	return items
end

-- A footer control: an icon or not, a label, a popover when clicked.
local function chip(icon: string?, label: string, msg: any, class: string?, tooltip: string?)
	return ui.row {
		class = `em-chip {class or ""}`,
		align = "center",
		msg = msg,
		tooltip = tooltip,
		icon and ui.icon { name = icon :: any },
		ui.text { label },
	}
end

local function centered(...: any)
	return ui.column { grow = true, align = "center", justify = "center", ... }
end

return mod {
	title = "Canvas",
	subscribe = { "cinder/nav", "provider/acp", "provider/history" },

	styles = {
		[".em-canvas"] = { padding = { 0, 24, 3, 24 } },
		[".em-import"] = {
			width = 320,
			height = 40,
			padding = { 0, 14 },
			gap = 12,
			radius = 12,
			border = true,
			border_color = "#272727",
			background = "background",
		},
		[".em-import:hover"] = { background = "#1f1f1f", border_color = "#333334" },
		[".em-pop-row"] = { padding = { 6, 10 }, radius = 8, gap = 10 },
		[".em-pop-row:hover"] = { background = "list-hover" },
		[".em-pop-head"] = { padding = { 6, 10, 4, 10 }, color = "muted-foreground", size = "small" },
		[".em-pop-detail"] = { color = "muted-foreground", size = "small" },
		[".em-composer"] = {
			height = 44,
			padding = { 0, 8, 0, 10 },
			gap = 12,
			radius = 14,
			background = "em-field",
			border = true,
			border_color = "em-field-line",
		},
		[".em-composer-icon"] = { color = "#d4d4d5", radius = 6, padding = 3 },
		[".em-composer-icon:hover"] = { background = "list-hover" },
		[".em-send"] = { background = "foreground", color = "background", radius = 8, padding = 5 },
		[".em-send-off"] = { background = "em-raised", color = "#8e8e8f", radius = 8, padding = 5 },
		[".em-foot"] = { height = 34, padding = { 0, 14 }, gap = 6 },
		[".em-chip"] = { padding = { 3, 6 }, radius = 6, gap = 6, color = "foreground" },
		[".em-chip:hover"] = { background = "list-hover" },
		[".em-chip-muted"] = { color = "muted-foreground" },
		[".em-thread"] = { padding = { 24, 0, 24, 0 }, gap = 20 },
		[".em-banner"] = { padding = { 8, 12 }, radius = 10, gap = 10, background = "#ffffff08", color = "muted-foreground", size = "small" },
		[".em-bubble"] = { background = "card", radius = 14, padding = { 9, 14 } },
		[".em-prose"] = { color = "foreground" },
		[".em-hidden"] = { color = "em-faint", size = "small" },
		[".em-question"] = { padding = { 10, 12 }, gap = 10, radius = 10, border = true, border_color = "#5a4a2a", background = "#f2a65a10" },
		[".em-warn"] = { color = "#f2a65a" },
		[".em-working"] = { gap = 8, color = "muted-foreground" },
		[".em-failed"] = { padding = { 8, 12 }, gap = 8, radius = 10, background = "#ff5f5714", color = "#ff8a80" },
		[".em-error"] = { color = "#ff8a80", size = "small", padding = { 0, 14, 6, 14 } },
		[".em-missing"] = { width = 460, padding = { 14, 16 }, gap = 10, radius = 12, border = true, border_color = "#333334" },
	},

	init = function(previous: State?): State
		return {
			mode = "code",
			open = "",
			draft = "",
			unfolded = {},
			popup = "",
			folder = (previous and previous.folder) or "",
			agent = previous and previous.agent,
			error = nil,
			acp = nil,
			newest = "",
		}
	end,

	update = function(state: State, msg)
		if msg == "close-popup" then
			state.popup = ""
			return
		elseif msg == "send" then
			msg = { msg = "ask", value = state.draft }
		end
		if type(msg) ~= "table" then
			return
		end
		if msg.topic == "provider/acp" then
			state.acp = slim(msg.value)
		elseif msg.topic == "provider/history" then
			state.newest = newest_folder(msg.value)
		elseif msg.topic == "cinder/nav" and msg.value then
			state.mode = msg.value.mode or "code"
			state.open = msg.value.open or ""
			state.popup = ""
			state.error = nil
		elseif msg.popup then
			state.popup = if state.popup == msg.popup then "" else msg.popup
		elseif msg.unfold then
			state.unfolded[msg.unfold] = not state.unfolded[msg.unfold]
		elseif msg.folder then
			state.folder = msg.folder
			state.popup = ""
		elseif msg.choose_folder then
			state.popup = ""
			return state, { { to = "app:pick", msg = { kind = "folder", prompt = "Start Here", reply = { picked_folder = true } } } }
		elseif msg.picked_folder then
			if msg.paths and msg.paths[1] then
				state.folder = msg.paths[1]
			end
		elseif msg.agent_pick then
			state.agent = msg.agent_pick
			state.popup = ""
		elseif msg.msg == "typing" then
			state.draft = msg.value or ""
		elseif msg.msg == "ask" and string.match(msg.value or "", "%S") then
			local text: string = msg.value
			local acp = state.acp
			local start: any
			if string.sub(state.open, 1, #RUN) == RUN then
				-- A follow-up: in the session's folder, with what was said.
				local thread = string.sub(state.open, #RUN + 1)
				local runs = session_runs(acp, thread)
				local first = runs[1]
				if not first then
					return
				end
				local said = {}
				for _, run in runs do
					if going(run) then
						return
					end
					table.insert(said, { request = run.text, answer = run.message or run.error or run.status })
				end
				start = { start = text, id = "cinder", thread = thread, agent = first.agent, cwd = first.cwd, instructions = FOLLOW_UP, context = { conversation = said } }
			elseif state.open == "" and state.mode == "code" then
				local folder = if state.folder ~= "" then state.folder else state.newest
				if folder == "" then
					state.error = "Choose a folder to start in, under the composer."
					return
				end
				local _, agent = agents(state, acp)
				start = { start = text, id = "cinder", agent = agent and agent.name, cwd = folder }
			else
				return
			end
			state.draft = ""
			state.error = nil
			return state, { { to = "provider:acp", msg = start } }
		elseif msg.reply == "cinder" and msg.action == "start" then
			if not msg.ok then
				state.error = msg.error
			elseif state.open == "" then
				-- The new session opens as itself, in the sidebar and in history.
				return state, { { to = "shell", msg = { open = RUN .. msg.run } } }
			end
		elseif msg.open then
			state.popup = ""
			return state, { { to = "shell", msg = { open = msg.open } } }
		end
	end,

	view = function(state: State, app)
		local history = app.history
		local acp = app.acp
		local chat = state.mode == "chat"
		local thread = if string.sub(state.open, 1, #RUN) == RUN then string.sub(state.open, #RUN + 1) else nil
		local reading = state.open ~= "" and thread == nil
		local usable, agent = agents(state, acp)
		local runs: { any } = if thread then session_runs(acp, thread) else {}
		local working = nil
		for _, run in runs do
			if going(run) then
				working = run
			end
		end

		local function thread_page(items: { any })
			return ui.column {
				grow = true,
				scroll = true,
				follow = true,
				gap = "none",
				ui.row {
					justify = "center",
					shrink = false,
					ui.column { class = "em-thread", grow = true, max_width = 760, table.unpack(items) },
				},
			}
		end

		-- What fills the page.
		local page
		if reading then
			local t = history and history.transcript
			if not t or t.id ~= state.open then
				page = centered(ui.row { gap = "normal", align = "center", ui.icon { name = "loader-circle", spin = true, tone = "muted" }, ui.text { "Reading the session…", tone = "muted" } })
			else
				local items = {}
				table.insert(items, ui.row {
					class = "em-banner",
					align = "center",
					ui.icon { name = "history", size = "small" },
					ui.text { `A CLI session in {t.project}{if t.branch ~= "" then " · " .. t.branch else ""}, read-only`, size = "small" },
				})
				if t.hidden > 0 then
					table.insert(items, ui.text { `{t.hidden} earlier turns are not shown.`, class = "em-hidden" })
				end
				for index, turn in t.turns do
					if turn.from == "user" then
						local text = if turn.parts[1] then turn.parts[1].text else ""
						table.insert(items, ui.row { justify = "end", ui.column { class = "em-bubble", max_width = 560, ui.text { text, class = "em-prose" } } })
					else
						table.insert(items, reply { parts = turn.parts, key = `{t.id}.{index}`, unfolded = state.unfolded })
					end
				end
				page = thread_page(items)
			end
		elseif thread then
			local first = runs[1]
			if acp == nil then
				page = centered(ui.icon { name = "loader-circle", spin = true, tone = "muted" })
			elseif not first then
				page = centered(ui.text { "This session is no longer kept.", tone = "muted" })
			else
				local newest = runs[#runs]
				local who = agent_name(newest)
				if newest.model then
					who ..= ` · {newest.model}`
				end
				local items = {}
				table.insert(items, ui.row {
					class = "em-banner",
					align = "center",
					ui.icon { name = "bot", size = "small" },
					ui.row { grow = true, ui.text { `{who} in {short(first.cwd)}`, size = "small", truncate = true } },
				})
				for _, run in runs do
					for _, item in render_run(state, run) do
						table.insert(items, item)
					end
				end
				page = thread_page(items)
			end
		elseif acp and #usable == 0 and not chat then
			-- No agent: what to install, as Pilot does for a missing pi.
			page = centered(ui.column {
				class = "em-missing",
				ui.row { gap = "normal", align = "center", ui.icon { name = "circle-alert", class = "em-warn" }, ui.text { "No coding agent is installed here", weight = "medium" } },
				ui.text {
					"Cinder runs a coding agent over the Agent Client Protocol. Install Node (Cinder then starts Claude's adapter with npx), or one of claude-agent-acp, codex-acp, gemini, opencode or copilot, then look again.",
					tone = "muted",
					size = "small",
					wrap = true,
				},
				ui.row { ui.button { "Look again", small = true, msg = { refresh = true }, to = "provider:acp" } },
			})
		else
			-- The new session: Cinder sitting on the import pill.
			local recent = {}
			local count = 0
			if history then
				count = history.total or 0
				table.insert(recent, ui.text { "Recent CLI sessions", class = "em-pop-head" })
				local shown = 0
				for _, group in history.groups or {} do
					for _, session in group.sessions do
						if shown < 10 then
							shown += 1
							table.insert(recent, ui.row {
								class = "em-pop-row",
								align = "center",
								msg = { open = session.id },
								ui.icon { name = "terminal", size = "small", tone = "muted" },
								ui.row { grow = true, ui.text { session.title, truncate = true } },
								ui.text { `{group.name} · {session.age}`, class = "em-pop-detail" },
							})
						end
					end
				end
			end
			local label = if chat
				then "Chats are not in Cinder yet"
				elseif count == 0 then "No CLI sessions yet"
				else `Import {count} CLI session{if count == 1 then "" else "s"}`
			page = centered(ui.popover {
				open = state.popup == "import" and not chat and count > 0,
				dismiss = "close-popup",
				side = "bottom",
				width = 460,
				content = ui.column { gap = "none", table.unpack(recent) },
				ui.row {
					class = "em-import",
					align = "center",
					msg = if chat or count == 0 then nil else { popup = "import" },
					ui.icon { name = if chat then "message-square" else "terminal", size = "small" },
					ui.text { label },
					ui.row { float = { top = -30, right = 10 }, sprite { tooltip = "Cinder" } },
				},
			})
		end

		-- The folders to start in: the repositories sessions ran in, or any.
		local folder = start_folder(state, history)
		local folders: { any } = { { heading = "Start in" } }
		local seen = {}
		if history and history.source == "cli" then
			for _, group in history.groups or {} do
				if group.path ~= "" and not seen[group.path] and #folders < 12 then
					seen[group.path] = true
					table.insert(folders, { id = group.path, label = group.name, icon = "folder", detail = short(group.path), msg = { folder = group.path } })
				end
			end
		end
		if state.folder ~= "" and not seen[state.folder] then
			table.insert(folders, { id = state.folder, label = last_name(state.folder), icon = "folder", detail = short(state.folder), msg = { folder = state.folder } })
		end
		table.insert(folders, "separator")
		table.insert(folders, { id = "choose", label = "Choose a folder…", icon = "folder-open", msg = { choose_folder = true } })

		-- The agents to pick from, with the model each last said it uses.
		local agent_items: { any } = { { heading = "Agent" } }
		for _, each in usable do
			table.insert(agent_items, {
				id = each.name,
				label = each.title,
				icon = if agent and each.name == agent.name then "check" else "bot",
				detail = model_of(acp, each.name) or (if each.installed then nil else "via npx"),
				msg = { agent_pick = each.name },
			})
		end
		local agent_label = "No agent"
		if thread and runs[#runs] then
			agent_label = agent_name(runs[#runs]) .. (if runs[#runs].model then ` · {runs[#runs].model}` else "")
		elseif agent then
			local model = model_of(acp, agent.name)
			agent_label = agent.title .. (if model then ` · {model}` else "")
		end

		local disabled = reading or chat or working ~= nil or (thread == nil and #usable == 0) or acp == nil
		local placeholder = if reading
			then "This CLI session is read-only. Start a new session to work with an agent."
			elseif chat then "Switch to code mode (⌘1) to work with an agent in a folder"
			elseif working then `{agent_name(working)} is working…`
			elseif thread then "Reply…"
			else "Describe a task or ask a question"

		local action = if working
			then ui.row { class = "em-send", tooltip = "Stop", msg = { cancel = working.id }, to = "provider:acp", ui.icon { name = "square", size = "small" } }
			else ui.row {
				class = if disabled or not string.match(state.draft, "%S") then "em-send-off" else "em-send",
				tooltip = "Send",
				msg = if disabled then nil else "send",
				ui.icon { name = "arrow-up", size = "small" },
			}

		local where = if thread and runs[1]
			then chip("folder", last_name(runs[1].cwd or ""), nil, nil, runs[1].cwd)
			else ui.popover {
				open = state.popup == "folder",
				dismiss = "close-popup",
				side = "top",
				width = 360,
				content = menu { items = folders },
				chip("folder", if folder ~= "" then last_name(folder) else "Choose a folder", { popup = "folder" }, nil, if folder ~= "" then folder else nil),
			}

		return ui.column {
			class = "em-canvas",
			grow = true,
			gap = "none",
			page,
			state.error and ui.text { state.error, class = "em-error", wrap = true },
			ui.row {
				justify = "center",
				shrink = false,
				ui.column {
					grow = true,
					max_width = 768,
					gap = "none",
					ui.row {
						class = "em-composer",
						align = "center",
						ui.input {
							id = "prompt",
							class = "em-input",
							grow = true,
							value = state.draft,
							placeholder = placeholder,
							disabled = disabled,
							change = "typing",
							submit = "ask",
						},
						action,
					},
					ui.row {
						class = "em-foot",
						align = "center",
						chip("laptop", "This computer", nil),
						where,
						ui.spacer(),
						if thread
							then chip(nil, agent_label, nil, "em-chip-muted")
							else ui.popover {
								open = state.popup == "agent",
								dismiss = "close-popup",
								side = "top",
								align = "end",
								width = 260,
								content = menu { items = agent_items },
								chip(nil, agent_label, if #usable > 0 then { popup = "agent" } else nil, "em-chip-muted"),
							},
						chip("shield", "Asks before edits", nil, "em-chip-muted", "The agent reads on its own; every edit and command waits for your answer here (providers/acp's allow)."),
					},
				},
			},
		}
	end,
}
