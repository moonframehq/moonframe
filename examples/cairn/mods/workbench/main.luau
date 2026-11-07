-- The card on the right: tabs for the session's changes, the Hello mod, Echo
-- (which talks to Hello through the greeter service), documents the chat
-- links to, and terminals the + opens.

local tabs = use "tabs"
local menu = use "menu"
local icon_button = use "icon-button"
local change_tree = use "change-tree"
local diff_view = use "diff-view"
local empty_state = use "empty-state"
local markdown = use "markdown"
local terminal = use "terminal"

type State = {
	tab: string,
	closed: { [string]: boolean },
	current: string?,
	picked: string?,
	folded: { [string]: boolean },
	collapsed: { [string]: boolean },
	greeter: any,
	name: string,
	doc: string?,
	shell: boolean,
	adding: boolean,
}

-- Turns a unified diff's text into `diff-view`'s hunks.
local function parse_diff(text: string): { any }
	local hunks: { any } = {}
	local hunk: any = nil
	local old, new = 0, 0
	for line in string.gmatch(text .. "\n", "(.-)\n") do
		local first = string.sub(line, 1, 1)
		if first == "@" then
			old = tonumber(string.match(line, "%-(%d+)")) or 0
			new = tonumber(string.match(line, "%+(%d+)")) or 0
			hunk = { header = line, lines = {} }
			table.insert(hunks, hunk)
		elseif hunk and first == "+" then
			table.insert(hunk.lines, { kind = "add", new = new, text = string.sub(line, 2) })
			new += 1
		elseif hunk and first == "-" then
			table.insert(hunk.lines, { kind = "del", old = old, text = string.sub(line, 2) })
			old += 1
		elseif hunk then
			table.insert(hunk.lines, { kind = "ctx", old = old, new = new, text = string.sub(line, 2) })
			old += 1
			new += 1
		end
	end
	return hunks
end

-- Asks the greeter service, through the Hello mod.
local function ask(state: State)
	local name = string.match(state.name, "^%s*(.-)%s*$") or ""
	local sends: { any } = {}
	if name ~= "" then
		table.insert(sends, { to = "hello", msg = { greet = name } })
	end
	return state, sends
end

local function session_files(app, id: string?): { any }
	for _, session: any in (app.sessions and app.sessions.sessions) or {} do
		if session.id == id then
			return session.files
		end
	end
	return {}
end

local function files_tab(state: State, files: { any })
	if #files == 0 then
		local message = if state.current then "This session has not touched a file." else "Open a session to see its changes."
		return empty_state { icon = "list-tree", title = "No changes", message = message }
	end
	local lines = {}
	local open = files[1]
	for _, file in files do
		table.insert(lines, `{file.status}\t{file.added}\t{file.deleted}\t{file.path}`)
		if file.path == state.picked then
			open = file
		end
	end
	return ui.column { grow = true, gap = "none",
		ui.column { class = "bench-tree", shrink = false,
			change_tree { files = lines, selected = open.path, counts = true, plain = true, folded = state.folded, pick = "file" },
		},
		ui.column { class = "bench-diff", grow = true,
			diff_view { path = open.path, status = open.status, hunks = parse_diff(open.diff), collapsed = state.collapsed[open.path], grow = true },
		},
	}
end

local function echo_tab(state: State)
	local greeter = state.greeter or { on = false, generation = 0, count = 0, answer = "", words = 0 }
	local status = if greeter.on then `active, generation {greeter.generation}` else "stopped"
	return ui.column { class = "echo", grow = true, scroll = true,
		ui.row { align = "center", gap = "normal",
			ui.icon { "wifi", size = "large" },
			ui.text { "Echo", class = "echo-title" },
		},
		ui.row { align = "center", gap = "normal",
			ui.row { class = if greeter.on then "echo-on" else "echo-off", width = 9, height = 9, shrink = false },
			ui.text { `Greeter service: {status}`, class = "echo-status" },
		},
		ui.column { class = "echo-card",
			ui.text { "Hello's answer", class = "echo-label" },
			ui.text { greeter.answer, class = "echo-answer" },
			ui.text { `{greeter.words} words, counted by the Hello mod`, tone = "muted", size = "small" },
			ui.text { `Hello's counter, read through the service: {greeter.count}`, tone = "muted", size = "small" },
		},
		ui.row { align = "center", gap = "normal",
			ui.input { id = "echo-name", class = "field", value = state.name, placeholder = "A name", grow = true, change = "name", submit = "ask" },
			ui.button { "Ask Hello", variant = "outline", msg = "ask", disabled = not greeter.on },
		},
	}
end

return mod {
	subscribe = { "cairn/sessions", "cairn/greeter" },

	styles = {
		[".bench-head"] = { padding = { 7, 10, 7, 12 } },
		[".bench-tree"] = { padding = { 4, 8, 10, 8 }, border = "bottom", border_color = "border" },
		[".bench-diff"] = { padding = { 10, 10 } },
		[".bench-doc"] = { padding = { 16, 20 } },
		[".echo"] = { padding = { 16, 18 }, gap = 16 },
		[".echo-title"] = { size = "large", weight = "semibold" },
		[".echo-status"] = { color = "#c8c8c8" },
		[".echo-on"] = { background = "success", radius = 999 },
		[".echo-off"] = { background = "muted-foreground", radius = 999 },
		[".echo-card"] = { background = "raised", radius = 10, padding = { 16, 18 }, gap = 8 },
		[".echo-label"] = { color = "#b4b4b4" },
		[".echo-answer"] = { size = "large", weight = "semibold" },
		button = { radius = 6 },
	},

	init = function(previous: State?): State
		local state = previous or {
			tab = "echo", closed = {}, folded = {}, name = "Kit", shell = false,
		} :: any
		state.adding = false
		state.collapsed = state.collapsed or {}
		return state
	end,

	update = function(state: State, msg)
		if msg == "add" then
			state.adding = not state.adding
		elseif msg == "add-close" then
			state.adding = false
		elseif msg == "ask" then
			return ask(state)
		elseif type(msg) ~= "table" then
			return
		elseif msg.msg == "name" then
			state.name = msg.value
		elseif msg.msg == "ask" then
			state.name = msg.value
			return ask(state)
		elseif msg.topic == "cairn/greeter" then
			state.greeter = msg.value
		elseif msg.topic == "cairn/sessions" and msg.value then
			if state.current ~= msg.value.current then
				state.current = msg.value.current
				state.picked = nil
			end
		elseif msg.select or msg.tab then
			state.tab = msg.select or msg.tab
			state.closed[state.tab] = nil
			return state, { { to = "shell", msg = { show_right = true } } }
		elseif msg.file then
			state.picked = msg.file
		elseif msg.reopen then
			state.closed[msg.reopen] = nil
			state.tab = msg.reopen
			state.adding = false
		elseif msg.open_shell then
			state.shell = true
			state.tab = "terminal"
			state.adding = false
		elseif msg.toggle then
			state.collapsed[msg.toggle] = not state.collapsed[msg.toggle]
		elseif msg.fold then
			state.folded[msg.fold] = not state.folded[msg.fold]
		elseif msg.open_doc then
			state.doc = msg.open_doc
			state.tab = "doc"
			return state, { { to = "shell", msg = { show_right = true } } }
		elseif msg.close then
			if msg.close == "doc" then
				state.doc = nil
			elseif msg.close == "terminal" then
				state.shell = false
			else
				state.closed[msg.close] = true
			end
			if state.tab == msg.close then
				state.tab = "files"
			end
		end
	end,

	view = function(state: State, app)
		local files = session_files(app, state.current)
		-- With a document and a terminal open, the strip needs the room.
		local crowded = state.doc ~= nil and state.shell
		local strip: { any } = { { id = "files", label = if crowded then "Files" else "Files Changed", icon = "list-tree", badge = if #files > 0 then #files else nil, closable = false } }
		local add_items: { any } = {}
		for _, entry in { { "hello", "Hello", "smile-plus" }, { "echo", "Echo", "wifi" } } do
			if state.closed[entry[1]] then
				table.insert(add_items, { id = entry[1], label = entry[2], icon = entry[3], msg = { reopen = entry[1] } })
			else
				table.insert(strip, { id = entry[1], label = entry[2], icon = entry[3] })
			end
		end
		if state.doc then
			table.insert(strip, { id = "doc", label = string.match(state.doc, "[^/]+$"), icon = "file-text" })
		end
		if state.shell then
			table.insert(strip, { id = "terminal", label = "Terminal", icon = "terminal" })
		else
			table.insert(add_items, { id = "terminal", label = "Terminal", icon = "terminal", msg = { open_shell = true } })
		end

		if crowded then
			for _, tab in strip do
				tab.icon = nil
			end
		end

		local body
		if state.tab == "hello" then
			body = ui.mount "hello"
		elseif state.tab == "echo" then
			body = echo_tab(state)
		elseif state.tab == "doc" and state.doc then
			local docs = (app.sessions and app.sessions.docs) or {}
			body = ui.column { class = "bench-doc", grow = true,
				markdown { text = docs[state.doc] or "Not in this sample.", grow = true, scroll = true },
			}
		elseif state.tab == "terminal" then
			body = ui.column { grow = true, terminal { id = "bench", cwd = "~" } }
		else
			body = files_tab(state, files)
		end

		return ui.column { grow = true, gap = "none",
			ui.row { class = "bench-head", align = "center", shrink = false,
				ui.row { grow = true, width = 0, scroll = "x", gap = "none",
					ui.row { shrink = false, gap = "none",
						tabs {
							variant = "pills", selected = state.tab, tabs = strip,
							add = if #add_items > 0 then "add" else nil,
							add_menu = { open = state.adding, dismiss = "add-close", width = 180, content = menu { items = add_items } },
						},
					},
				},
				icon_button { icon = "panel-right", tooltip = "Hide the workbench", msg = "toggle-right", to = "shell" },
			},
			body,
		}
	end,
}
