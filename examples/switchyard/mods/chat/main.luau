-- The chats of the workspace: a tab per agent chat (from the `chat`
-- provider, herdr-acp underneath), the chosen one's thread, and the
-- composer. A message starting with `!` is a shell command instead: it runs
-- in the workspace through the `shell` provider, and its card joins the
-- thread where it ran; Add to chat sends its output with the next message.
--
-- What the provider says each chat is doing arrives on `provider/chat`;
-- the mod keeps a little of it (`known`) so `update`, which cannot read
-- `app`, can tell a running turn from an idle one.

local chat_entry = use "chat-entry"
local shell_card = use "shell-card"
local composer = use "agent-composer"
local dropdown = use "dropdown"
local icon_button = use "icon-button"
local loading = use "loading"

local AGENT_ICONS = {
	claude = "agent-claude", codex = "agent-codex", gemini = "agent-gemini", copilot = "agent-copilot",
	opencode = "agent-opencode", cursor = "agent-cursor", amp = "agent-amp", qwen = "agent-qwen",
	grok = "agent-grok", droid = "agent-droid", kiro = "agent-kiro",
}
local AGENT_TITLES = {
	claude = "Claude", codex = "Codex", gemini = "Gemini", copilot = "Copilot", opencode = "OpenCode", pi = "Pi",
}
-- Settings in the order the composer shows them.
local SETTING_ORDER = { model = 1, model_config = 2, thought_level = 3, mode = 4, other = 5 }

type Anchor = { run: string, after: number }
type Known = { status: string, working: boolean, cwd: string, after: number? }

type State = {
	workspace: { path: string, name: string, branch: string }?,
	selected: { [string]: string? },
	draft: string,
	timeline: { [string]: { Anchor } },
	context: { [string]: { { id: string, label: string, text: string } } },
	waiting: { [string]: { string } },
	pending: { [string]: any },
	known: { [string]: Known },
	open: { [string]: boolean? },
	menu: string?,
	highlight: number,
	error: string?,
	serial: number,
	completion: { path: string, folder: string }?,
}

local function agent_icon(agent: string?): any
	return AGENT_ICONS[agent or ""] or "bot"
end

local function agent_title(agent: string?): string
	return AGENT_TITLES[agent or ""] or agent or "Agent"
end

-- The chat shown for the workspace, if any.
local function current_chat(state: State): string?
	local workspace = state.workspace
	local id = workspace and state.selected[workspace.path]
	if id and state.known[id] then
		return id
	end
	return nil
end

-- Where a shell command's card goes: the chat shown, else the workspace.
local function timeline_key(state: State): string
	return current_chat(state) or `ws:{(state.workspace and state.workspace.path) or ""}`
end

local function anchor(state: State): number
	local id = current_chat(state)
	return if id then state.known[id].after or 0 else 0
end

-- The chats of the workspace, oldest first, from what the mod knows.
local function tabs(state: State): { string }
	local ids = {}
	local path = state.workspace and state.workspace.path
	for id, known in state.known do
		if known.cwd == path then
			table.insert(ids, id)
		end
	end
	table.sort(ids)
	return ids
end

local function last_entry(app_chat: any): number
	local transcript = app_chat and app_chat.transcript
	local entries = transcript and transcript.entries
	if entries and #entries > 0 then
		return entries[#entries].id
	end
	return 0
end

-- The prompt with the outputs the user added, each fenced and named.
local function with_context(text: string, items: { any }): string
	if #items == 0 then
		return text
	end
	local parts = { text }
	for _, item in items do
		table.insert(parts, `Output of \`{item.label}\` in the workspace:\n\`\`\`\n{item.text}\n\`\`\``)
	end
	return table.concat(parts, "\n\n")
end

local function next_id(state: State): string
	state.serial += 1
	return `c{state.serial}`
end

local function focus(state: State): { any }
	local id = current_chat(state)
	return if id then { { to = "provider:chat", msg = { focus = id } } } else {}
end

-- Keep unsent text even when another draft was typed before a rejection.
local function restore_prompt(state: State, prompt: string)
	state.draft = prompt .. (if state.draft == "" then "" else "\n\n" .. state.draft)
end

local function prompt_message(state: State, chat: string, prompt: string): any
	local token = next_id(state)
	state.pending[token] = { kind = "prompt", prompt = prompt }
	return { to = "provider:chat", msg = { prompt = prompt, chat = chat, id = token } }
end

-- Sends the draft: a shell command, a prompt to the chat shown, or a new
-- chat that gets the prompt once it is up.
local function send(state: State): (State, { any }?)
	local text = state.draft
	local workspace = state.workspace
	if not workspace or string.match(text, "^%s*$") then
		return state, nil
	end
	local chat = current_chat(state)
	if string.sub(text, 1, 1) ~= "!" and chat
		and (state.known[chat].status == "ended" or state.known[chat].status == "failed") then
		state.error = "Resume this chat before sending a message"
		return state, nil
	end
	state.draft = ""
	state.highlight = 1
	state.error = nil
	if string.sub(text, 1, 1) == "!" then
		local command = string.match(string.sub(text, 2), "^%s*(.-)%s*$") or ""
		if command == "" then
			return state, nil
		end
		local token = next_id(state)
		state.pending[token] = { kind = "run", key = timeline_key(state), after = anchor(state) }
		return state, { { to = "provider:shell", msg = { run = command, cwd = workspace.path, id = token } } }
	end
	local key = timeline_key(state)
	local prompt = with_context(text, state.context[key] or {})
	state.context[key] = nil
	if chat then
		if state.known[chat].status ~= "live" then
			local waiting = state.waiting[chat] or {}
			table.insert(waiting, prompt)
			state.waiting[chat] = waiting
			return state, nil
		end
		return state, { prompt_message(state, chat, prompt) }
	end
	local token = next_id(state)
	state.pending[token] = { kind = "start", prompt = prompt, path = workspace.path }
	return state, { { to = "provider:chat", msg = { start = "", cwd = workspace.path, id = token } } }
end

local function start_chat(state: State, agent: string): (State, { any }?)
	local workspace = state.workspace
	if not workspace then
		return state, nil
	end
	local token = next_id(state)
	state.pending[token] = { kind = "start", path = workspace.path }
	return state, { { to = "provider:chat", msg = { start = agent, cwd = workspace.path, id = token } } }
end

-- The last shell word, decoding quoting so a completed directory with
-- spaces can itself be completed. The prefix is kept byte for byte.
local function shell_word(draft: string): (string, string)
	local start, escaped = 2, false
	local word: { string } = {}
	local quote: string = ""
	for at = 2, #draft do
		local char = string.sub(draft, at, at)
		if escaped then
			table.insert(word, char)
			escaped = false
		elseif quote == "'" then
			if char == "'" then quote = "" else table.insert(word, char) end
		elseif char == "\\" then
			escaped = true
		elseif quote == '"' then
			if char == '"' then quote = "" else table.insert(word, char) end
		elseif char == "'" or char == '"' then
			quote = char
		elseif string.match(char, "[%s;|&<>()]") then
			start, word = at + 1, {}
		else
			table.insert(word, char)
		end
	end
	return string.sub(draft, 1, start - 1), table.concat(word)
end

local function shell_quote(path: string): string
	if string.match(path, "^[%w_./%-]+$") then return path end
	return "'" .. string.gsub(path, "'", "'\\''") .. "'"
end

local function completion_request(state: State): { any }?
	if not state.workspace then return nil end
	local folder = ""
	if string.sub(state.draft, 1, 1) == "!" then
		local _, word = shell_word(state.draft)
		folder = string.match(word, "^(.+)/[^/]*$") or ""
		local previous = state.completion
		if previous and previous.path == state.workspace.path and previous.folder == folder then return nil end
		state.completion = { path = state.workspace.path, folder = folder }
	elseif state.completion then
		state.completion = nil
	else
		return nil
	end
	return { { to = "provider:projects", msg = { complete = folder, cwd = state.workspace.path } } }
end

-- Replaces the shell word, including its quotes, or a slash command.
local function complete(state: State, insert: string)
	local before = if string.sub(state.draft, 1, 1) == "!" then shell_word(state.draft) else ""
	state.draft = before .. insert
	state.highlight = 1
end

local function on_reply(state: State, msg: any): (State, { any }?)
	local pending = state.pending[msg.reply]
	if msg.action == "finished" then
		return state, nil
	end
	state.pending[msg.reply] = nil
	if not msg.ok then
		state.error = msg.error or "That did not work"
		if pending and pending.prompt then restore_prompt(state, pending.prompt) end
		return state, nil
	end
	if pending and pending.kind == "run" then
		local list = state.timeline[pending.key] or {}
		table.insert(list, { run = msg.run, after = pending.after })
		state.timeline[pending.key] = list
	elseif pending and pending.kind == "start" and msg.chat then
		state.selected[pending.path] = msg.chat
		state.known[msg.chat] = { status = "starting", working = false, cwd = pending.path }
		-- Shell cards run before the chat existed move into it.
		local loose = `ws:{pending.path}`
		if state.timeline[loose] then
			for _, item in state.timeline[loose] do item.after = 0 end
			state.timeline[msg.chat] = state.timeline[loose]
			state.timeline[loose] = nil
		end
		if state.context[loose] then
			state.context[msg.chat] = state.context[loose]
			state.context[loose] = nil
		end
		if pending.prompt then
			state.waiting[msg.chat] = { pending.prompt }
		end
		return state, { { to = "provider:chat", msg = { focus = msg.chat } } }
	end
	return state, nil
end

-- What the provider says of the chats: kept in `known`, and prompts
-- waiting for a new chat go out once it is attached.
local function on_chats(state: State, value: any): (State, { any }?)
	local known = {}
	local sends = {}
	for _, chat in (value and value.chats) or {} do
		local transcript = chat.transcript
		local working = transcript ~= nil and transcript.status ~= nil and transcript.status.state == "working"
		known[chat.id] = { status = chat.status, working = working, cwd = chat.cwd, after = last_entry(chat) }
		local waiting = state.waiting[chat.id]
		if waiting and chat.status == "live" then
			state.waiting[chat.id] = nil
			for _, prompt in waiting do table.insert(sends, prompt_message(state, chat.id, prompt)) end
		elseif waiting and (chat.status == "failed" or chat.status == "ended") then
			state.waiting[chat.id] = nil
			state.error = chat.error or "The chat did not start"
			restore_prompt(state, table.concat(waiting, "\n\n"))
		end
	end
	state.known = known
	return state, sends
end

return mod {
	title = "Chat",
	subscribe = { "switchyard/workspace", "provider/chat" },

	styles = {
		[".chat-tabs"] = { padding = { 0, 10 }, gap = 2, border = "bottom", border_color = "border" },
		[".chat-tab"] = { padding = { 10, 10, 8, 10 }, gap = 7, color = "muted-foreground", border = "bottom", border_color = "#00000000" },
		[".chat-tab:hover"] = { color = "foreground" },
		[".chat-tab:selected"] = { color = "foreground", border_color = "accent" },
		[".chat-tab-x"] = { radius = 4, color = "muted-foreground" },
		[".chat-tab-x:hover"] = { background = "list-hover", color = "foreground" },
		[".chat-thread"] = { padding = { 18, 28, 12, 28 }, gap = 10 },
		[".chat-bottom"] = { padding = { 4, 22, 14, 22 }, gap = 6 },
		[".chat-card"] = { background = "secondary", radius = 12, padding = { 10, 14 }, gap = 8, border = true, border_color = "border" },
		[".chat-card-title"] = { weight = "medium" },
		[".chat-plan-done"] = { color = "muted-foreground" },
		[".chat-working"] = { color = "muted-foreground", padding = { 2, 2 } },
		[".chat-error"] = { color = "danger", padding = { 0, 6 } },
		[".chat-empty"] = { gap = 12, padding = 32 },
		[".chat-agent"] = { background = "secondary", radius = 10, padding = { 8, 12 }, gap = 8, border = true, border_color = "border" },
		[".chat-agent:hover"] = { background = "list-hover" },
	},

	init = function(previous: State?): State
		local state: State = previous or {
			workspace = nil, selected = {}, draft = "", timeline = {}, context = {}, waiting = {},
			pending = {}, known = {}, open = {}, menu = nil, highlight = 1, error = nil, serial = 0,
		}
		state.menu = nil
		state.error = nil
		state.pending = {}
		state.waiting = {}
		state.highlight = 1
		state.completion = nil
		return state
	end,

	update = function(state: State, msg)
		if type(msg) == "table" and msg.topic == "switchyard/workspace" then
			state.workspace = msg.value
			state.menu = nil
			local sends = focus(state)
			local requests: { any } = completion_request(state) or {}
			for _, request in requests do table.insert(sends, request) end
			return state, sends
		elseif type(msg) == "table" and msg.topic == "provider/chat" then
			return on_chats(state, msg.value)
		elseif type(msg) == "table" and msg.reply ~= nil then
			return on_reply(state, msg)
		end

		local chat = current_chat(state)
		local working = chat ~= nil and state.known[chat].working
		if type(msg) == "table" and (msg.msg == "send" or msg.msg == "newline") then
			state.draft = msg.value
			msg = msg.msg
		end
		if msg == "send" then
			return send(state)
		elseif msg == "newline" then
			state.draft ..= "\n"
		elseif msg == "stop" then
			return state, chat and { { to = "provider:chat", msg = { cancel = chat } } } or nil
		elseif msg == "close-menu" then
			state.menu = nil
		elseif msg == "new-chat" then
			return start_chat(state, "")
		elseif msg == "close-chat" then
			return state, chat and { { to = "provider:chat", msg = { close = chat } } } or nil
		elseif msg == "next-chat" or msg == "previous-chat" then
			local ids = tabs(state)
			local at = table.find(ids, chat or "") or 0
			local step = if msg == "next-chat" then 1 else -1
			local id = ids[((at - 1 + step) % math.max(#ids, 1)) + 1]
			if id and state.workspace then
				state.selected[state.workspace.path] = id
				return state, focus(state)
			end
		elseif type(msg) ~= "table" then
			return
		elseif msg.escape then
			-- Esc stops what runs (the newest command, else the turn), else
			-- clears the field.
			state.menu = nil
			if msg.running then
				return state, { { to = "provider:shell", msg = { stop = msg.running } } }
			elseif working then
				return state, { { to = "provider:chat", msg = { cancel = chat } } }
			end
			state.draft = ""
		elseif msg.msg == "draft" then
			state.draft = msg.value
			state.highlight = 1
			return state, completion_request(state)
		elseif msg.highlight then
			state.highlight = msg.highlight
		elseif msg.complete then
			complete(state, msg.complete)
			return state, completion_request(state)
		elseif msg.menu then
			state.menu = if state.menu == msg.menu then nil else msg.menu
		elseif msg.select_chat and state.workspace then
			state.selected[state.workspace.path] = msg.select_chat
			return state, focus(state)
		elseif msg.close_chat then
			return state, { { to = "provider:chat", msg = { close = msg.close_chat } } }
		elseif msg.resume then
			return state, { { to = "provider:chat", msg = { resume = msg.resume, id = next_id(state) } } }
		elseif msg.pick_new then
			state.menu = nil
			return start_chat(state, msg.pick_new)
		elseif msg.pick_mode and chat then
			state.menu = nil
			return state, { { to = "provider:chat", msg = { mode = msg.pick_mode, chat = chat } } }
		elseif msg.auto_accept ~= nil and chat then
			return state, { { to = "provider:chat", msg = { auto_accept = msg.auto_accept, chat = chat } } }
		elseif msg.answer and chat then
			return state, { { to = "provider:chat", msg = {
				answer = chat, permission = msg.answer.permission, option = msg.answer.option, id = next_id(state),
			} } }
		elseif msg.unqueue and chat then
			return state, { { to = "provider:chat", msg = { unqueue = msg.unqueue, chat = chat } } }
		elseif msg.uncontext then
			local key = timeline_key(state)
			local kept = {}
			for _, item in (state.context[key] or {}) :: { any } do
				if item.id ~= msg.uncontext then
					table.insert(kept, item)
				end
			end
			state.context[key] = kept
		elseif msg.toggle then
			state.open[msg.toggle] = if state.open[msg.toggle] then nil else true
		elseif msg.shell_stop then
			return state, { { to = "provider:shell", msg = { stop = msg.shell_stop } } }
		elseif msg.rerun and state.workspace then
			local token = next_id(state)
			state.pending[token] = { kind = "run", key = timeline_key(state), after = anchor(state) }
			return state, { { to = "provider:shell", msg = { run = msg.rerun, cwd = state.workspace.path, id = token } } }
		elseif msg.add then
			local key = timeline_key(state)
			local list: { any } = state.context[key] or {}
			for _, item in list do
				if item.id == msg.add.id then
					return
				end
			end
			table.insert(list, { id = msg.add.id, label = msg.add.command, text = msg.add.text })
			state.context[key] = list
		elseif msg.terminal then
			return state, { { to = "console", msg = { show = "terminal" } } }
		else
			for key, value in msg do
				local option = string.match(key, "^pick_opt:(.+)$")
				if option and chat then
					state.menu = nil
					return state, { { to = "provider:chat", msg = { setting = option, value = value, chat = chat } } }
				end
			end
		end
	end,

	view = function(state: State, app)
		local workspace = state.workspace
		local service = app.chat
		local chats = (service and service.chats) or {}
		local by_id = {}
		for _, chat in chats do
			by_id[chat.id] = chat
		end
		local shown_id = current_chat(state)
		local shown = shown_id and by_id[shown_id]
		local transcript = shown and shown.transcript
		local key = timeline_key(state)

		-- The tab strip: this workspace's chats, then a new-chat menu.
		local strip = {}
		for _, id in tabs(state) do
			local chat = by_id[id]
			if chat then
				local title = (chat.transcript and chat.transcript.session and chat.transcript.session.title) or agent_title(chat.agent)
				local busy = state.known[id] and state.known[id].working
				table.insert(strip, ui.row { class = "chat-tab", align = "center", selected = id == shown_id, msg = { select_chat = id },
					tooltip = chat.cwd,
					ui.icon { name = agent_icon(chat.agent), size = "small" },
					ui.row { max_width = 200, ui.text { title, truncate = true } },
					busy and ui.icon { name = "loader", size = "small", spin = true },
					id == shown_id and ui.row { class = "chat-tab-x", msg = { close_chat = id }, tooltip = "Close this chat", ui.icon { name = "close", size = "small" } },
				})
			end
		end
		local agent_options = {}
		for _, agent in (service and service.agents) or {} do
			table.insert(agent_options, { id = agent, label = agent_title(agent), icon = agent_icon(agent) })
		end
		if #agent_options == 0 then
			table.insert(agent_options, { id = (service and service.default) or "claude", label = agent_title((service and service.default) or "claude") })
		end
		table.insert(strip, dropdown {
			open = state.menu == "new", value = "", icon = "plus", options = agent_options,
			toggle = { menu = "new" }, dismiss = "close-menu", event = "pick_new", width = 200, chevron = "chevron-down",
		})

		-- The thread: entries, with shell cards after the entry they followed.
		local items: { any } = {}
		local entries = (transcript and transcript.entries) or {}
		for index, entry in entries do
			table.insert(items, { order = entry.id, seq = 0, entry = entry, last = index == #entries })
		end
		local running = nil
		local runs = {}
		for _, run in (app.shell and app.shell.runs) or {} do
			runs[run.id] = run
		end
		for seq, anchor in (state.timeline[key] or {}) :: { Anchor } do
			local run = runs[anchor.run]
			if run then
				table.insert(items, { order = anchor.after, seq = seq, run = run })
				if run.status == "running" then
					running = run.id
				end
			end
		end
		table.sort(items, function(a, b)
			if a.order ~= b.order then
				return a.order < b.order
			end
			return a.seq < b.seq
		end)

		local added = {}
		for _, item in (state.context[key] or {}) :: { any } do
			added[item.id] = true
		end
		local working = transcript ~= nil and transcript.status ~= nil and transcript.status.state == "working"
		local thread = {}
		if transcript and (transcript.hidden or 0) > 0 then
			table.insert(thread, ui.text { `{transcript.hidden} earlier entries`, tone = "muted", size = "small" })
		end
		for _, item in items do
			if item.entry then
				local open_key = `{shown_id}:{item.entry.id}`
				table.insert(thread, chat_entry {
					entry = item.entry, open = state.open[open_key], toggle = { toggle = open_key },
					streaming = working and item.last,
				})
			else
				local run = item.run
				table.insert(thread, shell_card {
					run = run, stop = { shell_stop = run.id }, rerun = { rerun = run.command }, terminal = { terminal = true },
					add = { add = { id = run.id, command = run.command, text = run.text or "" } }, added = added[run.id],
				})
			end
		end

		-- The chat's own state: starting, a question for the user, its plan.
		if shown and (shown.status == "starting" or shown.status == "reconnecting") then
			table.insert(thread, loading { text = if shown.status == "starting" then `Starting {agent_title(shown.agent)}…` else "Attaching…" })
		elseif shown and (shown.status == "ended" or shown.status == "failed") then
			table.insert(thread, ui.row { class = "chat-card", align = "center",
				ui.icon { name = "circle-alert", tone = "warning" },
				ui.text { shown.error or "The chat ended", wrap = true },
				ui.spacer(),
				shown.session and ui.button { "Resume", small = true, msg = { resume = shown.id } },
				ui.button { "Close", small = true, variant = "ghost", msg = { close_chat = shown.id } },
			})
		end
		for _, permission in (transcript and transcript.permissions) or {} do
			local buttons = {}
			for _, option in permission.options or {} do
				local allows = option.kind == "allow_once" or option.kind == "allow_always"
				table.insert(buttons, ui.button { option.name, small = true,
					variant = if option.kind == "allow_once" then "primary" elseif allows then "outline" else "ghost",
					msg = { answer = { permission = permission.id, option = option.id } } })
			end
			table.insert(thread, ui.column { class = "chat-card",
				ui.row { align = "center", gap = "small", ui.icon { name = "shield", tone = "warning" },
					ui.text { permission.title, class = "chat-card-title", wrap = true } },
				ui.row { gap = "small", wrap = true, table.unpack(buttons) },
			})
		end
		if working and (#entries == 0 or entries[#entries].kind.type ~= "agent") then
			table.insert(thread, ui.row { class = "chat-working", align = "center", gap = "small",
				ui.icon { name = "loader", size = "small", spin = true }, ui.text { "Working…" } })
		end
		local plan = (transcript and transcript.plan) or {}
		local plan_rows = {}
		for _, step in plan do
			local done = step.status == "completed"
			table.insert(plan_rows, ui.row { align = "center", gap = "small",
				ui.icon { name = if done then "circle-check" elseif step.status == "in_progress" then "loader" else "circle", size = "small",
					spin = step.status == "in_progress" },
				ui.text { step.content, class = if done then "chat-plan-done" else nil, strike = done, truncate = true },
			})
		end

		-- The empty workspace: pick an agent, or just type.
		local body
		if not workspace then
			body = ui.column { grow = true, align = "center", justify = "center", loading { text = "Looking for your worktrees…" } }
		elseif service and service.available == false and #thread == 0 then
			body = ui.column { grow = true, align = "center", justify = "center", class = "chat-empty",
				ui.icon { name = "circle-alert", size = "large", tone = "warning" },
				ui.text { service.reason or "Chats are not available", wrap = true },
			}
		elseif #thread == 0 then
			local starts = {}
			for _, option in agent_options do
				table.insert(starts, ui.row { class = "chat-agent", align = "center", msg = { pick_new = option.id },
					ui.icon { name = (option.icon or "bot") :: any }, ui.text { option.label } })
			end
			body = ui.column { grow = true, align = "center", justify = "center", class = "chat-empty",
				ui.text { workspace.branch, size = "title", weight = "semibold" },
				ui.text { `{workspace.name} · start a chat with an agent, or type ! to run a command here`, tone = "muted" },
				ui.row { gap = "small", wrap = true, justify = "center", table.unpack(starts) },
			}
		else
			-- `follow`: the newest message stays in view as it streams in.
			body = ui.column { grow = true, scroll = true, follow = true, gap = "none",
				ui.row { justify = "center", ui.column { class = "chat-thread", grow = true, max_width = 920, table.unpack(thread) } },
			}
		end

		-- The composer, with this chat's settings.
		local draft = state.draft
		local shell = string.sub(draft, 1, 1) == "!"
		local completions = {}
		if shell and draft ~= "!" then
			local _, word = shell_word(draft)
			local found_folder, found_base = string.match(word, "^(.*/)([^/]*)$")
			local folder: string = found_folder or ""
			local base: string = found_base or word
			local projects = app.projects
			local listing = projects and projects.current and workspace and projects.current.path == workspace.path
				and projects.dirs and projects.dirs[string.gsub(folder, "/$", "")]
			for _, item in listing or {} do
				local kind, name = string.sub(item, 1, 1), string.sub(item, 2)
				if string.sub(name, 1, #base) == base and name ~= base and #completions < 8 then
					local dir = kind == "d" or kind == "D"
					local insert = shell_quote(folder .. name .. (if dir then "/" else ""))
					table.insert(completions, { label = name .. (if dir then "/" else ""), icon = if dir then "folder" else "file", insert = insert })
				end
			end
		elseif string.match(draft, "^/%S*$") and transcript then
			for _, command in transcript.commands or {} do
				if string.sub("/" .. command.name, 1, #draft) == draft and #completions < 8 then
					table.insert(completions, { label = "/" .. command.name, detail = command.description, icon = "zap", insert = `/{command.name} ` })
				end
			end
		end

		local settings = {}
		for _, option in (transcript and transcript.options) or {} do
			if option.kind and option.kind.type == "select" then
				local choices, value = {}, option.kind.current
				for _, choice in option.kind.choices or {} do
					table.insert(choices, { id = choice.id, label = choice.name })
					if choice.id == option.kind.current then
						value = choice.name
					end
				end
				table.insert(settings, {
					key = `opt:{option.id}`, value = value, options = choices, selected = option.kind.current,
					open = state.menu == `opt:{option.id}`, order = SETTING_ORDER[option.category or "other"] or 5,
					icon = if option.category == "model" then agent_icon(shown and shown.agent) else nil,
				})
			end
		end
		local modes = transcript and transcript.modes
		if modes and #(modes.available or {}) > 0 then
			local choices, value = {}, modes.current
			for _, mode in modes.available do
				table.insert(choices, { id = mode.id, label = mode.name })
				if mode.id == modes.current then
					value = mode.name
				end
			end
			table.insert(settings, { key = "mode", value = value, options = choices, selected = modes.current, open = state.menu == "mode", order = 4 })
		end
		table.sort(settings, function(a, b)
			return a.order < b.order
		end)

		local usage = transcript and transcript.usage
		local usage_text = nil
		if usage and usage.size and usage.size > 0 then
			usage_text = `{math.floor(usage.used * 100 / usage.size)}% context`
			if usage.cost then
				usage_text ..= string.format(" · $%.2f", usage.cost.micros / 1000000)
			end
		end
		local context_chips = {}
		for _, item in (state.context[key] or {}) :: { any } do
			table.insert(context_chips, { id = item.id, label = item.label })
		end
		local queued = {}
		for _, item in (transcript and transcript.queued) or {} do
			table.insert(queued, { id = item.id, text = item.text })
		end

		return ui.column { grow = true, gap = "none",
			ui.row { class = "chat-tabs", align = "center", height = 44, shrink = false,
				ui.row { gap = "none", align = "center", scroll = "x", grow = true, table.unpack(strip) },
				icon_button { icon = "panel-bottom", tooltip = "Show or hide the console", msg = "toggle-bottom", to = "shell", size = "small" },
			},
			body,
			ui.column { class = "chat-bottom", shrink = false,
				#plan_rows > 0 and ui.column { class = "chat-card", ui.text { "Plan", class = "chat-card-title" }, table.unpack(plan_rows) },
				state.error and ui.text { state.error, class = "chat-error", wrap = true },
				composer {
					draft = draft, shell = shell, completions = completions, highlighted = state.highlight,
					working = working, settings = settings,
					auto_accept = if transcript then transcript.auto_accept == true else nil,
					usage = usage_text, context = context_chips, queued = queued,
					placeholder = if shown then `Message {agent_title(shown.agent)}, or ! for a shell command…` else nil,
					escape = { escape = true, running = running },
				},
			},
		}
	end,
}
