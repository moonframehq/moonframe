-- The agent chat, beside any layout: talk to a coding agent (Claude, Codex…)
-- about the vault, and have it do things there: answer from your notes,
-- file a source, tidy a page, keep the wiki. What you send goes to the
-- built-in `acp` provider, one run per message, working in the vault's
-- folder; the runs of one conversation share a `thread`, and a follow-up
-- carries the earlier requests and answers, since each run starts the agent
-- afresh. The agent asks before it changes anything (providers/acp `allow`).

local chat_markdown = use "chat-markdown"
local composer = use "composer"
local agent_run = use "agent-run"
local icon_button = use "icon-button"

type State = {
	draft: string,
	-- The conversation shown; a new chat starts another.
	thread: string,
	chats: number,
	-- The note in view in the layout, as it last said.
	note: string?,
	starting: boolean,
	error: string?,
}

local INSTRUCTIONS = [==[
You are the assistant of Commonplace, the user's second brain: a folder of markdown notes (Obsidian-compatible) that is your working folder. Read AGENTS.md at its root first: it says how the vault is laid out (inbox.md, journal/, notes/, raw/ for sources, wiki/ for the pages you keep, index.md, log.md) and how to change it. Keep each note's frontmatter and its `id`. Link notes with [[Title]] wikilinks. When you change the vault, add a line to log.md saying what you did. Do only what the user asks. End with a short answer in Markdown that names the notes you used or changed as [[Title]] links.]==]

local IN_VIEW = [[


`open_note` in the context is the path of the note the user has open; "this note" means it.]]

local FOLLOW_UP = [[


This continues a conversation: `conversation` in the context has the user's earlier requests and the answers given, oldest first. Read the notes as they are now rather than trusting the earlier answers.]]

local function going(run: any): boolean
	return run.status == "starting" or run.status == "running" or run.status == "waiting"
end

-- This conversation's runs, oldest first.
local function thread_runs(app: any, thread: string): { any }
	local found = {}
	local runs = (app.acp and app.acp.runs) or {}
	for index = #runs, 1, -1 do
		local run = runs[index]
		if run.owner == "chat" and run.thread == thread then
			table.insert(found, run)
		end
	end
	return found
end

local function wikify(text: string): string
	local out = string.gsub(text, "%[%[([^%]|]+)|([^%]]+)%]%]", "[%2](<note:%1>)")
	out = string.gsub(out, "%[%[([^%]]+)%]%]", "[%1](<note:%1>)")
	return out
end

local function turn(run: any)
	return ui.column {
		class = "chat-turn",
		ui.row { justify = "end", ui.column { class = "chat-bubble", max_width = 300, ui.text { run.text, wrap = true } } },
		agent_run { run = run, lines = 0 },
		run.message and chat_markdown { text = wikify(run.message), link = "link" },
	}
end

local function empty(vault: string?)
	return ui.column {
		class = "chat-empty",
		grow = true,
		justify = "center",
		align = "center",
		ui.icon { name = "sparkles", class = "chat-empty-icon" },
		ui.text { "Ask about your notes, or have the agent do something in them.", class = "muted small", wrap = true },
		ui.text { "“What did I save about Rolleiflex cameras?”", class = "muted small", wrap = true },
		ui.text { "“Summarise this note into the wiki.”", class = "muted small", wrap = true },
		not vault and ui.text { "Waiting for the vault…", class = "muted small" },
	}
end

return mod {
	title = "Agent",
	subscribe = { "commonplace/open" },

	styles = {
		[".chat"] = { background = "card", border = "left", border_color = "border" },
		[".chat-head"] = { padding = { 12, 12, 12, 16 }, gap = 6, border = "bottom", border_color = "border" },
		[".chat-title"] = { weight = "semibold" },
		[".chat-transcript"] = { padding = { 16, 16, 8, 16 }, gap = 22 },
		[".chat-turn"] = { gap = 10 },
		[".chat-bubble"] = { background = "list-active", radius = 12, padding = { 8, 12 } },
		[".chat-foot"] = { padding = { 6, 12, 12, 12 }, gap = 6 },
		[".chat-note"] = { size = "small", color = "muted-foreground", padding = { 0, 4 } },
		[".chat-error"] = { size = "small", color = "danger", padding = { 0, 4 } },
		[".chat-empty"] = { gap = 8, padding = { 0, 28 } },
		[".chat-empty-icon"] = { color = "accent", size = "large" },
	},

	init = function(previous: any): State
		local state: State = previous or { draft = "", thread = "chat-1", chats = 1 }
		state.starting = false
		state.error = nil
		return state
	end,

	update = function(state: State, msg)
		if msg == "new-chat" then
			state.chats += 1
			state.thread = `chat-{state.chats}`
			state.error = nil
			return
		elseif msg == "close" then
			return state, { { to = "shell", msg = { chat = false } } }
		elseif type(msg) ~= "table" then
			return
		elseif msg.topic == "commonplace/open" then
			state.note = msg.value and msg.value.path
		elseif msg.draft then
			state.draft = msg.value or ""
		elseif msg.stop then
			return state, { { to = "provider:acp", msg = { cancel = msg.stop } } }
		elseif msg.send then
			-- From Enter (with `value`) or the button (the kept draft).
			local words = string.match(msg.value or state.draft, "^%s*(.-)%s*$") or ""
			if words == "" or state.starting or msg.busy or not msg.cwd then
				return
			end
			local instructions = INSTRUCTIONS
			local context: { [string]: any } = {}
			if state.note then
				instructions ..= IN_VIEW
				context.open_note = state.note
			end
			if msg.said and #msg.said > 0 then
				instructions ..= FOLLOW_UP
				context.conversation = msg.said
			end
			state.draft = ""
			state.error = nil
			state.starting = true
			return state, { { to = "provider:acp", msg = {
				start = words,
				id = "chat",
				thread = state.thread,
				cwd = msg.cwd,
				instructions = instructions,
				context = context,
			} } }
		elseif msg.reply == "chat" and msg.action == "start" then
			state.starting = false
			if not msg.ok then
				state.error = msg.error or "The agent could not start."
			end
		elseif msg.msg == "link" then
			-- A note it names opens in the layout.
			return state, { { to = "shell", msg = { link = msg } } }
		end
	end,

	view = function(state: State, app)
		local notes: any = app.notes or {}
		local vault = notes.vault and notes.vault.path
		local runs = thread_runs(app, state.thread)
		local turns = {}
		local said = {}
		local busy = nil
		for _, run in runs do
			table.insert(turns, turn(run))
			table.insert(said, { request = run.text, answer = run.message or run.error or run.status })
			if going(run) then
				busy = run.id
			end
		end
		local send = { send = true, cwd = vault, said = said, busy = busy ~= nil }
		return ui.column {
			class = "chat",
			grow = true,
			gap = "none",
			ui.row {
				class = "chat-head",
				align = "center",
				ui.icon { name = "sparkles", tone = "accent" },
				ui.text { "Agent", class = "chat-title" },
				ui.spacer(),
				busy and icon_button { icon = "square", tooltip = "Stop the agent", size = "small", msg = { stop = busy } },
				icon_button { icon = "square-pen", tooltip = "New chat", size = "small", msg = "new-chat" },
				icon_button { icon = "close", tooltip = "Close (⌘J)", size = "small", msg = "close" },
			},
			if #turns == 0
				then empty(vault)
				else ui.column { class = "chat-transcript", grow = true, scroll = true, follow = true, table.unpack(turns) },
			ui.column {
				class = "chat-foot",
				shrink = false,
				state.error and ui.text { state.error, class = "chat-error", wrap = true },
				state.note and ui.text { `Sees {state.note}`, class = "chat-note", truncate = true },
				composer {
					id = "chat",
					value = state.draft,
					placeholder = if busy then "The agent is working…" else "Ask or tell the agent…",
					change = { draft = true },
					submit = send,
					send = send,
					focus = true,
				},
			},
		}
	end,
}
