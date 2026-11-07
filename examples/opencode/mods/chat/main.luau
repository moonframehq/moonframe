-- The session: its header, transcript and composer. The agent is scripted:
-- the sample transcripts come from `app.sessions`, and what you send gets a
-- canned answer that says so. A real one would be a provider (`acp` or
-- `chat`) publishing its replies here. Messages with Hello in them really go
-- to the greeter service.

local chat_markdown = use "chat-markdown"
local dictation = use "dictation"
local dialog = use "dialog"
local dropdown = use "dropdown"
local icon_button = use "icon-button"
local mention_field = use "mention-field"
local menu = use "menu"

type State = {
	current: string?,
	list: { any },
	extra: { [string]: { any } },
	open_tools: { [string]: boolean },
	draft: string,
	caret: number,
	highlighted: number,
	no_suggestions: boolean,
	model: string,
	effort: string,
	dropdown: string?,
	session_menu: boolean,
	renaming: boolean,
	new_title: string,
	greeter: any,
	toast_hidden: number,
	attached: { string },
	listening: boolean,
}

local MODELS = {
	{ id = "opus", label = "Claude Opus 5.5" },
	{ id = "sonnet", label = "Claude Sonnet 5.5" },
	{ id = "haiku", label = "Claude Haiku 5.5" },
}
local EFFORTS = { { id = "low", label = "Low" }, { id = "medium", label = "Medium" }, { id = "high", label = "High" } }
local COMMANDS = {
	{ id = "new", name = "new", detail = "Start a new session", icon = "square-pen", insert = "" },
	{ id = "clear", name = "clear", detail = "Forget what you sent here", icon = "trash-2", insert = "" },
}

local function current_session(state: State): any
	for _, session: any in state.list do
		if session.id == state.current then
			return session
		end
	end
	return nil
end

local function sample(app, id: string?): any
	for _, session: any in (app.sessions and app.sessions.sessions) or {} do
		if session.id == id then
			return session
		end
	end
	return nil
end

-- What a turn the user sent does: the canned answer, and any sends.
local function answer(state: State, text: string): (string, { any })
	local name = string.match(text, "^[Hh]ello,?%s+(%S+)")
	if name and not (state.greeter and state.greeter.on) then
		return "The greeter service is off, so nobody answered. Turn it on in the **Hello** tab.", {}
	elseif name then
		name = string.gsub(name, "[%p]+$", "")
		return `I asked the greeter service to say hello to **{name}**. Its answer is above the composer and in Echo.`,
			{ { to = "hello", msg = { greet = name } } }
	end
	return "This is a demo, so no model ran and nothing changed. A real agent would answer here, and its changes would land in **Files Changed**.\n\nTry `hello Kit`, which does reach the greeter service.", {}
end

local function send(state: State, text: string)
	local words = string.match(text, "^%s*(.-)%s*$") or ""
	local id = state.current
	if words == "" or not id then
		return state
	end
	local session = current_session(state)
	local turns = state.extra[id] or {}
	state.extra[id] = turns
	local reply, sends = answer(state, words)
	table.insert(turns, { from = "user", text = words, attached = state.attached })
	table.insert(turns, { from = "agent", text = reply })
	table.insert(sends, { to = "sidebar", msg = { spent = id, tokens = 600 + #words * 4 } })
	state.draft = ""
	state.attached = {}
	if session and session.title == "New session" then
		table.insert(sends, { to = "sidebar", msg = { rename = id, title = string.sub(words, 1, 60) } })
	end
	return state, sends
end

local function tokens(value: number): string
	return if value < 1000 then tostring(value) else string.format("%.1fK", value / 1000)
end

local function tools_row(state: State, key: string, tools: any)
	local open = state.open_tools[key] == true
	local steps = {}
	if open then
		for _, step in tools.steps do
			table.insert(steps, ui.text { step, class = "tool-step" })
		end
	end
	return ui.column { gap = "small",
		ui.row { class = "tools", align = "center", hug = true, msg = { tools = key },
			ui.text { "Used", tone = "muted" },
			ui.text { `{tools.count} {tools.names}`, weight = "medium" },
			ui.icon { if open then "chevron-down" else "chevron-right", tone = "muted" },
		},
		table.unpack(steps),
	}
end

-- A paragraph with links, as a wrapped row of words: the host's markdown draws
-- links in the text colour, and here they are blue. Everything else in an
-- answer stays markdown.
local function linked_paragraph(paragraph: string)
	local words = {}
	local rest = paragraph
	while rest ~= "" do
		local before, label, url, after = string.match(rest, "^(.-)%[([^%]]+)%]%(([^%)]+)%)(.*)$")
		for word in string.gmatch(before or rest, "%S+") do
			table.insert(words, ui.text(word))
		end
		if not label then
			break
		end
		-- Punctuation right after a link stays on it.
		local remainder: string = after or ""
		local tail = string.match(remainder, "^[%.,;:]+") or ""
		rest = string.sub(remainder, #tail + 1)
		table.insert(words, ui.row { gap = "none", hug = true,
			ui.row { class = "doc-link", msg = { msg = "link", url = url }, ui.text(label :: string) },
			tail ~= "" and ui.text(tail),
		})
	end
	return ui.row { wrap = true, gap = "small", table.unpack(words) }
end

local function answer_blocks(text: string): { any }
	local blocks: { any } = {}
	local plain: { string } = {}
	local function flush()
		if #plain > 0 then
			table.insert(blocks, chat_markdown { text = table.concat(plain, "\n\n") })
			plain = {}
		end
	end
	for paragraph in string.gmatch(text .. "\n\n", "(.-)\n\n") do
		if string.find(paragraph, "%]%(") then
			flush()
			table.insert(blocks, linked_paragraph(paragraph))
		else
			table.insert(plain, paragraph)
		end
	end
	flush()
	return blocks
end

local function turn_view(state: State, key: string, turn: any)
	if turn.from == "user" then
		local names = {}
		for _, path in turn.attached or {} do
			table.insert(names, ui.text { `@ {string.match(path, "[^/]+$")}`, tone = "muted", size = "small" })
		end
		return ui.row { justify = "end",
			ui.column { class = "bubble", max_width = 430, gap = "small",
				ui.text(turn.text),
				table.unpack(names),
			},
		}
	end
	return ui.column { class = "answer",
		turn.tools and tools_row(state, key, turn.tools),
		table.unpack(answer_blocks(turn.text)),
	}
end

local function composer(state: State, session: any, app)
	local people: { any } = {}
	if state.no_suggestions then
		-- Dismissed: none until the next edit, so Enter sends the draft.
		people = {}
	elseif string.sub(state.draft, 1, 1) == "/" then
		people = COMMANDS
	else
		local data = sample(app, state.current)
		for _, file in (data and data.files) or {} do
			table.insert(people, { name = file.path, icon = "file", insert = `@{file.path} ` })
		end
	end
	local chips = {}
	for index, path in state.attached do
		table.insert(chips, ui.row { class = "chip", align = "center", msg = { detach = index }, tooltip = "Remove",
			ui.icon { "file", tone = "muted" }, ui.text { string.match(path, "[^/]+$") or path, size = "small" }, ui.icon { "close", tone = "muted" } })
	end
	local ready = string.match(state.draft, "%S") ~= nil
	return ui.column { class = "composer-box", shrink = false, gap = "none",
		#chips > 0 and ui.row { gap = "small", wrap = true, table.unpack(chips) },
		ui.column { class = "composer-field",
			mention_field {
				id = "prompt", value = state.draft, caret = state.caret, grow = true, focus = true,
				trigger = if string.sub(state.draft, 1, 1) == "/" then "/" else "@",
				heading = if string.sub(state.draft, 1, 1) == "/" then "Commands" else "Context",
				people = people, highlighted = state.highlighted, width = 320,
				placeholder = "Ask anything, / for commands, @ for context...",
				change = "draft", submit = "send", dismiss = "no-suggestions",
			},
		},
		ui.row { align = "center", gap = "none",
			icon_button { icon = "plus", tooltip = "Attach a file", msg = "attach" },
			dropdown {
				side = "top", options = MODELS, selected = state.model, event = "model", width = 200,
				open = state.dropdown == "model", toggle = { dropdown = "model" }, dismiss = "close-dropdown",
			},
			dropdown {
				side = "top", options = EFFORTS, selected = state.effort, event = "effort", width = 130,
				open = state.dropdown == "effort", toggle = { dropdown = "effort" }, dismiss = "close-dropdown",
			},
			ui.spacer(),
			dictation { id = "prompt", text = "dictated", listening = "listening" },
			ui.row { class = if ready then "send-on" else "send", width = 30, height = 30, align = "center", justify = "center",
				msg = if ready then "send-click" else nil, tooltip = "Send",
				ui.icon "arrow-up",
			},
		},
	}
end

return mod {
	subscribe = { "opencode/sessions", "opencode/greeter", "opencode/page" },

	styles = {
		[".chat-head"] = { padding = { 10, 10, 10, 14 }, gap = 8 },
		[".crumb-title"] = { clip = true },
		[".crumb-fade"] = { gradient = "90deg, #17171700, #171717" },
		[".state-pill"] = { background = "#242424", radius = 999, padding = { 3, 10, 3, 8 }, gap = 6, color = "#c8c8c8" },
		[".state-on"] = { background = "success", radius = 999 },
		[".state-off"] = { background = "muted-foreground", radius = 999 },
		[".transcript"] = { padding = { 20, 26, 10, 26 }, gap = 26 },
		[".bubble"] = { background = "#2b2b2b", radius = 12, padding = { 9, 13 } },
		[".answer"] = { gap = 14 },
		[".doc-link"] = { color = "link" },
		[".doc-link:hover"] = { color = "foreground" },
		[".tools"] = { gap = 5, padding = { 2, 0 } },
		[".tool-step"] = { color = "muted-foreground", size = "small", font = "mono" },
		[".toast-pill"] = { background = "#242424", radius = 999, padding = { 5, 12, 5, 10 }, gap = 6 },
		[".toast-pill:hover"] = { background = "#2c2c2c" },
		[".composer-wrap"] = { padding = { 6, 12, 12, 12 }, gap = 8 },
		[".composer-box"] = { background = "#212121", radius = 12, border = true, border_color = "#2c2c2c", padding = { 12, 10, 8, 10 } },
		[".composer-field"] = { padding = { 3, 6, 14, 6 } },
		[".chip"] = { background = "#2c2c2c", radius = 6, padding = { 3, 7 }, gap = 5, margin = { 0, 0, 8, 6 } },
		[".send"] = { background = "#2c2c2c", color = "#6e6e6e", radius = 8 },
		[".send-on"] = { background = "foreground", color = "#111111", radius = 8 },
		[".dictation"] = { color = "muted-foreground" },
	},

	init = function(previous: State?): State
		local state = previous or {
			list = {}, extra = {}, open_tools = {}, draft = "", caret = 0, highlighted = 1,
			model = "opus", effort = "high", toast_hidden = -1, attached = {}, new_title = "",
		} :: any
		state.dropdown = nil
		state.session_menu = false
		state.renaming = false
		state.listening = false
		state.no_suggestions = false
		return state
	end,

	update = function(state: State, msg)
		if msg == "send-click" then
			return send(state, state.draft)
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
		elseif msg == "hide-toast" then
			state.toast_hidden = (state.greeter and state.greeter.count) or -1
		elseif msg == "attach" then
			return state, { { to = "app:pick", msg = { kind = "open", multiple = true, reply = { attached = true } } } }
		elseif type(msg) ~= "table" then
			return
		elseif msg.topic == "opencode/sessions" and msg.value then
			state.list = msg.value.list
			state.current = msg.value.current
		elseif msg.topic == "opencode/greeter" then
			state.greeter = msg.value
		elseif msg.topic == "opencode/page" then
			-- Home draws no mic button, so nothing else would stop it.
			if msg.value ~= "chat" and state.listening then
				state.listening = false
				return state, { { to = "provider:speech", msg = { cancel = true } } }
			end
		elseif msg.msg == "listening" then
			state.listening = msg.value == true
		elseif msg.msg == "draft" then
			if msg.value ~= state.draft then
				state.highlighted = 1
				state.no_suggestions = false
			end
			state.draft = msg.value
			state.caret = msg.caret or #msg.value
		elseif msg.msg == "no-suggestions" then
			state.no_suggestions = true
		elseif msg.msg == "send" then
			return send(state, msg.value)
		elseif msg.msg == "dictated" then
			state.draft = if state.draft == "" then msg.value else `{state.draft} {msg.value}`
			state.caret = #state.draft
		elseif msg.msg == "rename-title" then
			state.new_title = msg.value
		elseif msg.msg == "link" then
			return state, { { to = "workbench", msg = { open_doc = msg.url } } }
		elseif msg.mention == "up" or msg.mention == "down" then
			state.highlighted = math.max(1, state.highlighted + (if msg.mention == "up" then -1 else 1))
		elseif msg.mention == "choose" then
			local sends: { any } = { { to = "app:edit", msg = msg.edit } }
			if msg.item == "new" then
				table.insert(sends, { to = "sidebar", msg = "new-session" })
			elseif msg.item == "clear" and state.current then
				state.extra[state.current] = nil
			end
			return state, sends
		elseif msg.tools then
			state.open_tools[msg.tools] = not state.open_tools[msg.tools]
		elseif msg.dropdown then
			state.dropdown = if state.dropdown == msg.dropdown then nil else msg.dropdown
		elseif msg.model then
			state.model = msg.model
			state.dropdown = nil
		elseif msg.effort then
			state.effort = msg.effort
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
			return ui.column { grow = true, justify = "center", align = "center", ui.text { "Pick a session", tone = "muted" } }
		end
		local data = sample(app, session.id)
		local turns = {}
		for index, turn in (data and data.turns) or {} do
			table.insert(turns, turn_view(state, `{session.id}-{index}`, turn))
		end
		local sent: { any } = state.extra[session.id] or {}
		for index, turn in sent do
			table.insert(turns, turn_view(state, `{session.id}-x{index}`, turn))
		end
		if #turns == 0 then
			table.insert(turns, ui.column { grow = true, align = "center", justify = "center", height = 240,
				ui.text { "What should we work on?", size = "large", weight = "medium" },
				ui.text { "This is a demo: the agent is scripted.", tone = "muted" },
			})
		end

		local greeter = state.greeter
		local on = greeter and greeter.on
		local toast = greeter and greeter.answer and greeter.count ~= state.toast_hidden

		return ui.column { grow = true, gap = "none",
			ui.row { class = "chat-head", align = "center", shrink = false,
				ui.row { class = "tile", width = 18, height = 18, shrink = false, align = "center", justify = "center", ui.text(session.letter) },
				ui.text { session.project, tone = "muted" },
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
				ui.row { class = "state-pill", align = "center", shrink = false,
					ui.row { class = if on then "state-on" else "state-off", width = 7, height = 7 },
					ui.text { if on then "Greeter on" else "Greeter off", size = "small", weight = "medium" },
				},
				ui.icon { "loader-circle", tone = "muted" },
				ui.text { tokens(session.tokens) },
				ui.text { string.format("$%.2f", session.cost), tone = "muted" },
			},
			-- The inner column keeps its height, or the scroll squeezes the turns.
			ui.column { grow = true, scroll = true, follow = true, gap = "none",
				ui.column { class = "transcript", shrink = false, table.unpack(turns) },
			},
			ui.column { class = "composer-wrap", shrink = false,
				toast and ui.row { justify = "center",
					ui.row { class = "toast-pill", align = "center", msg = "hide-toast", tooltip = "Dismiss",
						ui.icon { "wifi", tone = "muted" },
						ui.text { `Hello says: {greeter.answer}`, size = "small", weight = "medium" },
					},
				},
				composer(state, session, app),
			},
			dialog {
				open = state.renaming, dismiss = "close-rename", title = "Rename session", size = "small", submit = "save-rename",
				buttons = { { label = "Cancel", msg = "close-rename", variant = "ghost" }, { label = "Rename", msg = "save-rename", variant = "primary" } },
				ui.input { id = "rename", class = "field", value = state.new_title, change = "rename-title", submit = "save-rename", focus = true },
			},
		}
	end,
}
