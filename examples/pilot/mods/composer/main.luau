-- Where a prompt is written. Mounted twice: in a thread (`id = "thread"`) and
-- on the start page (`props.draft`), which also picks the project and whether
-- to work in a worktree. Enter sends, or queues a follow-up during a run;
-- ⌘Enter steers the run; @ completes files and / commands.

local dropdown = use "dropdown"
local menu_button = use "menu-button"
local segmented = use "segmented"
local file_icon = use "file-icon"

type Props = { draft: boolean? }
type State = {
	text: string,
	caret: number,
	drafts: { [string]: string },
	thread: string?,
	images: { string },
	highlighted: number,
	token: { kind: string, query: string }?,
	thinking_open: boolean,
	workspace_open: boolean,
	workspace: string?,
	mode: string,
	prefill_seq: number,
	returned_seq: number?,
	compose_seq: number,
}

local BUILTIN = {
	{ name = "compact", description = "Shrink the context, keeping a summary" },
	{ name = "name", description = "Name this thread" },
	{ name = "new", description = "Start a new thread" },
	{ name = "model", description = "Switch model" },
	{ name = "reload", description = "Restart pi for this thread" },
}
local LEVELS = { "off", "minimal", "low", "medium", "high" }

-- What the view last drew, for `update`, which has no `app`.
local suggestions: { any } = {}
local draft_folder: string? = nil

-- The `@word` or leading `/word` the caret is in.
local function token_at(text: string, caret: number): { kind: string, query: string }?
	local before = string.sub(text, 1, caret)
	local command = string.match(before, "^/(%S*)$")
	if command then
		return { kind = "command", query = command }
	end
	local file = string.match(before, "@(%S*)$")
	if file then
		local at = #before - #file - 1
		if at == 0 or string.match(string.sub(before, at, at), "%s") then
			return { kind = "file", query = file }
		end
	end
	return nil
end

local function matching_commands(query: string, commands: { any }): { any }
	local found: { any } = {}
	local wanted = string.lower(query)
	local function offer(item: any, detail: string)
		if #found < 8 and string.find(string.lower(item.name), wanted, 1, true) then
			table.insert(found, { label = `/{item.name}`, detail = detail, insert = `/{item.name} `, icon = "terminal" })
		end
	end
	for _, item in BUILTIN do
		offer(item, item.description)
	end
	for _, item in commands do
		offer(item, item.description or item.source)
	end
	return found
end

local function matching_files(query: string, found: any): { any }
	local list: { any } = {}
	if not found or found.query ~= query then
		return list
	end
	for index, path in found.results do
		if index > 8 then
			break
		end
		table.insert(list, { label = path, insert = `@{path} `, file = true })
	end
	return list
end

local function accept(state: State, choice: any, props: Props?): any
	local token = state.token
	if not token then
		return nil
	end
	local typed = (if token.kind == "file" then "@" else "/") .. token.query
	state.token = nil
	return {
		to = "app:edit",
		msg = { field = if props and props.draft then "prompt-new" else "prompt", insert = choice.insert, back = #typed, at = state.caret, expect = typed },
	}
end

local function send(state: State, value: string, behavior: string?, props: Props?): (State, { any })
	local text = string.match(value, "^%s*(.-)%s*$") or ""
	if text == "" and #state.images == 0 then
		return state, {}
	end
	local images = state.images
	state.text = ""
	state.images = {}
	state.token = nil
	if text == "/new" then
		return state, { { to = "shell", msg = { page = "new" } } }
	elseif text == "/model" then
		return state, { { to = "palette", msg = { open = "models" } } }
	end
	if props and props.draft then
		return state, {
			{ to = "provider:pi", msg = { new_thread = { workspace = state.workspace, mode = state.mode, prompt = text, images = images } } },
			{ to = "shell", msg = { page = "threads" } },
		}
	end
	return state, { { to = "provider:pi", msg = { prompt = text, images = images, behavior = behavior } } }
end

local function suggestion_list(state: State, pi: any, repo: any): { any }
	local token = state.token
	if not token then
		return {}
	end
	if token.kind == "command" then
		return matching_commands(token.query, (pi and pi.commands) or {})
	end
	return matching_files(token.query, repo and repo.found)
end

local function suggestion_rows(list: { any }, highlighted: number): any
	local rows: { any } = {}
	for index, choice in list do
		table.insert(rows, ui.row {
			class = "suggestion",
			selected = index == highlighted,
			align = "center",
			msg = { choose = index },
			if choice.file then file_icon { name = choice.label } else ui.icon { choice.icon :: any, class = "suggestion-detail" },
			ui.text { choice.label, wrap = false },
			choice.detail and ui.text { choice.detail, truncate = true, class = "suggestion-detail" },
		})
	end
	return ui.column { gap = "none", table.unpack(rows) }
end

local function queued_rows(thread: any): { any }
	local rows: { any } = {}
	local function add(list: { string }, label: string, icon: string)
		for _, text in list do
			table.insert(rows, ui.row {
				class = "queued-row",
				align = "center",
				ui.icon { icon :: any, class = "suggestion-detail" },
				ui.text { label, class = "suggestion-detail", wrap = false },
				ui.text { text, truncate = true },
			})
		end
	end
	add(thread.queued.steering, "Steering", "arrow-right")
	add(thread.queued.followUp, "Next", "list")
	if #rows > 0 then
		table.insert(rows, ui.row { justify = "end", ui.button { "Clear queue", small = true, variant = "ghost", msg = { clear_queue = true }, to = "provider:pi" } })
	end
	return rows
end

-- What an extension asks of the user, in place of a dialog.
local function question(dialog: any): any
	local answers: { any } = {}
	if dialog.kind == "select" then
		for _, option in dialog.options do
			table.insert(answers, ui.button { option, msg = { answer = { dialog = dialog.id, value = option } }, to = "provider:pi" })
		end
	elseif dialog.kind == "confirm" then
		table.insert(answers, ui.button { "Yes", variant = "primary", msg = { answer = { dialog = dialog.id, confirmed = true } }, to = "provider:pi" })
		table.insert(answers, ui.button { "No", msg = { answer = { dialog = dialog.id, confirmed = false } }, to = "provider:pi" })
	else
		table.insert(answers, ui.row {
			class = "page-field",
			grow = true,
			ui.input { id = `answer-{dialog.id}`, grow = true, focus = true, multiline = dialog.kind == "editor", value = dialog.prefill, placeholder = dialog.placeholder, submit = { answered = dialog.id } },
		})
	end
	return ui.column {
		class = "question-card",
		gap = "small",
		ui.row { align = "center", gap = "small", ui.icon { "circle-help", tone = "warning" }, ui.text { dialog.title, weight = "medium" } },
		dialog.message ~= "" and ui.text { dialog.message, tone = "muted" },
		ui.row { gap = "small", wrap = true, align = "center", table.unpack(answers) },
		ui.row { justify = "end", ui.button { "Dismiss", small = true, variant = "ghost", msg = { answer = { dialog = dialog.id, cancelled = true } }, to = "provider:pi" } },
	}
end

local function context_label(thread: any): string?
	local context = thread.context
	if not context then
		return nil
	end
	local tokens = if context.tokens >= 1000 then `{context.tokens // 1000}k` else tostring(context.tokens)
	local cost = if thread.cost > 0 then ` · ${string.format("%.2f", thread.cost)}` else ""
	if context.used then
		return `{math.floor(context.used * 100 + 0.5)}% of context{cost}`
	end
	return `{tokens} tokens{cost}`
end

return mod {
	subscribe = { "pilot/thread", "pilot/workspace", "pilot/compose" },

	styles = {
		[".composer-wrap"] = { padding = { 0, 32, 20, 32 }, gap = 8 },
		[".composer-box"] = { background = "card", border = true, radius = 14, padding = { 14, 14, 10, 18 }, gap = 10, shadow = "0 1px 2px #00000008" },
		[".prompt-input"] = { size = "normal" },
		[".chip"] = { background = "muted", radius = 7, padding = { 4, 9 }, gap = 6, size = "small" },
		[".chip:hover"] = { background = "list-active" },
		[".composer-note"] = { size = "small", color = "muted-foreground" },
		[".send"] = { width = 36, height = 34, radius = 9, background = "muted", color = "muted-foreground" },
		[".send-ready"] = { width = 36, height = 34, radius = 9, background = "ink", color = "background" },
		[".send-stop"] = { width = 36, height = 34, radius = 9, background = "danger", color = "#ffffff" },
		[".attach"] = { padding = 7, radius = 8, color = "muted-foreground" },
		[".attach:hover"] = { background = "list-hover", color = "foreground" },
		[".attached"] = { background = "muted", radius = 7, padding = { 3, 4, 3, 8 }, gap = 6, size = "small" },
		[".suggestion"] = { padding = { 6, 8 }, radius = 7, gap = 8 },
		[".suggestion:hover"] = { background = "list-hover" },
		[".suggestion:selected"] = { background = "list-active" },
		[".suggestion-detail"] = { color = "muted-foreground", size = "small" },
		[".queued-row"] = { padding = { 5, 10 }, background = "secondary", radius = 8, gap = 8 },
		[".question-card"] = { border = true, border_color = "warning/50", background = "warning/5", radius = 12, padding = { 12, 14 } },
		[".banner-row"] = { padding = { 6, 10 }, radius = 8, background = "warning/10", gap = 8, size = "small" },
		[".widget-text"] = { font = "mono", size = "small", color = "muted-foreground" },
		[".draft-tools"] = { gap = 8 },
	},

	init = function(previous: State?, _props: Props?): State
		local state: State = previous or {
			text = "",
			caret = 0,
			drafts = {},
			images = {},
			highlighted = 1,
			thinking_open = false,
			workspace_open = false,
			mode = "local",
			prefill_seq = 0,
			compose_seq = 0,
		}
		state.compose_seq = state.compose_seq or 0
		state.token = nil
		state.thinking_open = false
		state.workspace_open = false
		return state
	end,

	update = function(state: State, msg: any, props: Props?)
		if msg == "dismiss" then
			state.token = nil
		elseif msg == "toggle-thinking" then
			state.thinking_open = not state.thinking_open
		elseif msg == "close-thinking" then
			state.thinking_open = false
		elseif msg == "toggle-workspace" then
			state.workspace_open = not state.workspace_open
		elseif msg == "close-workspace" then
			state.workspace_open = false
		elseif msg == "attach" then
			return state, { { to = "app:pick", msg = { kind = "open", multiple = true, types = { "png", "jpg", "jpeg", "gif", "webp" }, reply = { picked = true } } } }
		elseif msg == "stop" then
			return state, { { to = "provider:pi", msg = { abort = true } } }
		elseif msg == "send-click" then
			return send(state, state.text, "follow_up", props)
		elseif msg == "add-folder" then
			return state, { { to = "sidebar", msg = "add-folder" } }
		elseif type(msg) ~= "table" then
			return
		elseif msg.topic == "pilot/workspace" then
			state.workspace = msg.value
		elseif msg.topic == "pilot/compose" then
			-- Skills' Try: text for whichever composer shows next.
			if msg.value and msg.value.seq ~= state.compose_seq then
				state.compose_seq = msg.value.seq
				state.text = msg.value.text
			end
		elseif msg.topic == "pilot/thread" then
			local thread = msg.value
			if not thread or (props and props.draft) then
				return
			end
			if thread.id ~= state.thread then
				if state.thread then
					state.drafts[state.thread] = state.text
				end
				state.thread = thread.id
				state.text = (thread.id and state.drafts[thread.id]) or ""
				state.images = {}
				state.token = nil
			end
			-- A fork puts its prompt back, and an extension may write here.
			if thread.text and thread.seq ~= state.prefill_seq then
				state.prefill_seq = thread.seq
				state.text = thread.text
			end
			-- A failed send's words, beside anything typed since.
			local back = thread.returned
			if back and back.seq ~= state.returned_seq then
				state.returned_seq = back.seq
				state.text = if state.text == "" then back.text else `{state.text}\n\n{back.text}`
			end
		elseif msg.msg == "type" then
			state.text = msg.value
			state.caret = msg.caret or #msg.value
			local token = token_at(state.text, state.caret)
			state.token = token
			state.highlighted = 1
			if token and token.kind == "file" then
				local root = if props and props.draft then draft_folder else nil
				return state, { { to = "provider:repo", msg = { find = token.query, root = root } } }
			end
		elseif msg.msg == "enter" then
			state.text = msg.value
			return send(state, msg.value, "follow_up", props)
		elseif msg.msg == "steer" then
			state.text = msg.value
			return send(state, msg.value, "steer", props)
		elseif msg.msg == "move" then
			local count = math.max(#suggestions, 1)
			state.highlighted = (state.highlighted - 1 + msg.by + count) % count + 1
		elseif msg.msg == "accept" then
			local edit = accept(state, suggestions[state.highlighted], props)
			return state, { edit }
		elseif msg.msg == "dismiss" then
			state.token = nil
		elseif msg.choose then
			local edit = accept(state, suggestions[msg.choose], props)
			return state, { edit }
		elseif msg.answered then
			return state, { { to = "provider:pi", msg = { answer = { dialog = msg.answered, value = msg.value } } } }
		elseif msg.picked or msg.dropped then
			for _, path in msg.paths or {} do
				table.insert(state.images, path)
			end
		elseif msg.unattach then
			table.remove(state.images, msg.unattach)
		elseif msg.pick and (state.thinking_open) then
			state.thinking_open = false
			return state, { { to = "provider:pi", msg = { set_thinking = msg.pick, draft = props ~= nil and props.draft == true } } }
		elseif msg.pick then
			state.workspace_open = false
			if msg.pick == "add" then
				return state, { { to = "sidebar", msg = "add-folder" } }
			end
			state.workspace = msg.pick
		elseif msg.mode then
			state.mode = msg.mode
		end
	end,

	view = function(state: State, app: any, props: Props?)
		if props == nil then
			return ui.column { width = 0, height = 0 }
		end
		local pi = app.pi
		local draft = props ~= nil and props.draft == true
		local thread = if draft then nil else (pi and pi.thread)
		suggestions = suggestion_list(state, pi, app.repo)
		local running = thread ~= nil and thread.running == true
		local has_text = state.text ~= "" or #state.images > 0
		local open = #suggestions > 0
		local keys: { [string]: any } = { ["secondary-enter"] = "steer" }
		if open then
			keys.up = { msg = "move", by = -1 }
			keys.down = { msg = "move", by = 1 }
			keys.enter = "accept"
			keys.tab = "accept"
			keys.escape = "dismiss"
		else
			keys.enter = "enter"
		end

		local attached: { any } = {}
		for index, path in state.images do
			table.insert(attached, ui.row {
				class = "attached",
				align = "center",
				ui.icon { "image" },
				ui.text { string.match(path, "[^/]+$") or path, truncate = true },
				ui.row { class = "quiet-button", msg = { unattach = index }, ui.icon { "close" } },
			})
		end

		local model = (thread and thread.model) or (pi and pi.prefs.last_model)
		local model_label = if model then (model.name or model.id) else "Choose a model"
		for _, known in (pi and pi.models) or {} do
			if model and known.provider == model.provider and known.id == model.id then
				model_label = known.name
			end
		end
		local level = (thread and thread.thinking) or (pi and pi.prefs.last_thinking) or "medium"
		local levels = if thread and #thread.levels > 0 then thread.levels else LEVELS
		local level_options: { any } = {}
		for _, name in levels do
			table.insert(level_options, { id = name, label = name, checked = name == level })
		end

		local first_workspace = pi and (pi.workspaces[1] or pi.chats)
		local workspace_path = state.workspace or (first_workspace and first_workspace.path)
		draft_folder = workspace_path
		local workspace_options: { any } = {}
		local workspace_entry: any = nil
		for _, workspace in (pi and pi.workspaces) or {} do
			table.insert(workspace_options, { id = workspace.path, label = workspace.name, detail = workspace.short, icon = "folder" })
			if workspace.path == workspace_path then
				workspace_entry = workspace
			end
		end
		if pi then
			table.insert(workspace_options, { id = pi.chats.path, label = pi.chats.name, detail = "Pilot's own empty folder", icon = "message-square" })
			if pi.chats.path == workspace_path then
				workspace_entry = pi.chats
			end
		end
		table.insert(workspace_options, "separator")
		table.insert(workspace_options, { id = "add", label = "Add a project folder…", icon = "folder-open" })

		local placeholder = if draft then "What should pi work on?"
			elseif running then "Queue a follow-up, or press ⌘↩ to steer…"
			else "Ask pi to inspect the repo, run a fix, or continue this thread…"

		-- The box takes the width, so the suggestions hang from a mark above it:
		-- a popover's anchor is only as wide as its content.
		local suggestions_anchor = ui.popover {
			open = open,
			dismiss = "dismiss",
			side = "top",
			align = "start",
			width = 440,
			anchor_clicks = true,
			content = suggestion_rows(suggestions, state.highlighted),
			ui.row { height = 1, width = 1 },
		}
		local box = ui.column {
				class = "composer-box",
				drop = { dropped = true },
				#attached > 0 and ui.row { gap = "small", wrap = true, table.unpack(attached) },
				ui.input {
					id = if draft then "prompt-new" else "prompt",
					class = "prompt-input",
					multiline = true,
					rows = 2,
					max_rows = 12,
					focus = true,
					value = state.text,
					placeholder = placeholder,
					change = "type",
					select = "type",
					keys = keys,
				},
				ui.row {
					align = "center",
					gap = "small",
					ui.row { class = "chip", align = "center", msg = { open = "models" }, to = "palette", tooltip = "Switch model", ui.text { model_label, wrap = false } },
					menu_button {
						class = "chip",
						label = level,
						open = state.thinking_open,
						toggle = "toggle-thinking",
						dismiss = "close-thinking",
						side = "top",
						width = 150,
						tooltip = "Thinking level",
						items = level_options,
					},
					thread and context_label(thread) and ui.text { context_label(thread) or "", class = "composer-note", wrap = false },
					ui.spacer(),
					ui.row { class = "attach", msg = "attach", tooltip = "Attach pictures", ui.icon { "plus" } },
					if running and not has_text
						then ui.row { class = "send-stop", align = "center", justify = "center", msg = "stop", tooltip = "Stop", ui.icon { "square" } }
						else ui.row {
							class = if has_text then "send-ready" else "send",
							align = "center",
							justify = "center",
							msg = "send-click",
							tooltip = if running then "Queue · ⌘↩ steers" else "Send",
							ui.icon { "arrow-up" },
						},
				},
		}

		local above: { any } = {}
		if draft then
			table.insert(above, ui.row {
				class = "draft-tools",
				align = "center",
				dropdown {
					icon = "folder",
					value = if workspace_entry then workspace_entry.name else "Choose a project",
					open = state.workspace_open,
					toggle = "toggle-workspace",
					dismiss = "close-workspace",
					selected = workspace_path,
					options = workspace_options,
					width = 320,
				},
				workspace_entry and workspace_entry.branch and ui.text { workspace_entry.branch, class = "composer-note" },
				ui.spacer(),
				workspace_entry and workspace_entry.git and segmented {
					selected = state.mode,
					event = "mode",
					options = { { id = "local", label = "Local" }, { id = "worktree", label = "Worktree" } },
				},
			})
		end
		if thread then
			if thread.retry then
				table.insert(above, ui.row { class = "banner-row", align = "center", ui.icon { "refresh-cw", spin = true }, ui.text { `Retrying ({thread.retry.attempt} of {thread.retry.max}): {thread.retry.error}`, truncate = true } })
			end
			if thread.compacting then
				table.insert(above, ui.row { class = "banner-row", align = "center", ui.icon { "loader-circle", spin = true }, ui.text { "Compacting the context…" } })
			end
			if thread.dialog then
				table.insert(above, question(thread.dialog))
			end
			for _, row in queued_rows(thread) do
				table.insert(above, row)
			end
			for _, line in thread.widgets do
				table.insert(above, ui.text { line, class = "widget-text" })
			end
			if #thread.statuses > 0 then
				table.insert(above, ui.text { table.concat(thread.statuses, " · "), class = "composer-note" })
			end
		end
		table.insert(above, suggestions_anchor)
		table.insert(above, box)
		return ui.column { class = "composer-wrap", shrink = false, table.unpack(above) }
	end,
}
