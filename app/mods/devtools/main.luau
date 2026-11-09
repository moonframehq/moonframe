-- Developer tools: what Inspect picked, its Luau source to edit and save
-- (the window reloads with it at once), its mod's state, and an agent to
-- ask for a change, in a session that stays open. The app draws it docked
-- on the right of the main window, around the shell, so no app's layout
-- makes room for it; or in a window of its own: drag the dock's grip out
-- of it, or use its buttons. View > Developer Tools (⌥⌘J) shows it, in
-- Developer Mode. It hears each pick on `app/selection` and where it is on
-- `app/devtools`, and asks the app with `app:devtools`, which only it may.
-- Agent sessions are the feedback mod's runs: it sends requests there.
local tabs = use "tabs"
local key_values = use "key-values"
local breadcrumbs = use "breadcrumbs"
local empty_state = use "empty-state"
local code_view = use "code-view"
local chat_message = use "chat-message"
local agent_run = use "agent-run"
local composer = use "composer"
local kbd = use "kbd"
local status_dot = use "status-dot"
local chat_markdown = use "chat-markdown"

local APP = "app:devtools"
-- The mod that starts agent runs and keeps their conversations.
local FEEDBACK = "feedback"
-- How far the grip is dragged before the dock becomes a window.
local TEAR = 60
-- Crumbs of the trail shown before the rest fold into "…".
local CRUMBS = 4

type Source = {
	path: string,
	-- The text as read or last saved, passed to the editor when it changes.
	loaded: string?,
	-- The text in the editor.
	text: string?,
	dirty: boolean,
	status: string?,
	error: string?,
}

type State = {
	place: string,
	tab: string,
	-- The pick, from `app/selection`.
	pick: any?,
	source: Source?,
	-- The picked mod's state, from `app:devtools`.
	inspected: any?,
	-- The grip was dragged far enough: it tore off once this drag.
	torn: boolean,
	-- Whether this app has the feedback mod, which runs the agents.
	agent: boolean,
	-- The agent session open: its first run's id, or nil for the list (in
	-- a window, a new session beside it).
	session: string?,
	-- Sessions there were when a new one was asked for: the next one to
	-- show up opens.
	awaiting: number?,
	-- Runs whose steps are unfolded, by id.
	steps: { [string]: boolean },
	prompt: string,
	runs: { any },
}

-- `~/…` for a path in the home folder, and the last three parts otherwise
-- long.
local function short(path: string): string
	local home = string.match(path, "^(/Users/[^/]+)") or string.match(path, "^(/home/[^/]+)")
	if home then
		path = "~" .. string.sub(path, #home + 1)
	end
	return string.match(path, "([^/]+/[^/]+/[^/]+)$") or path
end

local function first_line(text: string): string
	return string.match(text, "^%s*([^\n]*)") or text
end

local function going(run: any): boolean
	return run.status == "starting" or run.status == "running" or run.status == "waiting"
end

-- Data as Luau, indented, for the state tab.
local function pretty(value: any, indent: string?): string
	local pad = indent or ""
	if type(value) == "string" then
		return string.format("%q", value)
	elseif type(value) ~= "table" then
		return tostring(value)
	end
	local keys = {}
	for key in value do
		table.insert(keys, key)
	end
	if #keys == 0 then
		return "{}"
	end
	table.sort(keys, function(a, b)
		return tostring(a) < tostring(b)
	end)
	local inner = pad .. "\t"
	local lines = {}
	local list = #value == #keys
	for _, key in keys do
		local name = if list then "" elseif type(key) == "string" and string.match(key, "^[%a_][%w_]*$") then `{key} = ` else `[{pretty(key)}] = `
		table.insert(lines, `{inner}{name}{pretty(value[key], inner)},`)
	end
	return "{\n" .. table.concat(lines, "\n") .. "\n" .. pad .. "}"
end

-- The feedback mod's runs as conversations, newest first: each request
-- with its follow-ups, oldest first.
local function conversations(runs: { any }): { { any } }
	local by: { [string]: { any } } = {}
	local order: { { any } } = {}
	for _, run in runs do
		if run.thread == nil then
			by[run.id] = { run }
			table.insert(order, by[run.id])
		end
	end
	for index = #runs, 1, -1 do
		local run = runs[index]
		if run.thread ~= nil and by[run.thread] then
			table.insert(by[run.thread], run)
		end
	end
	return order
end

-- Asks for the picked mod's source and state.
local function load(state: State): { any }
	local pick = state.pick
	local sends: { any } = {}
	if pick and pick.file and (state.source == nil or state.source.path ~= pick.file) then
		state.source = { path = pick.file, dirty = false }
		table.insert(sends, { to = APP, msg = { read = pick.file, reply = { source = pick.file } } })
	end
	if pick and pick.mod then
		table.insert(sends, { to = APP, msg = { state = pick.mod, reply = { inspected = pick.mod } } })
	end
	return sends
end

-- Sends `text` to the agent: a reply in the session shown, else a new
-- request about the pick. The tools stay as they are.
local function ask(state: State, text: string?): any
	text = text or state.prompt
	if string.match(text or "", "^%s*$") then
		return nil
	end
	state.prompt = ""
	if state.session then
		return state, { { to = FEEDBACK, msg = { follow = state.session, value = text, quiet = true } } }
	end
	-- A new session: it opens once it starts.
	state.awaiting = #conversations(state.runs)
	local context = {}
	if state.pick then
		context.inspect_element_selection = state.pick.report
	end
	return state, { { to = FEEDBACK, msg = { send = true, value = text, context = context, quiet = true } } }
end

local function header(state: State): any
	local docked = state.place ~= "window"
	return ui.row {
		class = "devtools-head",
		align = "center",
		gap = "small",
		ui.row {
			class = "devtools-grip",
			align = "center",
			gap = "small",
			pointer = if docked then "grip" else nil,
			cursor = if docked then "grab" else nil,
			tooltip = if docked then "Drag out to open in a window of its own" else nil,
			ui.icon { name = "grip-vertical", size = "small" },
			ui.text { "Developer", weight = "semibold" },
		},
		ui.spacer(),
		ui.button {
			icon = if docked then "maximize-2" else "panel-right",
			small = true,
			variant = "ghost",
			msg = { place = if docked then "window" else "dock" },
			tooltip = if docked then "Open in a window of its own" else "Dock beside the window",
		},
		docked and ui.button {
			icon = "close",
			small = true,
			variant = "ghost",
			msg = { place = "closed" },
			tooltip = "Close (⌥⌘J)",
		},
	}
end

local function pick_button(): any
	return ui.button {
		"Pick  ⌥⌘I",
		icon = "inspector",
		small = true,
		variant = "primary",
		command = "inspect",
		tooltip = "Pick an element in the window (⌥⌘I)",
	}
end

-- What the tools work on, with Pick at its end: the two read as one.
local function target(pick: any?): any
	local name, detail
	if pick then
		name = pick.element
		detail = `mod {pick.mod}`
		if pick.file then
			detail ..= ` · {string.match(pick.file, "([^/]+)$") or pick.file}{if pick.line then `:{pick.line}` else ""}`
		end
		detail ..= ` · {pick.width} × {pick.height}`
	end
	return ui.row {
		class = if pick then "devtools-target" else "devtools-target devtools-target-none",
		align = "center",
		gap = "small",
		ui.icon { name = "inspector", size = "small", class = if pick then "devtools-accent" else "devtools-muted" },
		ui.column {
			grow = true,
			gap = "none",
			ui.text { name or "Nothing picked", class = if pick then "devtools-path" else "devtools-muted", truncate = true, tooltip = name },
			ui.text { detail or "Click any element in the window", class = "devtools-muted", truncate = true, tooltip = detail },
		},
		pick_button(),
	}
end

-- Where the pick sits: the mods and elements around it, each a click away.
local function trail(pick: any): any
	local crumbs = {}
	local all: { any } = pick.trail or {}
	local from = math.max(1, #all - CRUMBS + 1)
	if from > 1 then
		table.insert(crumbs, { label = "…" })
	end
	for index = from, #all do
		local crumb = all[index]
		table.insert(crumbs, {
			label = crumb.label,
			msg = if crumb.path then { select = crumb.path } else nil,
		})
	end
	return breadcrumbs { crumbs = crumbs }
end

-- Before anything is picked, every tab shows this.
local function unpicked(): any
	return ui.column {
		class = "devtools-hero",
		align = "center",
		gap = "normal",
		ui.row {
			class = "devtools-hero-glyph",
			width = 44,
			height = 44,
			align = "center",
			justify = "center",
			ui.icon { name = "inspector", size = "large" },
		},
		ui.text { "Pick an element", weight = "semibold" },
		ui.text {
			"Its source, its mod's state and an agent session open here.",
			class = "devtools-muted",
			wrap = true,
		},
		ui.button { "Pick element  ⌥⌘I", icon = "inspector", variant = "primary", command = "inspect" },
	}
end

local function element(state: State): any
	local pick = state.pick
	local rows = {
		{ "Element", pick.element },
		{ "Mod", pick.mod },
		{ "Built at", short(pick.at) },
		{ "Size", `{pick.width} × {pick.height}` },
	}
	return ui.column {
		gap = "large",
		grow = true,
		scroll = true,
		key_values { rows = rows, width = 72 },
		ui.row {
			gap = "small",
			wrap = true,
			pick.file and ui.button { "Edit source", icon = "code", small = true, variant = "outline", msg = { tab = "source" } },
			ui.button { "Ask an agent", icon = "sparkles", small = true, variant = "outline", msg = { tab = "agent" } },
			ui.button { "Copy for agent", icon = "copy", small = true, variant = "ghost", copy = pick.report },
		},
		ui.text { "Saved for agents, who read it when you say \"here\".", class = "devtools-muted" },
	}
end

local function source(state: State): any
	local file = state.source
	if file == nil then
		return empty_state { icon = "code", title = "No source", message = "This element was not drawn by a mod's file." }
	end
	local status = if file.error
		then ui.text { file.error, class = "devtools-error", truncate = true, tooltip = file.error }
		elseif file.dirty then ui.text { "● Unsaved", class = "devtools-warn" }
		else ui.text { file.status or "", class = "devtools-ok" }
	local line = state.pick and state.pick.file == file.path and state.pick.line
	return ui.column {
		gap = "small",
		grow = true,
		ui.row {
			align = "center",
			gap = "small",
			ui.icon { name = "file-text", size = "small", class = "devtools-muted" },
			ui.text { short(file.path), class = "devtools-path", truncate = true, tooltip = file.path },
			line and ui.badge { `line {line}` },
			ui.spacer(),
			status,
		},
		-- Drawn while it reads, so the text read is a new value it takes.
		ui.input {
			id = "devtools-source",
			value = file.loaded,
			placeholder = "Reading…",
			multiline = true,
			code = "luau",
			line = line or nil,
			grow = true,
			class = "devtools-editor",
			change = "edit",
			keys = { ["secondary-s"] = "save" },
		},
		ui.row {
			align = "center",
			gap = "small",
			kbd { keys = "cmd-s", small = true },
			ui.text { "saves, and the window reloads", class = "devtools-muted" },
			ui.spacer(),
			ui.button { "Revert", small = true, variant = "ghost", msg = "revert", disabled = not file.dirty },
			ui.button { "Save", small = true, variant = "primary", msg = "save", disabled = not file.dirty },
		},
	}
end

local function inspected(state: State): any
	local seen = state.inspected
	if seen == nil then
		return ui.text { "Reading…", class = "devtools-muted" }
	end
	return ui.column {
		gap = "small",
		grow = true,
		ui.row {
			align = "center",
			gap = "small",
			ui.text { `mods/{seen.mod}`, class = "devtools-path" },
			ui.spacer(),
			ui.button { "Refresh", icon = "refresh-cw", small = true, variant = "ghost", msg = "refresh-state" },
		},
		seen.problem and ui.text { seen.problem, class = "devtools-error", wrap = true },
		seen.error and ui.text { seen.error, class = "devtools-error", wrap = true },
		seen.state ~= nil and code_view { text = pretty(seen.state), language = "lua", grow = true, id = "devtools-state" },
	}
end

local function dot(run: any): any
	local status = if run.status == "waiting" or run.status == "failed" then "blocked"
		elseif going(run) then "working"
		elseif run.status == "done" then "done"
		else "idle"
	return status_dot { status = status, spin = status == "working", size = "small" }
end

-- "Claude · 46 steps · 5 min", or why it stopped.
local function about(turns: { any }): string
	local last = turns[#turns]
	local parts = { last.agent_title or last.agent or "Agent" }
	if going(last) then
		table.insert(parts, if last.status == "waiting" then "needs you" else "working")
	elseif last.status == "failed" then
		table.insert(parts, "failed" .. if last.error then `: {first_line(last.error)}` else "")
	elseif last.status ~= "done" then
		table.insert(parts, "stopped")
	end
	local steps = 0
	for _, turn in turns do
		steps += turn.tool_count or 0
	end
	if steps > 0 then
		table.insert(parts, `{steps} {if steps == 1 then "step" else "steps"}`)
	end
	if #turns > 1 then
		table.insert(parts, `{#turns} turns`)
	end
	return table.concat(parts, " · ")
end

-- Every session, running ones first, each a click from its conversation.
local function sessions(state: State, threads: { { any } }): any
	local running, earlier = {}, {}
	for _, turns in threads do
		local id = turns[1].id
		local row = ui.column {
			class = "devtools-session",
			selected = state.session == id,
			msg = { session = id },
			gap = "none",
			ui.row {
				align = "center",
				gap = "small",
				dot(turns[#turns]),
				ui.text { first_line(turns[1].text), truncate = true, tooltip = turns[1].text },
			},
			ui.text { about(turns), class = "devtools-muted devtools-session-about", truncate = true },
		}
		table.insert(if going(turns[#turns]) then running else earlier, row)
	end
	local rows: { any } = {}
	if #running > 0 then
		table.insert(rows, ui.text { "Running", class = "devtools-group" })
		for _, row in running do
			table.insert(rows, row)
		end
	end
	if #earlier > 0 then
		table.insert(rows, ui.text { if #running > 0 then "Earlier" else "Sessions", class = "devtools-group" })
		for _, row in earlier do
			table.insert(rows, row)
		end
	end
	return ui.column { gap = "none", grow = true, scroll = true, table.unpack(rows) }
end

-- One session's turns: the request, the run with its steps folded into
-- its head, and the answer as markdown.
local function conversation(state: State, turns: { any }): { any }
	local body: { any } = {}
	for _, turn in turns do
		table.insert(body, chat_message { from = "user", text = turn.text })
		local open = state.steps[turn.id] == true
		local count = turn.tool_count or 0
		table.insert(body, agent_run { run = turn, lines = 0, tools = if open then 12 else 0 })
		if count > 0 then
			table.insert(body, ui.row {
				ui.button {
					if open then "Hide steps" else `Show steps ({count})`,
					icon = if open then "chevron-down" else "chevron-right",
					small = true,
					variant = "ghost",
					msg = { steps = turn.id },
				},
			})
		end
		if turn.message then
			table.insert(body, chat_markdown { text = turn.message, cursor = going(turn) })
		end
	end
	return body
end

local function composer_for(state: State, turns: { any }?): any
	local waiting = turns and going(turns[#turns])
	return composer {
		id = "devtools-prompt",
		value = state.prompt,
		agent = if turns then "Reply in this session" else "New session",
		icon = "sparkles",
		placeholder = if waiting
			then "The agent is working; reply when it is done…"
			elseif turns then "Not quite? Say what is still wrong…  ↩ sends"
			elseif state.pick then `What should change in {state.pick.element}?`
			else "What should change?",
		change = "prompt",
		submit = "ask",
		send = "ask",
	}
end

-- The open session: its name, Undo, the conversation and a reply field.
local function opened(state: State, turns: { any }, back: boolean): any
	local changed = false
	for _, turn in turns do
		changed = changed or (turn.changed and not turn.undone)
	end
	return ui.column {
		gap = "normal",
		grow = true,
		ui.row {
			class = "devtools-session-head",
			align = "center",
			gap = "small",
			back and ui.button { "Sessions", icon = "chevron-left", small = true, variant = "ghost", msg = "sessions" },
			ui.row { grow = true, ui.text { first_line(turns[1].text), weight = "medium", truncate = true, tooltip = turns[1].text } },
			changed and not going(turns[#turns]) and ui.button {
				"Undo",
				icon = "undo-2",
				small = true,
				variant = "ghost",
				msg = { undo_thread = turns[1].id },
				to = FEEDBACK,
				tooltip = "Undo this session's changes",
			},
		},
		ui.column { gap = "large", grow = true, scroll = true, follow = true, table.unpack(conversation(state, turns)) },
		composer_for(state, turns),
	}
end

-- Nothing open: start a session about the pick.
local function fresh(state: State, threads: { { any } }, list: boolean): any
	return ui.column {
		gap = "normal",
		grow = true,
		list and #threads > 0 and sessions(state, threads) or empty_state {
			icon = "sparkles",
			title = "Ask for a change",
			message = if state.pick
				then `About {state.pick.element}: the agent gets the pick, edits the files, and the window reloads.`
				else "Pick an element first, so the agent knows where.",
		},
		composer_for(state, nil),
	}
end

-- The Agent tab. Narrow: the list of sessions, and a session opens over
-- it with a way back. Wide: the list beside the session.
local function session(state: State, acp: any, wide: boolean): any
	if not state.agent then
		return empty_state {
			icon = "sparkles",
			title = "No agent in this app",
			message = "Agent sessions run through the feedback mod: add it with keep = [\"feedback\"] in moonframe.toml.",
		}
	elseif acp == nil then
		return empty_state { icon = "sparkles", title = "No agent here", message = "This app has no coding agent provider (acp)." }
	end
	local threads = conversations(state.runs)
	local current: { any }? = nil
	for _, turns in threads do
		if turns[1].id == state.session then
			current = turns
		end
	end
	if not wide then
		return if current then opened(state, current, true) else fresh(state, threads, true)
	end
	return ui.row {
		gap = "none",
		grow = true,
		ui.column {
			class = "devtools-sessions",
			width = 260,
			gap = "small",
			ui.row {
				align = "center",
				ui.text { `{#threads} {if #threads == 1 then "session" else "sessions"}`, class = "devtools-muted" },
				ui.spacer(),
				ui.button { "New", icon = "plus", small = true, variant = "ghost", msg = "sessions" },
			},
			sessions(state, threads),
		},
		ui.column {
			class = "devtools-conversation",
			grow = true,
			if current then opened(state, current, false) else fresh(state, threads, false),
		},
	}
end

local function body(state: State, app: any, wide: boolean): any
	if state.pick == nil and state.tab ~= "agent" then
		return unpicked()
	elseif state.tab == "agent" then
		return session(state, app.acp, wide)
	elseif state.tab == "source" then
		return source(state)
	elseif state.tab == "state" then
		return inspected(state)
	end
	return element(state)
end

return mod {
	title = "Developer",
	subscribe = { "app/selection", "app/devtools", "provider/acp" },
	window = { size = { width = 1080, height = 720 }, min_size = { width = 560, height = 400 } },

	init = function(previous: State?): State
		local state: State = previous or {
			place = "closed",
			tab = "element",
			torn = false,
			agent = true,
			steps = {},
			prompt = "",
			runs = {},
		} :: State
		-- State kept from before the steps were folded has none.
		state.steps = state.steps or {}
		return state
	end,

	update = function(state: State, msg)
		if msg == "ask" then
			-- The round button, which cannot read the field.
			return ask(state, nil)
		elseif msg == "sessions" then
			-- Back to the list, or a new session beside it.
			state.session = nil
			return state
		elseif msg == "save" then
			local file = state.source
			if file == nil or not file.dirty or file.text == nil then
				return nil
			end
			file.status = "Saving…"
			return state, { { to = APP, msg = { save = file.path, text = file.text, reply = { saved = file.path } } } }
		elseif msg == "revert" then
			local file = state.source
			if file then
				-- Read again: the editor takes the text as a new value.
				file.loaded = nil
				file.dirty = false
				return state, { { to = APP, msg = { read = file.path, reply = { source = file.path } } } }
			end
			return nil
		elseif msg == "refresh-state" then
			if state.pick then
				return state, { { to = APP, msg = { state = state.pick.mod, reply = { inspected = state.pick.mod } } } }
			end
			return nil
		elseif type(msg) ~= "table" then
			return nil
		end
		if msg.topic == "app/selection" then
			state.pick = msg.value
			return state, load(state)
		elseif msg.topic == "app/devtools" then
			state.place = msg.value and msg.value.place or "closed"
			state.agent = not (msg.value and msg.value.agent == false)
			state.torn = false
			return state
		elseif msg.topic == "provider/acp" then
			local runs = {}
			for _, run in (msg.value and msg.value.runs) or {} do
				if run.owner == FEEDBACK then
					table.insert(runs, run)
				end
			end
			state.runs = runs
			-- The session asked for has started: open it.
			local threads = conversations(runs)
			if state.awaiting and #threads > state.awaiting then
				state.session = threads[1][1].id
				state.awaiting = nil
			end
			return state
		elseif msg.msg == "grip" then
			-- Dragged out of the dock: it becomes a window.
			if msg.phase == "down" or msg.phase == "up" then
				state.torn = false
			elseif not state.torn and (math.abs(msg.dx or 0) > TEAR or math.abs(msg.dy or 0) > TEAR) then
				state.torn = true
				return state, { { to = APP, msg = { place = "window" } } }
			end
			return state
		elseif msg.place then
			return state, { { to = APP, msg = { place = msg.place } } }
		elseif msg.select then
			-- The crumb's own message: `{ select = path }`.
			return state, { { to = APP, msg = { select = msg.select } } }
		elseif msg.tab then
			state.tab = msg.tab
			return state
		elseif msg.select_tab then
			state.tab = msg.select_tab
			return state
		elseif msg.session then
			state.session = msg.session
			return state
		elseif msg.steps then
			if state.steps[msg.steps] then
				state.steps[msg.steps] = nil
			else
				state.steps[msg.steps] = true
			end
			return state
		elseif msg.msg == "edit" then
			local file = state.source
			if file then
				file.text = msg.value
				file.dirty = msg.value ~= file.loaded
				file.error = nil
			end
			return state
		elseif msg.msg == "prompt" then
			state.prompt = msg.value or ""
			return state
		elseif msg.msg == "ask" then
			return ask(state, msg.value)
		elseif msg.source then
			local file = state.source
			if file == nil or file.path ~= msg.source then
				return nil
			end
			if msg.text then
				file.loaded = msg.text
				file.text = msg.text
				file.dirty = false
				file.error = nil
				file.status = "Read"
			else
				file.error = msg.error or "cannot read it"
			end
			return state
		elseif msg.saved then
			local file = state.source
			if file == nil or file.path ~= msg.saved then
				return nil
			end
			if msg.ok then
				file.loaded = file.text
				file.dirty = false
				file.error = nil
				file.status = if msg.written and msg.written ~= file.path
					then `Saved to {short(msg.written)}; reloaded`
					else "Saved · the window reloaded"
				if state.pick then
					return state, { { to = APP, msg = { state = state.pick.mod, reply = { inspected = state.pick.mod } } } }
				end
			else
				file.error = msg.error or "cannot save it"
				file.status = nil
			end
			return state
		elseif msg.inspected then
			state.inspected = { mod = msg.inspected, state = msg.state, problem = msg.problem, error = msg.error }
			return state
		end
		return nil
	end,

	view = function(state: State, app)
		local busy = false
		for _, run in state.runs do
			busy = busy or going(run)
		end
		local list: { any } = {
			{ id = "element", label = "Element", icon = "inspector", closable = false },
			{ id = "source", label = "Source", icon = "code", closable = false, badge = state.source and state.source.dirty and "●" or nil },
			{ id = "state", label = "State", icon = "database", closable = false },
		}
		if state.agent then
			table.insert(list, { id = "agent", label = "Agent", icon = "sparkles", closable = false, status = if busy then "working" else nil })
		end
		local tab = if state.tab == "agent" and not state.agent then "element" else state.tab
		local strip = tabs { variant = "underline", selected = tab, event = "select_tab", tabs = list }
		local wide = state.place == "window"
		local main: any
		if wide and tab ~= "agent" and state.agent then
			-- A window of its own has room for the agent beside the rest.
			main = ui.row {
				gap = "large",
				grow = true,
				ui.column { grow = true, gap = "normal", body(state, app, false) },
				ui.column { class = "devtools-side", width = 380, gap = "normal", session(state, app.acp, false) },
			}
		else
			main = body(state, app, wide)
		end
		return ui.column {
			class = "devtools",
			grow = true,
			gap = "normal",
			header(state),
			target(state.pick),
			state.pick and trail(state.pick),
			strip,
			main,
		}
	end,

	styles = {
		[".devtools"] = { padding = { 10, 12, 12, 12 } },
		[".devtools-head"] = { padding = { 0, 0, 2, 0 } },
		[".devtools-grip"] = { padding = { 3, 6 }, radius = 6, color = "muted-foreground" },
		[".devtools-grip:hover"] = { background = "list-hover", color = "foreground" },
		[".devtools-path"] = { size = "small", font = "mono" },
		[".devtools-muted"] = { size = "small", color = "muted-foreground" },
		[".devtools-ok"] = { size = "small", color = "success" },
		[".devtools-warn"] = { size = "small", color = "warning" },
		[".devtools-error"] = { size = "small", color = "danger" },
		[".devtools-editor"] = { padding = 0, radius = 8 },
		[".devtools-side"] = { padding = { 0, 0, 0, 14 }, border = "left", border_color = "border" },
		[".devtools-accent"] = { color = "accent" },
		[".devtools-target"] = {
			padding = { 6, 6, 6, 10 },
			radius = 9,
			background = "accent/8",
			border = true,
			border_color = "accent/30",
		},
		[".devtools-target-none"] = { background = "#00000000", border_color = "border" },
		[".devtools-hero"] = {
			margin = { 24, 12 },
			padding = { 26, 18 },
			radius = 12,
			border = true,
			border_color = "accent/30",
			background = "accent/4",
		},
		[".devtools-hero-glyph"] = { radius = 12, background = "accent/12", color = "accent" },
		[".devtools-group"] = {
			size = "small",
			weight = "semibold",
			color = "muted-foreground",
			padding = { 8, 8, 3, 8 },
		},
		[".devtools-session"] = { padding = { 6, 8 }, radius = 7, gap = 2 },
		[".devtools-session:hover"] = { background = "list-hover" },
		[".devtools-session:selected"] = { background = "accent/10" },
		[".devtools-session-about"] = { padding = { 0, 0, 0, 14 } },
		[".devtools-session-head"] = { padding = { 0, 0, 6, 0 }, border = "bottom", border_color = "border" },
		[".devtools-sessions"] = { padding = { 0, 10, 0, 0 }, border = "right", border_color = "border" },
		[".devtools-conversation"] = { padding = { 0, 0, 0, 14 } },
	},
}
