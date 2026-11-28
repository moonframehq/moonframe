-- A whole conversation with a coding agent (Claude, Codex, OpenCode, pi,
-- Gemini…) run through herdr-acp by the `chat` provider: the transcript
-- with folded tool runs and thoughts, the agent's permission questions,
-- the prompts queued behind the running turn, Stop and Resume, pickers for
-- the agent, mode, model and effort, and a composer that grows with its
-- lines, sends on Enter, recalls earlier prompts with Up, completes `/`
-- commands and takes pasted or dropped files.
--
-- It draws one chat of `app.chat` from the view the provider prepares
-- (moonframe-chat-view), so it parses nothing itself. Without `chat` it
-- starts one on the first prompt and tells the mod through `started`.
-- The drawing mod must be in providers/chat's `starters`.

type Props = {
	id: string?,
	chat: string?,
	agent: string?,
	cwd: string?,
	context: string?,
	placeholder: string?,
	suggestions: { any }?,
	title: string?,
	started: any,
	link: any,
	to: string?,
	view: any,
}

type State = {
	chat: string?,
	-- The agent picked for a new chat.
	agent: string?,
	draft: string,
	files: { string },
	sent: { string },
	-- A prompt waiting for the chat `start` makes.
	waiting: { prompt: string, files: { string } }?,
	folds: { [string]: boolean },
	output: any,
	open: string,
	shown: number,
	pick: number,
	seq: number,
	error: string?,
}

-- Rows drawn at first; "Show earlier" adds as many again.
local PAGE = 60
-- Prompts the composer recalls.
local HISTORY = 100

local AGENT_ICONS: { [string]: string } = {
	claude = "agent-claude",
	codex = "agent-codex",
	opencode = "agent-opencode",
	gemini = "agent-gemini",
	copilot = "agent-copilot",
	cursor = "agent-cursor",
	qwen = "agent-qwen",
	amp = "agent-amp",
}

local chat_markdown = use "chat-markdown"
local dropdown = use "dropdown"
local menu = use "menu"
local thinking = use "thinking"
local tool_group = use "tool-group"

local function agent_icon(agent: string?): any
	return AGENT_ICONS[string.lower(agent or "")] or "bot"
end

-- The chat this instance draws, from the stand-in or `app.chat`.
local function find(props: Props, state: State, app: any): any
	if props.view then
		return props.view
	end
	local id = props.chat or state.chat
	local service = app and app.chat
	if id == nil or service == nil then
		return nil
	end
	for _, chat in service.chats or {} do
		if chat.id == id then
			return chat
		end
	end
	return nil
end

local function chat_id(props: Props, state: State): string?
	return props.chat or state.chat or (props.view and props.view.id)
end

-- A message for the chat provider (or, in a demo, the mod).
local function tell(props: Props, msg: any): any
	return { to = props.to or "provider:chat", msg = msg }
end

-- The slash commands that complete what is typed, when it is `/word`.
local function completions(draft: string, view: any): { any }
	local typed = string.match(draft, "^/(%S*)$")
	if typed == nil or view == nil then
		return {}
	end
	local found: { any } = {}
	for _, command in view.commands or {} do
		if string.sub(command.name, 1, #typed) == typed then
			table.insert(found, command)
			if #found == 12 then
				break
			end
		end
	end
	return found
end

-- Sends the draft: to the chat, or a new chat first.
local function send_prompt(props: Props, state: State, text: string): { any }
	local prompt = string.gsub(text, "%s+$", "")
	if prompt == "" and #state.files == 0 then
		return {}
	end
	local sends: { any } = {}
	local files = state.files
	if #prompt > 0 and state.sent[#state.sent] ~= prompt then
		table.insert(state.sent, prompt)
		if #state.sent > HISTORY then
			table.remove(state.sent, 1)
		end
	end
	local id = chat_id(props, state)
	if id then
		table.insert(sends, tell(props, { focus = id }))
		table.insert(sends, tell(props, {
			prompt = prompt,
			chat = id,
			files = if #files > 0 then files else nil,
			context = props.context,
			id = "agent-chat",
		}))
	else
		state.waiting = { prompt = prompt, files = files }
		-- An empty agent is the provider's default.
		table.insert(sends, tell(props, {
			start = state.agent or props.agent or "",
			cwd = props.cwd,
			id = "agent-chat-start",
		}))
	end
	state.draft = ""
	state.files = {}
	state.open = ""
	state.error = nil
	state.seq += 1
	return sends
end

local function picker_options(picker: any): { any }
	local options: { any } = {}
	local group: string? = nil
	for _, choice in picker.choices or {} do
		if choice.group and choice.group ~= group then
			group = choice.group
			table.insert(options, { heading = choice.group })
		end
		table.insert(options, { id = choice.id, label = choice.name, detail = choice.description })
	end
	return options
end

-- A dropdown for the mode, model or effort; a toggle as a small button.
local function picker(name: string, value: any, state: State): any
	if value == nil then
		return nil
	end
	if value.kind == "toggle" then
		return ui.button {
			value.name,
			small = true,
			variant = if value.current == "true" then "outline" else "ghost",
			msg = { toggle_setting = value.id, on = value.current ~= "true" },
		}
	end
	return dropdown {
		value = value.current_name,
		selected = value.current,
		options = picker_options(value),
		open = state.open == name,
		toggle = { open = if state.open == name then "" else name },
		dismiss = { open = "" },
		event = "pick_" .. name,
		side = "top",
		width = 300,
	}
end

local function status_line(chat: any, view: any): any
	if chat.status == "failed" or chat.status == "ended" then
		return ui.row {
			class = "agent-chat-ended",
			align = "center",
			gap = "small",
			ui.icon { name = "circle-alert", size = "small" },
			ui.text { chat.error or "The chat ended.", size = "small", wrap = true },
			chat.session and ui.button { "Resume", icon = "rotate-ccw", small = true, msg = "resume" },
		}
	end
	if view == nil or view.status == "starting" or chat.status ~= "live" then
		local label = if chat.status == "reconnecting" then "Reconnecting…" else "Starting the agent…"
		return ui.row {
			class = "agent-chat-working",
			gap = "small",
			align = "center",
			ui.icon { name = "loader-circle", size = "small", spin = true },
			ui.text { label, size = "small" },
		}
	end
	if view.status == "working" then
		local running = #(view.activities or {})
		return ui.row {
			class = "agent-chat-working",
			gap = "small",
			align = "center",
			ui.icon { name = "loader-circle", size = "small", spin = true },
			ui.text { if #(view.permissions or {}) > 0 then "Waiting for you" else "Working…", size = "small" },
			running > 0 and ui.text { `· {running} running in the background`, size = "small", tone = "muted" },
		}
	end
	if view.status == "failed" or view.status == "exited" then
		return ui.row {
			class = "agent-chat-ended",
			gap = "small",
			ui.icon { name = "circle-alert", size = "small" },
			ui.text { view.message or "The agent stopped.", size = "small", wrap = true },
		}
	end
	return nil
end

local function user_row(row: any): any
	local chips: { any } = {}
	for _, name in row.files or {} do
		table.insert(chips, ui.row {
			class = "agent-chat-file",
			gap = "small",
			align = "center",
			hug = true,
			ui.icon { name = "paperclip", size = "small" },
			ui.text { name, size = "small" },
		})
	end
	return ui.column {
		key = row.key,
		self_align = "end",
		max_width = 560,
		gap = "small",
		ui.column {
			class = "agent-chat-user",
			ui.text { row.text, wrap = true },
		},
		(#chips > 0 or row.context) and ui.row {
			gap = "small",
			justify = "end",
			wrap = true,
			row.context and ui.icon {
				name = "layers",
				size = "small",
				class = "agent-chat-context",
				tooltip = "Sent with the app's context",
			},
			table.unpack(chips),
		},
	}
end

local function row_element(row: any, state: State, props: Props): any
	local kind = row.type
	if kind == "user" then
		return user_row(row)
	elseif kind == "answer" then
		return ui.column {
			key = row.key,
			chat_markdown { text = row.text, cursor = row.streaming, link = props.link and "link" },
		}
	elseif kind == "thought" then
		local open = state.folds[row.key] == true
		return ui.column {
			key = row.key,
			thinking {
				label = if row.streaming then "Thinking…" elseif row.gist ~= "" then row.gist else "Thought",
				open = open,
				text = row.text,
				msg = { fold = row.key },
			},
		}
	elseif kind == "tools" then
		local open = state.folds[row.key]
		if open == nil then
			open = row.live
		end
		return ui.column {
			key = row.key,
			tool_group {
				group = row,
				open = open,
				inspect = true,
				output = if state.output and state.output.chat == chat_id(props, state) then state.output else nil,
				msg = { fold = row.key, open = not open },
			},
		}
	elseif kind == "turn" then
		return ui.row {
			key = row.key,
			class = "agent-chat-turn",
			state = row["end"],
			gap = "small",
			align = "center",
			ui.icon { name = if row["end"] == "done" then "check" else "circle-alert", size = "small" },
			ui.text { row.text, size = "small" },
		}
	elseif kind == "notice" then
		return ui.row {
			key = row.key,
			class = "agent-chat-notice",
			state = row.level,
			gap = "small",
			ui.icon { name = if row.level == "info" then "info" else "triangle-alert", size = "small" },
			ui.text { row.text, size = "small", wrap = true },
		}
	elseif kind == "image" then
		return ui.row {
			key = row.key,
			class = "agent-chat-file",
			hug = true,
			self_align = if row.author == "user" then "end" else "start",
			gap = "small",
			ui.icon { name = "image", size = "small" },
			ui.text { "Picture", size = "small" },
		}
	end
	return nil
end

local function permission_card(permission: any): any
	local buttons: { any } = {}
	for _, option in permission.options or {} do
		table.insert(buttons, ui.button {
			option.name,
			small = true,
			variant = if option.kind == "allow_once" then "primary" elseif option.allows then "outline" else "ghost",
			msg = { answer = permission.id, option = option.id },
		})
	end
	return ui.column {
		class = "agent-chat-permission",
		gap = "small",
		ui.row {
			gap = "small",
			align = "center",
			ui.icon { name = "shield", size = "small", class = "agent-chat-permission-icon" },
			ui.text { permission.title, weight = "medium", wrap = true },
		},
		permission.tool and permission.tool ~= permission.title
			and ui.text { permission.tool, size = "small", class = "agent-chat-permission-tool", wrap = true },
		ui.row { gap = "small", wrap = true, table.unpack(buttons) },
	}
end

local function plan_strip(plan: any, state: State): any
	if plan == nil or plan.total == 0 then
		return nil
	end
	local open = state.folds["plan"] == true
	local items: { any } = {}
	if open then
		for _, item in plan.items do
			local icon = if item.status == "done" then "circle-check" elseif item.status == "running" then "circle-dot" else "circle"
			table.insert(items, ui.row {
				gap = "small",
				align = "center",
				ui.icon { name = icon :: any, size = "small", class = "agent-chat-plan-" .. item.status },
				ui.text { item.text, size = "small", strike = item.status == "done", wrap = true },
			})
		end
	end
	return ui.column {
		class = "agent-chat-plan",
		gap = "small",
		ui.row {
			gap = "small",
			align = "center",
			msg = { fold = "plan" },
			ui.icon { name = if open then "chevron-down" else "list-checks", size = "small" },
			ui.text { `Plan {plan.done}/{plan.total}`, size = "small", weight = "medium" },
			not open and plan.current and ui.text { plan.current, size = "small", tone = "muted", truncate = true },
		},
		table.unpack(items),
	}
end

local function queue_list(queue: { any }): any
	if #queue == 0 then
		return nil
	end
	local rows: { any } = {}
	for _, queued in queue do
		table.insert(rows, ui.row {
			class = "agent-chat-queued",
			gap = "small",
			align = "center",
			ui.icon { name = "clock", size = "small" },
			ui.text { queued.text, size = "small", truncate = true },
			ui.button { icon = "close", small = true, variant = "ghost", tooltip = "Take it back", msg = { unqueue = queued.id } },
		})
	end
	return ui.column { gap = "small", table.unpack(rows) }
end

local function empty_state(props: Props, service: any): any
	local chips: { any } = {}
	for _, suggestion in props.suggestions or {} do
		local label = if type(suggestion) == "table" then suggestion.label else suggestion
		local prompt = if type(suggestion) == "table" then (suggestion.prompt or suggestion.label) else suggestion
		table.insert(chips, ui.button { label, small = true, variant = "outline", msg = { suggest = prompt } })
	end
	local reason = service and service.available == false and service.reason
	return ui.column {
		grow = true,
		align = "center",
		justify = "center",
		gap = "large",
		padding = "large",
		ui.icon { name = agent_icon(props.agent), size = "large", class = "agent-chat-empty-icon" },
		ui.text { props.title or "Ask the agent", size = "large", weight = "semibold" },
		reason and ui.text { reason, tone = "muted", size = "small", wrap = true },
		#chips > 0 and ui.row { gap = "small", wrap = true, justify = "center", table.unpack(chips) },
	}
end

local function composer(props: Props, state: State, view: any, chat: any, service: any): any
	local found = completions(state.draft, view)
	local completing = #found > 0 and state.open ~= "no-complete"
	local working = view ~= nil and view.status == "working"
	local accepts = chat == nil or view == nil or view.accepts
	local keys: { [string]: any }? = nil
	if completing then
		keys = { up = "complete-up", down = "complete-down", tab = "complete", enter = "complete", escape = "complete-close" }
	elseif working then
		keys = { escape = "stop" }
	end
	local items: { any } = {}
	for _, command in found do
		table.insert(items, { id = command.name, label = "/" .. command.name, detail = command.hint or command.description })
	end
	local chips: { any } = {}
	for index, path in state.files do
		table.insert(chips, ui.row {
			class = "agent-chat-file",
			gap = "small",
			align = "center",
			hug = true,
			ui.icon { name = "paperclip", size = "small" },
			ui.text { string.match(path, "[^/\\]+$") or path, size = "small" },
			ui.button { icon = "close", small = true, variant = "ghost", msg = { unfile = index } },
		})
	end
	-- Before the first prompt, which agent it starts.
	local agents: { any } = {}
	local agent = state.agent or props.agent or (service and service.default) or "claude"
	if chat == nil and service and #(service.agents or {}) > 1 then
		for _, name in service.agents do
			table.insert(agents, { id = name, label = name, icon = agent_icon(name) })
		end
	end
	local field = ui.input {
		id = "prompt",
		value = state.draft,
		caret = "end",
		placeholder = props.placeholder or "Ask anything, / for commands",
		multiline = true,
		enter = "submit",
		rows = 1,
		max_rows = 10,
		history = state.sent,
		change = "draft",
		submit = "send",
		paste = "paste",
		keys = keys,
		focus = true,
		grow = true,
		disabled = not accepts,
		class = "agent-chat-field",
	}
	return ui.column {
		class = "agent-chat-composer",
		gap = "small",
		#chips > 0 and ui.row { gap = "small", wrap = true, table.unpack(chips) },
		ui.popover {
			open = completing,
			dismiss = "complete-close",
			side = "top",
			align = "start",
			width = 360,
			anchor_clicks = true,
			content = menu { items = items, highlighted = math.clamp(state.pick, 1, math.max(#items, 1)), event = "complete_with" },
			field,
		},
		ui.row {
			gap = "small",
			align = "center",
			#agents > 0 and dropdown {
				icon = agent_icon(agent),
				value = agent,
				selected = agent,
				options = agents,
				open = state.open == "agent",
				toggle = { open = if state.open == "agent" then "" else "agent" },
				dismiss = { open = "" },
				event = "pick_agent",
				side = "top",
			},
			view and picker("mode", view.mode, state),
			view and picker("model", view.model, state),
			view and picker("effort", view.effort, state),
			view and view.auto_accept and ui.button { "Auto-accept", icon = "zap", small = true, variant = "ghost", msg = { auto_accept = false }, tooltip = "Asks before acting again" },
			ui.row { grow = true },
			view and view.usage and ui.text {
				view.usage.text .. (if view.usage.cost then " · " .. view.usage.cost else ""),
				size = "small",
				tone = "muted",
				tooltip = `{view.usage.percent}% of the context window`,
			},
			working and ui.button { "Stop", icon = "square", small = true, variant = "outline", msg = "stop", tooltip = "Stop the turn (Esc)" },
			ui.button {
				icon = "arrow-up",
				small = true,
				variant = "primary",
				msg = "send-draft",
				disabled = not accepts,
				tooltip = if working then "Queue it behind this turn (Enter)" else "Send (Enter)",
			},
		},
	}
end

return component {
	title = "Agent Chat",
	category = "Data",
	icon = "messages-square",
	description = "A whole conversation with a coding agent through herdr-acp: transcript with folded tool runs, permission questions, queue, Stop and Resume, mode/model/effort pickers and a multiline composer with history, / commands and attachments.",
	when = "Give an app a chat with Claude, Codex, OpenCode, pi or Gemini on the `chat` provider (crates/moonframe-chat) in a few lines; the mod must be in providers/chat's `starters`. For one run of an agent the app starts itself use `agent-run`; for a static message use `chat-message`; for a send box alone use `composer`.",
	props = {
		["id"] = "the instance's id (one per chat drawn)",
		["chat"] = "the `app.chat` chat to draw; without it the first prompt starts one, and `started` says which",
		["agent"] = "the agent a new chat starts (default `app.chat.default`); the composer offers the others",
		["cwd"] = "the folder a new chat starts in (default the provider's)",
		["context"] = "text put before every prompt for the agent (what the user is looking at); never shown in the user's bubble",
		["placeholder"] = "the composer's grey text",
		["suggestions"] = "chips on the empty chat: strings, or { label, prompt }",
		["title"] = "the empty chat's heading (default \"Ask the agent\")",
		["started"] = "sent to the mod with `chat` set to the id of a chat it started",
		["link"] = "sent to the mod with `url` when a link in an answer is clicked",
		["to"] = "where its commands go (default \"provider:chat\"; \"parent\" sends them to the mod, as a demo does)",
		["view"] = "stands in for the `app.chat` chat { id, status, error?, session?, view }, for a preview or demo",
	},
	styles = {
		[".agent-chat"] = { gap = 0 },
		[".agent-chat-transcript"] = { padding = { 16, 20 }, gap = 14 },
		[".agent-chat-user"] = { background = "secondary", radius = 14, padding = { 8, 14 } },
		[".agent-chat-context"] = { color = "muted-foreground" },
		[".agent-chat-file"] = { background = "muted", radius = 6, padding = { 2, 8 } },
		[".agent-chat-turn"] = { color = "muted-foreground", padding = { 0, 2 } },
		[".agent-chat-turn:cancelled"] = { color = "warning" },
		[".agent-chat-turn:failed"] = { color = "danger" },
		[".agent-chat-notice"] = { color = "muted-foreground", radius = 8, padding = { 6, 10 }, background = "muted" },
		[".agent-chat-notice:warning"] = { color = "warning" },
		[".agent-chat-notice:error"] = { color = "danger" },
		[".agent-chat-working"] = { color = "muted-foreground", padding = { 0, 2 } },
		[".agent-chat-ended"] = { color = "danger", background = "danger/10", radius = 8, padding = { 6, 10 } },
		[".agent-chat-bottom"] = { padding = { 0, 16, 14, 16 }, gap = 8 },
		[".agent-chat-permission"] = { border = true, border_color = "warning/50", background = "warning/8", radius = 12, padding = { 10, 12 } },
		[".agent-chat-permission-icon"] = { color = "warning" },
		[".agent-chat-permission-tool"] = { font = "mono", color = "muted-foreground" },
		[".agent-chat-queued"] = { background = "muted", radius = 8, padding = { 2, 4, 2, 10 }, color = "muted-foreground" },
		[".agent-chat-plan"] = { border = true, border_color = "border", radius = 10, padding = { 6, 10 } },
		[".agent-chat-plan-done"] = { color = "success" },
		[".agent-chat-plan-running"] = { color = "accent" },
		[".agent-chat-composer"] = { background = "background", border = true, border_color = "border", radius = 14, padding = { 8, 10 }, shadow = "0 2px 8px #0000000d" },
		[".agent-chat-field"] = { background = "#00000000", border = "none", shadow = "none", padding = { 4, 2 } },
		[".agent-chat-empty-icon"] = { color = "accent" },
		[".agent-chat-error"] = { color = "danger" },
	},
	example = {
		id = "example",
		title = "Ask Claude about this repository",
		suggestions = { "What does this app do?", "Find the slowest test" },
		to = "parent",
	},
	usage = [[local agent_chat = use "agent-chat"

-- in view: a chat on the mod's state.chat, started by the first prompt
agent_chat {
	id = "assistant",
	chat = state.chat,
	agent = "claude",
	context = `The user is reading: {state.subject}`,
	placeholder = "Ask about this message…",
	suggestions = { "Summarize it", "Draft a reply" },
	started = "chat-started",
}

-- in update: keep the chat it started
if msg.msg == "chat-started" then
	state.chat = msg.chat
end
]],

	init = function(previous: State?, props: any)
		local state: State = previous or {
			draft = "",
			files = {},
			sent = {},
			folds = {},
			open = "",
			shown = PAGE,
			pick = 1,
			seq = 0,
		}
		local id = props.chat or state.chat
		if id and not props.view then
			return state, { tell(props, { focus = id }) }
		end
		return state
	end,

	update = function(state: State, msg: any, props: any)
		local view = props.view and props.view.view
		local id = chat_id(props, state)
		local sends: { any } = {}
		local function chat_msg(fields: any)
			if id then
				fields.chat = fields.chat or id
				table.insert(sends, tell(props, fields))
			end
		end
		local kind = if type(msg) == "table" then msg.msg else msg
		if kind == "draft" then
			state.draft = msg.value or ""
			state.pick = 1
			if state.open == "no-complete" then
				state.open = ""
			end
		elseif kind == "send" then
			sends = send_prompt(props, state, msg.value or state.draft)
		elseif kind == "send-draft" then
			sends = send_prompt(props, state, state.draft)
		elseif kind == "paste" then
			for _, path in msg.files or {} do
				if #state.files < 10 then
					table.insert(state.files, path)
				end
			end
		elseif type(msg) == "table" and msg.drop then
			for _, path in msg.paths or {} do
				if #state.files < 10 then
					table.insert(state.files, path)
				end
			end
		elseif type(msg) == "table" and msg.unfile then
			table.remove(state.files, msg.unfile)
		elseif type(msg) == "table" and msg.suggest then
			sends = send_prompt(props, state, msg.suggest)
		elseif type(msg) == "table" and msg.close_output then
			state.output = nil
		elseif type(msg) == "table" and msg.inspect then
			state.output = { chat = id, key = msg.inspect, page = msg.page }
			if msg.fetch then
				chat_msg({ tool_output = msg.inspect, page = msg.page, id = { output = msg.inspect, page = msg.page, chat = id } })
			end
		elseif type(msg) == "table" and type(msg.reply) == "table" and msg.reply.output then
			local output = state.output
			if output and output.chat == msg.reply.chat and output.key == msg.reply.output and output.page == msg.reply.page then
				if msg.ok then
					output.text = msg.text
					output.more = msg.more
				else
					state.error = msg.error
					state.output = nil
				end
			end
		elseif type(msg) == "table" and msg.fold then
			if msg.open ~= nil then
				state.folds[msg.fold] = msg.open
			else
				state.folds[msg.fold] = not state.folds[msg.fold]
			end
		elseif kind == "show-earlier" then
			state.shown += PAGE
		elseif type(msg) == "table" and msg.open then
			state.open = msg.open
		elseif kind == "complete-up" then
			state.pick = math.max(1, state.pick - 1)
		elseif kind == "complete-down" then
			state.pick += 1
		elseif kind == "complete-close" then
			state.open = "no-complete"
		elseif kind == "complete" or (type(msg) == "table" and msg.complete_with) then
			local name = type(msg) == "table" and msg.complete_with
			if not name then
				local found = completions(state.draft, view)
				local chosen = found[math.clamp(state.pick, 1, math.max(#found, 1))]
				name = chosen and chosen.name
			end
			if name then
				state.draft = "/" .. name .. " "
				state.seq += 1
			end
			state.open = "no-complete"
		elseif kind == "stop" then
			if id then
				table.insert(sends, tell(props, { cancel = id }))
			end
		elseif kind == "resume" then
			if id then
				table.insert(sends, tell(props, { resume = id }))
			end
		elseif type(msg) == "table" and msg.answer then
			if id then
				table.insert(sends, tell(props, { answer = id, permission = msg.answer, option = msg.option }))
			end
		elseif type(msg) == "table" and msg.unqueue then
			chat_msg({ unqueue = msg.unqueue })
		elseif type(msg) == "table" and msg.auto_accept ~= nil then
			chat_msg({ auto_accept = msg.auto_accept })
		elseif type(msg) == "table" and msg.pick_mode then
			chat_msg({ mode = msg.pick_mode })
			state.open = ""
		elseif type(msg) == "table" and (msg.pick_model or msg.pick_effort) then
			local which = if msg.pick_model then "model" else "effort"
			local current = view and view[which]
			-- The setting's own id, from the published view when there is one.
			local setting = (current and current.id) or which
			chat_msg({ setting = setting, value = msg.pick_model or msg.pick_effort })
			state.open = ""
		elseif type(msg) == "table" and msg.toggle_setting then
			chat_msg({ setting = msg.toggle_setting, value = msg.on })
		elseif type(msg) == "table" and msg.pick_agent then
			state.open = ""
			state.agent = msg.pick_agent
		elseif kind == "link" then
			if props.link ~= nil then
				local forward = if type(props.link) == "table" then table.clone(props.link) else { msg = props.link }
				forward.url = msg.url
				table.insert(sends, { to = "parent", msg = forward })
			end
		elseif type(msg) == "table" and msg.reply == "agent-chat-start" then
			if msg.ok and msg.chat then
				state.chat = msg.chat
				if props.started ~= nil then
					local told = if type(props.started) == "table" then table.clone(props.started) else { msg = props.started }
					told.chat = msg.chat
					table.insert(sends, { to = "parent", msg = told })
				end
				local waiting = state.waiting
				state.waiting = nil
				if waiting then
					table.insert(sends, tell(props, {
						prompt = waiting.prompt,
						chat = msg.chat,
						files = if #waiting.files > 0 then waiting.files else nil,
						context = props.context,
						id = "agent-chat",
					}))
				end
			else
				state.error = msg.error or "The chat could not start"
				state.waiting = nil
			end
		elseif type(msg) == "table" and msg.reply == "agent-chat" then
			state.error = if msg.ok then nil else msg.error
		else
			return nil
		end
		return state, sends
	end,

	build = function(props: Props, kept: any, app: any)
		local state: State = kept
		local service = app and app.chat
		local chat = find(props, state, app)
		local view = chat and chat.view
		local body: any
		local bottom: { any } = {}
		if chat == nil and state.waiting == nil then
			body = empty_state(props, service)
		else
			local children: { any } = {}
			local rows = (view and view.rows) or {}
			local first = math.max(1, #rows - state.shown + 1)
			local earlier = first - 1 + ((view and view.hidden) or 0)
			if first > 1 then
				table.insert(children, ui.row {
					justify = "center",
					ui.button { `Show earlier ({earlier})`, small = true, variant = "ghost", msg = "show-earlier" },
				})
			elseif earlier > 0 then
				table.insert(children, ui.row {
					justify = "center",
					ui.text { `{earlier} earlier rows are not kept`, size = "small", tone = "muted" },
				})
			end
			for index = first, #rows do
				local element = row_element(rows[index], state, props)
				if element then
					table.insert(children, element)
				end
			end
			if state.waiting then
				table.insert(children, user_row({ key = "waiting", text = state.waiting.prompt, files = {}, context = props.context ~= nil }))
			end
			local status = status_line(chat or { status = "starting" }, view)
			if status then
				table.insert(children, status)
			end
			table.insert(children, ui.column { key = "agent-chat-end", height = 1 })
			body = ui.column {
				class = "agent-chat-transcript",
				grow = true,
				scroll = true,
				follow = true,
				scroll_to = { key = "agent-chat-end", seq = state.seq },
				table.unpack(children),
			}
			if view then
				local plan = plan_strip(view.plan, state)
				if plan then
					table.insert(bottom, plan)
				end
				for _, permission in view.permissions or {} do
					table.insert(bottom, permission_card(permission))
				end
				local queue = queue_list(view.queue or {})
				if queue then
					table.insert(bottom, queue)
				end
			end
		end
		if state.error then
			table.insert(bottom, ui.text { state.error, size = "small", class = "agent-chat-error", wrap = true })
		end
		table.insert(bottom, composer(props, state, view, chat, service))
		return ui.column {
			class = "agent-chat",
			grow = true,
			drop = { drop = true },
			body,
			ui.column { class = "agent-chat-bottom", table.unpack(bottom) },
		}
	end,
}
