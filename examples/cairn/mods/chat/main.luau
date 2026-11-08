-- The session: its header, transcript and composer. What you send goes to a
-- coding agent through the built-in `acp` provider (providers/acp), one run
-- per message, working in the session's folder: every run of a session
-- carries its id as `thread`, and a follow-up sends the earlier requests and
-- answers along, since each run starts the agent afresh. The transcript is
-- those runs as `app.acp` shares them, live: the tools the agent used, its
-- answer, and any permission question, which waits on the user here.

local chat_markdown = use "chat-markdown"
local dictation = use "dictation"
local dialog = use "dialog"
local dropdown = use "dropdown"
local empty_state = use "empty-state"
local icon_button = use "icon-button"
local mention_field = use "mention-field"
local menu = use "menu"

type State = {
	current: string?,
	list: { any },
	-- The chat's runs, newest first, from `provider/acp`; not kept.
	runs: { any },
	open_tools: { [string]: boolean },
	draft: string,
	caret: number,
	highlighted: number,
	no_suggestions: boolean,
	agent: string?,
	dropdown: string?,
	session_menu: boolean,
	renaming: boolean,
	new_title: string,
	attached: { string },
	listening: boolean,
	-- Why the last message could not start, for the session it was sent in.
	error: { session: string, text: string }?,
	-- A start sent and not yet answered.
	starting: boolean,
}

local COMMANDS = {
	{ id = "new", name = "new", detail = "Start a new session", icon = "square-pen", insert = "" },
}

local INSTRUCTIONS = [[
You are the coding agent of Cairn, a desktop app where the user talks with you about the folder you work in. Work in that folder. Keep to what they ask. Do not commit or push unless they ask. End with a short answer in Markdown: what you found or changed, and where.]]

local FOLLOW_UP = [[


This continues a conversation: `conversation` in the context has the user's earlier requests and the answers given, oldest first. Read the files as they are now rather than trusting the earlier answers.]]

local ATTACHED = [[


`attached_files` in the context are paths of files the user attached to this message.]]

local function going(run: any): boolean
	return run.status == "starting" or run.status == "running" or run.status == "waiting"
end

local function current_session(state: State): any
	for _, session: any in state.list do
		if session.id == state.current then
			return session
		end
	end
	return nil
end

-- The session's runs, oldest first.
local function session_runs(runs: { any }, id: string?): { any }
	local found = {}
	for index = #runs, 1, -1 do
		local run = runs[index]
		if id and (run.thread or run.id) == id then
			table.insert(found, run)
		end
	end
	return found
end

local function busy(state: State): any
	for _, run in session_runs(state.runs, state.current) do
		if going(run) then
			return run
		end
	end
	return nil
end

local function send(state: State, text: string): (State, { any })
	local words = string.match(text, "^%s*(.-)%s*$") or ""
	local session = current_session(state)
	if words == "" or not session or session.folder == "" or busy(state) or state.starting then
		return state, {}
	end
	local said = {}
	for _, run in session_runs(state.runs, session.id) do
		table.insert(said, { request = run.text, answer = run.message or run.error or run.status })
	end
	local instructions = INSTRUCTIONS
	local context: { [string]: any } = {}
	if #said > 0 then
		instructions ..= FOLLOW_UP
		context.conversation = said
	end
	if #state.attached > 0 then
		instructions ..= ATTACHED
		context.attached_files = state.attached
	end
	state.draft = ""
	state.caret = 0
	state.attached = {}
	state.error = nil
	state.starting = true
	return state, { { to = "provider:acp", msg = {
		start = words,
		id = "chat",
		thread = session.id,
		cwd = session.folder,
		agent = state.agent,
		instructions = instructions,
		context = context,
	} } }
end

-- The agents this machine can run, for the picker, and the chosen one's title.
local function agents(state: State, acp: any): ({ any }, string)
	local options = {}
	local chosen = state.agent or (acp and acp.agent) or "claude"
	local title = chosen
	for _, agent in (acp and acp.agents) or {} do
		if agent.installed or agent.fetched then
			table.insert(options, { id = agent.name, label = agent.title, detail = if agent.installed then nil else "via npx" })
		end
		if agent.name == chosen then
			title = agent.title
		end
	end
	return options, title
end

local TOOL_ICONS = {
	pending = "circle-dashed",
	in_progress = "loader-circle",
	completed = "circle-check",
	failed = "circle-x",
}

local function tools_row(state: State, run: any): any
	local count = run.tool_count or 0
	if count == 0 then
		return nil
	end
	local open = state.open_tools[run.id] == true
	local steps = {}
	if open then
		for _, tool in run.tools or {} do
			table.insert(steps, ui.row { gap = "small", align = "center",
				ui.icon { name = (TOOL_ICONS[tool.status] or "circle-dashed") :: any, tone = "muted", size = "small", spin = tool.status == "in_progress" },
				ui.text { tool.title, class = "tool-step", truncate = true },
			})
		end
		if count > #(run.tools or {}) then
			table.insert(steps, ui.text { `and {count - #run.tools} earlier`, class = "tool-step" })
		end
	end
	return ui.column { gap = "small",
		ui.row { class = "tools", align = "center", hug = true, msg = { tools = run.id },
			ui.text { "Used", tone = "muted" },
			ui.text { `{count} {if count == 1 then "tool" else "tools"}`, weight = "medium" },
			ui.icon { if open then "chevron-down" else "chevron-right", tone = "muted" },
		},
		table.unpack(steps),
	}
end

-- A permission question: the agent waits on one of its options. "Always"
-- is left out: the provider refuses it, as nothing allowed should outlive
-- the run.
local function question_card(run: any)
	local question = run.question
	local buttons = {}
	local refusal = false
	for _, option in question.options or {} do
		if option.kind ~= "allow_always" then
			refusal = refusal or option.kind == "reject_once" or option.kind == "reject_always"
			table.insert(buttons, ui.button {
				option.name,
				small = true,
				variant = if option.kind == "allow_once" then "primary" else "outline",
				msg = { answer = run.id, option = option.id },
			})
		end
	end
	if not refusal then
		table.insert(buttons, ui.button { "Deny", small = true, variant = "outline", msg = { answer = run.id, deny = true } })
	end
	return ui.column { class = "question",
		ui.row { gap = "small", align = "center",
			ui.icon { name = "circle-alert", class = "question-icon" },
			ui.text { `{run.agent_title} asks to: {question.title}`, weight = "medium", wrap = true },
		},
		ui.row { gap = "small", wrap = true, table.unpack(buttons) },
	}
end

local STATUS_LINE = {
	starting = "Starting the agent…",
	running = "Working…",
	waiting = "Waiting for your answer",
}

local function run_view(state: State, run: any)
	local answer: { any } = {}
	if run.question then
		table.insert(answer, question_card(run))
	end
	if going(run) and run.status ~= "waiting" then
		table.insert(answer, ui.row { gap = "small", align = "center",
			ui.icon { name = "loader-circle", tone = "muted", spin = true },
			ui.text { STATUS_LINE[run.status], tone = "muted" },
		})
	end
	if run.message then
		table.insert(answer, chat_markdown { text = run.message })
	end
	if run.error then
		table.insert(answer, ui.text { run.error, class = "run-error", wrap = true })
	elseif run.status == "cancelled" then
		table.insert(answer, ui.text { "Stopped.", tone = "muted" })
	elseif run.status == "stopped" then
		table.insert(answer, ui.text { "Stopped when the app quit.", tone = "muted" })
	end
	if run.warning then
		table.insert(answer, ui.text { run.warning, class = "run-warning", wrap = true })
	end
	return ui.column { class = "turn",
		ui.row { justify = "end",
			ui.column { class = "bubble", max_width = 430, gap = "small", ui.text(run.text) },
		},
		ui.column { class = "answer",
			tools_row(state, run),
			table.unpack(answer),
		},
	}
end

-- The files the session's folder changed, for `@`, when the projects
-- provider has that folder open.
local function changed_files(app: any, folder: string): { any }
	local current = app.projects and app.projects.current
	local people: { any } = {}
	if current and current.path == folder then
		for _, line: string in current.files or {} do
			local path = string.match(line, "^%a\t%d+\t%d+\t(.+)$")
			if path then
				table.insert(people, { name = path, icon = "file", insert = `@{path} ` })
			end
		end
	end
	return people
end

local function composer(state: State, session: any, app: any, options: { any }, title: string)
	local slash = string.sub(state.draft, 1, 1) == "/"
	local people: { any } = {}
	if state.no_suggestions then
		-- Dismissed: none until the next edit, so Enter sends the draft.
		people = {}
	elseif slash then
		people = COMMANDS
	else
		people = changed_files(app, session.folder)
	end
	local chips = {}
	for index, path in state.attached do
		table.insert(chips, ui.row { class = "chip", align = "center", msg = { detach = index }, tooltip = "Remove",
			ui.icon { "file", tone = "muted" }, ui.text { string.match(path, "[^/\\]+$") or path, size = "small" }, ui.icon { "close", tone = "muted" } })
	end
	local working = busy(state)
	local ready = string.match(state.draft, "%S") ~= nil and #options > 0 and not state.starting
	local button
	if working then
		button = ui.row { class = "send-on", width = 30, height = 30, align = "center", justify = "center", msg = "stop", tooltip = "Stop the agent",
			ui.icon "square" }
	else
		button = ui.row { class = if ready then "send-on" else "send", width = 30, height = 30, align = "center", justify = "center",
			msg = if ready then "send-click" else nil, tooltip = "Send", ui.icon "arrow-up" }
	end
	return ui.column { class = "composer-box", shrink = false, gap = "none",
		#chips > 0 and ui.row { gap = "small", wrap = true, table.unpack(chips) },
		ui.column { class = "composer-field",
			mention_field {
				id = "prompt", value = state.draft, caret = state.caret, grow = true, focus = true,
				trigger = if slash then "/" else "@",
				heading = if slash then "Commands" else "Changed files",
				people = people, highlighted = state.highlighted, width = 320,
				placeholder = `Ask {title} anything, / for commands, @ for changed files...`,
				change = "draft", submit = "send", dismiss = "no-suggestions",
			},
		},
		ui.row { align = "center", gap = "none",
			icon_button { icon = "plus", tooltip = "Attach a file", msg = "attach" },
			#options > 0 and dropdown {
				side = "top", options = options, selected = state.agent or (app.acp and app.acp.agent), value = title,
				event = "agent", width = 200, open = state.dropdown == "agent", toggle = { dropdown = "agent" }, dismiss = "close-dropdown",
			},
			ui.spacer(),
			dictation { id = "prompt", text = "dictated", listening = "listening" },
			button,
		},
	}
end

return mod {
	subscribe = { "cairn/sessions", "cairn/page", "provider/acp" },

	styles = {
		[".chat-head"] = { padding = { 10, 10, 10, 14 }, gap = 8 },
		[".crumb-title"] = { clip = true },
		[".crumb-fade"] = { gradient = "90deg, #17171700, #171717" },
		[".transcript"] = { padding = { 20, 26, 10, 26 }, gap = 26 },
		[".bubble"] = { background = "#2b2b2b", radius = 12, padding = { 9, 13 } },
		[".turn"] = { gap = 26 },
		[".answer"] = { gap = 14 },
		[".tools"] = { gap = 5, padding = { 2, 0 } },
		[".tool-step"] = { color = "muted-foreground", size = "small", font = "mono" },
		[".question"] = { gap = 10, padding = { 12, 14 }, radius = 10, border = true, border_color = "warning/50", background = "warning/8" },
		[".question-icon"] = { color = "warning" },
		[".run-error"] = { color = "danger" },
		[".run-warning"] = { color = "warning", size = "small" },
		[".composer-wrap"] = { padding = { 6, 12, 12, 12 }, gap = 8 },
		[".composer-box"] = { background = "#212121", radius = 12, border = true, border_color = "#2c2c2c", padding = { 12, 10, 8, 10 } },
		[".composer-field"] = { padding = { 3, 6, 14, 6 } },
		[".composer-error"] = { color = "danger", padding = { 0, 6 } },
		[".chip"] = { background = "#2c2c2c", radius = 6, padding = { 3, 7 }, gap = 5, margin = { 0, 0, 8, 6 } },
		[".send"] = { background = "#2c2c2c", color = "#6e6e6e", radius = 8 },
		[".send-on"] = { background = "foreground", color = "#111111", radius = 8 },
		[".dictation"] = { color = "muted-foreground" },
	},

	init = function(previous: any): State
		local state = previous or { open_tools = {}, draft = "", caret = 0, highlighted = 1, attached = {}, new_title = "" }
		state.list = {}
		state.runs = {}
		state.open_tools = state.open_tools or {}
		state.attached = state.attached or {}
		state.dropdown = nil
		state.session_menu = false
		state.renaming = false
		state.listening = false
		state.no_suggestions = false
		state.error = nil
		state.starting = false
		-- The sample chat's model and effort pickers are gone.
		state.model = nil
		state.effort = nil
		state.extra = nil
		return state
	end,

	update = function(state: State, msg)
		if msg == "send-click" then
			return send(state, state.draft)
		elseif msg == "stop" then
			local run = busy(state)
			if run then
				return state, { { to = "provider:acp", msg = { cancel = run.id } } }
			end
			return
		elseif msg == "new-session" then
			return state, { { to = "sidebar", msg = "new-session" } }
		elseif msg == "no-suggestions" then
			state.no_suggestions = true
		elseif msg == "close-dropdown" then
			state.dropdown = nil
		elseif msg == "session-menu" then
			state.session_menu = not state.session_menu
		elseif msg == "close-session-menu" then
			state.session_menu = false
		elseif msg == "close-rename" then
			state.renaming = false
		elseif msg == "save-rename" then
			local title = string.match(state.new_title, "^%s*(.-)%s*$") or ""
			state.renaming = false
			if title ~= "" then
				return state, { { to = "sidebar", msg = { rename = state.current, title = title } } }
			end
		elseif msg == "attach" then
			return state, { { to = "app:pick", msg = { kind = "open", multiple = true, reply = { attached = true } } } }
		elseif type(msg) ~= "table" then
			return
		elseif msg.topic == "cairn/sessions" and msg.value then
			state.list = msg.value.list
			state.current = msg.value.current
		elseif msg.topic == "provider/acp" then
			local runs = {}
			for _, run in (msg.value and msg.value.runs) or {} do
				if run.owner == "chat" then
					table.insert(runs, run)
				end
			end
			state.runs = runs
		elseif msg.topic == "cairn/page" then
			-- Home draws no mic button, so nothing else would stop it.
			if msg.value ~= "chat" and state.listening then
				state.listening = false
				return state, { { to = "provider:speech", msg = { cancel = true } } }
			end
		elseif msg.reply == "chat" and msg.action == "start" then
			state.starting = false
			if not msg.ok and state.current then
				state.error = { session = state.current, text = msg.error or "The agent could not start." }
			end
		elseif msg.reply == "chat" then
			-- A run finished: the transcript reads it from `app.acp`.
			return
		elseif msg.answer then
			local option: any = if msg.deny then false else msg.option
			return state, { { to = "provider:acp", msg = { answer = msg.answer, option = option } } }
		elseif msg.forget_session then
			-- The sidebar deleted it: stop what runs, forget what ran.
			local sends: { any } = {}
			for _, run in session_runs(state.runs, msg.forget_session) do
				table.insert(sends, { to = "provider:acp", msg = if going(run) then { cancel = run.id } else { forget = run.id } })
			end
			return state, sends
		elseif msg.msg == "listening" then
			state.listening = msg.value == true
		elseif msg.msg == "draft" then
			if msg.value ~= state.draft then
				state.highlighted = 1
				state.no_suggestions = false
			end
			state.draft = msg.value
			state.caret = msg.caret or #msg.value
		elseif msg.msg == "send" then
			return send(state, msg.value)
		elseif msg.msg == "dictated" then
			state.draft = if state.draft == "" then msg.value else `{state.draft} {msg.value}`
			state.caret = #state.draft
		elseif msg.msg == "rename-title" then
			state.new_title = msg.value
		elseif msg.mention == "up" or msg.mention == "down" then
			state.highlighted = math.max(1, state.highlighted + (if msg.mention == "up" then -1 else 1))
		elseif msg.mention == "choose" then
			local sends: { any } = { { to = "app:edit", msg = msg.edit } }
			if msg.item == "new" then
				table.insert(sends, { to = "sidebar", msg = "new-session" })
			end
			return state, sends
		elseif msg.tools then
			state.open_tools[msg.tools] = not state.open_tools[msg.tools]
		elseif msg.dropdown then
			state.dropdown = if state.dropdown == msg.dropdown then nil else msg.dropdown
		elseif msg.agent then
			state.agent = msg.agent
			state.dropdown = nil
		elseif msg.attached and msg.paths then
			for _, path in msg.paths do
				table.insert(state.attached, path)
			end
		elseif msg.detach then
			table.remove(state.attached, msg.detach)
		elseif msg.pick then
			state.session_menu = false
			if msg.pick == "rename" then
				local session = current_session(state)
				state.new_title = (session and session.title) or ""
				state.renaming = true
			elseif msg.pick == "unread" then
				return state, { { to = "sidebar", msg = { mark_unread = state.current } } }
			elseif msg.pick == "delete" then
				return state, { { to = "sidebar", msg = { delete = state.current } } }
			end
		end
	end,

	view = function(state: State, app)
		local session = current_session(state)
		if not session then
			return ui.column { grow = true, justify = "center", align = "center",
				empty_state { icon = "square-pen", title = "No session open", message = "Start one in a folder, and talk to a coding agent there.", action = "New session", msg = "new-session" },
			}
		end
		local acp = app.acp
		local options, title = agents(state, acp)

		local turns = {}
		for _, run in session_runs(state.runs, session.id) do
			table.insert(turns, run_view(state, run))
		end
		if #turns == 0 then
			local blank
			if acp == nil then
				blank = empty_state { icon = "loader-circle", title = "Starting…" }
			elseif #options == 0 then
				blank = empty_state {
					icon = "circle-alert",
					title = "No coding agent found",
					message = "Cairn talks to a coding agent over ACP. Install one (claude-agent-acp, codex-acp, gemini, opencode or copilot), or Node for npx, then open Cairn again.",
				}
			else
				blank = ui.column { align = "center", gap = "small",
					ui.text { "What should we work on?", size = "large", weight = "medium" },
					ui.text { `{title} works in {session.folder}, and asks you before it changes anything.`, tone = "muted", wrap = true },
				}
			end
			table.insert(turns, ui.column { grow = true, align = "center", justify = "center", height = 240, blank })
		end

		local error: string? = if state.error and state.error.session == session.id then state.error.text else nil

		return ui.column { grow = true, gap = "none",
			ui.row { class = "chat-head", align = "center", shrink = false,
				ui.row { class = "tile", width = 18, height = 18, shrink = false, align = "center", justify = "center",
					ui.text(string.upper(string.sub(session.name, 1, 1))) },
				ui.text { session.name, tone = "muted", tooltip = session.folder },
				ui.text { "/", tone = "muted" },
				ui.row { class = "crumb-title", grow = true, width = 0, align = "center",
					ui.text { session.title, wrap = false, weight = "medium" },
					ui.row { width = 26, float = "right", class = "crumb-fade" },
				},
				ui.popover {
					open = state.session_menu, dismiss = "close-session-menu", side = "bottom", align = "end", width = 190,
					content = menu { items = {
						{ id = "rename", label = "Rename…", icon = "pencil" },
						{ id = "unread", label = "Mark as unread", icon = "circle-dot" },
						"separator",
						{ id = "delete", label = "Delete session", icon = "trash-2", danger = true },
					} :: { any } },
					icon_button { icon = "ellipsis", tooltip = "Session", msg = "session-menu", size = "small" },
				},
			},
			-- The inner column keeps its height, or the scroll squeezes the turns.
			ui.column { grow = true, scroll = true, follow = true, gap = "none",
				ui.column { class = "transcript", shrink = false, table.unpack(turns) },
			},
			ui.column { class = "composer-wrap", shrink = false,
				error and ui.text { error, class = "composer-error", wrap = true },
				composer(state, session, app, options, title),
			},
			dialog {
				open = state.renaming, dismiss = "close-rename", title = "Rename session", size = "small", submit = "save-rename",
				buttons = { { label = "Cancel", msg = "close-rename", variant = "ghost" }, { label = "Rename", msg = "save-rename", variant = "primary" } },
				ui.input { id = "rename", class = "field", value = state.new_title, change = "rename-title", submit = "save-rename", focus = true },
			},
		}
	end,
}
