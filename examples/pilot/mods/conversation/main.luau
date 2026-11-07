-- The middle of the window: a thread's header, its timeline and the composer.
-- A run's steps show while it goes and fold under one line afterwards, so
-- what stays is what was asked and what came of it.

local chat_markdown = use "chat-markdown"
local thinking = use "thinking"
local timeline_tool = use "timeline-tool"
local find_bar = use "find-bar"
local diff_counts = use "diff-counts"
local empty_state = use "empty-state"
local menu_button = use "menu-button"
local code_view = use "code-view"

type State = {
	runs: { [string]: boolean },
	thoughts: { [string]: boolean },
	menu: boolean,
	finding: boolean,
	query: string,
	match: number,
	found: number,
}

-- The thread the view last drew, for `update`, which has no `app`.
local latest: any = nil

local function menu_items(thread: any): { any }
	return {
		{ id = "find", label = "Find in thread", icon = "search", shortcut = "cmd-f" },
		{ id = "compact", label = "Compact context", icon = "minimize-2" },
		{ id = "pin", label = if thread.pinned then "Unpin" else "Pin", icon = "pin" },
		"separator",
		{ id = "archive", label = "Archive", icon = "archive" },
	}
end

local function age(seconds: number): string
	if seconds < 60 then
		return "now"
	elseif seconds < 3600 then
		return `{seconds // 60}m ago`
	elseif seconds < 86400 then
		return `{seconds // 3600}h ago`
	end
	return `{seconds // 86400}d ago`
end

-- The items a query finds, in order.
local function matches(items: { any }, query: string): { string }
	local found: { string } = {}
	if query == "" then
		return found
	end
	local wanted = string.lower(query)
	for _, item in items do
		if item.text and string.find(string.lower(item.text), wanted, 1, true) then
			table.insert(found, item.id)
		end
	end
	return found
end

local function user_item(item: any, thread: any, hit: boolean): any
	return ui.column {
		gap = "none",
		key = `i{item.id}`,
		ui.column {
			class = if hit then { "user-bubble", "user-bubble-hit" } else "user-bubble",
			gap = "small",
			ui.text { item.text, wrap = true, class = "user-text" },
			item.images and ui.row { align = "center", gap = "small", ui.icon { "image", class = "user-attached" }, ui.text { `{item.images} attached`, class = "user-attached" } },
		},
		item.entry and ui.row {
			class = "user-actions",
			justify = "end",
			gap = "small",
			ui.row { class = "faint-button", copy = item.text, tooltip = "Copy", ui.icon { "copy" } },
			item.entry and ui.row { class = "faint-button", msg = { fork = { thread = thread.id, entry = item.entry } }, to = "provider:pi", tooltip = "Fork from here", ui.icon { "git-branch" } },
			item.entry and thread.git and ui.row { class = "faint-button", msg = { fork = { thread = thread.id, entry = item.entry, worktree = true } }, to = "provider:pi", tooltip = "Fork into a new worktree", ui.icon { "folder-git-2" } },
		},
	}
end

local function run_item(item: any, open: boolean): any
	local foldable = item.steps > 0 and not item.running
	return ui.row {
		class = "run-line",
		align = "center",
		msg = if foldable then { fold = item.id, open = not open } else nil,
		ui.row { class = "run-rule", grow = true },
		item.running and ui.icon { "loader-circle", spin = true, class = "run-text" },
		ui.text { item.label, class = "run-text" },
		foldable and not open and item.summary ~= "" and ui.text { `· {item.summary}`, class = "run-text" },
		foldable and ui.icon { (if open then "chevron-down" else "chevron-right") :: any, class = "run-text" },
		ui.row { class = "run-rule", grow = true },
	}
end

local function changes_item(item: any): any
	local rows: { any } = {}
	for _, file in item.files do
		table.insert(rows, ui.row {
			class = "change-file",
			align = "center",
			msg = { review = { from = item.from, to = item.to, path = file.path } },
			to = "workbench",
			ui.text { file.path, truncate = true, class = "change-path" },
			ui.spacer(),
			diff_counts { added = file.added, removed = file.deleted, zero = true },
		})
	end
	return ui.column {
		class = "change-card",
		gap = "none",
		ui.row {
			class = "change-head",
			align = "center",
			msg = { review = { from = item.from, to = item.to } },
			to = "workbench",
			ui.icon { "git-compare", class = "tool-icon" },
			ui.text { `{item.count} {if item.count == 1 then "file" else "files"} changed` },
			ui.spacer(),
			diff_counts { added = item.added, removed = item.deleted, zero = true },
			ui.text { "Review", class = "change-review" },
		},
		table.unpack(rows),
	}
end

local function build_item(item: any, thread: any, state: State, hit: boolean): any
	if item.k == "user" then
		return user_item(item, thread, hit)
	elseif item.k == "text" then
		return ui.column {
			class = if hit then "text-hit" else "text-item",
			chat_markdown { text = item.text, cursor = item.streaming == true, link = "open-link" },
		}
	elseif item.k == "think" then
		return thinking {
			label = if item.streaming then "Thinking…" else "Thought",
			text = item.text,
			open = state.thoughts[item.id] == true or item.streaming == true,
			msg = { thought = item.id },
		}
	elseif item.k == "tool" then
		local reviewable = item.tool == "edit" or item.tool == "write"
		return timeline_tool { item = item, review = if reviewable and item.status == "done" then { review_tool = item.title } else nil, to = "workbench" }
	elseif item.k == "changes" then
		return changes_item(item)
	elseif item.k == "error" then
		return ui.row { class = "error-card", align = "start", ui.icon { "circle-alert", tone = "danger" }, ui.text { item.text, wrap = true } }
	elseif item.k == "bash" then
		return ui.column {
			class = "bash-card",
			gap = "none",
			ui.row { class = "bash-head", align = "center", ui.text { `$ {item.command}`, truncate = true, class = "bash-command" }, ui.spacer(), item.code ~= 0 and ui.badge { `exit {item.code}`, tone = "danger" } },
			item.output ~= "" and code_view { id = `sh-{item.id}`, text = item.output, numbers = false, height = 180, language = "text" },
		}
	end
	return ui.text { item.text, tone = "muted", size = "small", class = "note-item" }
end

return mod {
	styles = {
		[".timeline-sheet"] = { padding = { 24, 32, 24, 32 }, gap = 6 },
		[".user-bubble"] = { background = "bubble", radius = 16, padding = { 14, 20 } },
		[".user-bubble-hit"] = { border = true, border_color = "accent" },
		[".user-text"] = { size = "normal" },
		[".user-attached"] = { size = "small", color = "muted-foreground" },
		[".user-actions"] = { padding = { 4, 4, 0, 0 } },
		[".faint-button"] = { padding = 4, radius = 6, color = "muted-foreground/50" },
		[".faint-button:hover"] = { background = "list-hover", color = "foreground" },
		[".run-line"] = { padding = { 10, 0 }, gap = 10 },
		[".run-rule"] = { height = 1, background = "border" },
		[".run-text"] = { color = "muted-foreground", size = "small" },
		[".text-item"] = { padding = { 6, 0 } },
		[".text-hit"] = { padding = { 6, 8 }, radius = 8, background = "accent/10" },
		[".note-item"] = { padding = { 4, 0 } },
		[".change-card"] = { border = true, radius = 10, margin = { 8, 0 } },
		[".change-head"] = { padding = { 9, 12 }, gap = 8 },
		[".change-head:hover"] = { background = "list-hover" },
		[".change-review"] = { size = "small", color = "accent", weight = "medium" },
		[".change-file"] = { padding = { 6, 12 }, border = "top", gap = 8 },
		[".change-file:hover"] = { background = "list-hover" },
		[".change-path"] = { font = "mono", size = "small" },
		[".tool-icon"] = { color = "muted-foreground" },
		[".error-card"] = { background = "danger/10", radius = 10, padding = { 10, 12 }, gap = 8, margin = { 6, 0 } },
		[".bash-card"] = { border = true, radius = 10, margin = { 6, 0 }, clip = true },
		[".bash-head"] = { padding = { 8, 12 }, background = "secondary" },
		[".bash-command"] = { font = "mono", size = "small" },
		[".more-row"] = { padding = { 6, 0 } },
		[".find-float"] = { padding = { 8, 20 } },
	},

	init = function(previous: State?): State
		local state: State = previous or { runs = {}, thoughts = {}, menu = false, finding = false, query = "", match = 1, found = 0 }
		state.menu = false
		state.finding = false
		state.query = ""
		return state
	end,

	update = function(state: State, msg: any)
		if msg == "toggle-menu" then
			state.menu = not state.menu
		elseif msg == "close-menu" then
			state.menu = false
		elseif msg == "find" then
			state.finding = true
		elseif type(msg) ~= "table" then
			return
		elseif msg.start then
			return state, { { to = "shell", msg = { page = "new" } } }
		elseif msg.fold then
			state.runs[msg.fold] = msg.open
		elseif msg.thought then
			state.thoughts[msg.thought] = not state.thoughts[msg.thought]
		elseif msg.msg == "open-link" then
			return state, { { to = "app:open", msg = msg.url } }
		elseif msg.msg == "query" then
			state.query = msg.value
			state.match = 1
		elseif msg.find then
			if msg.find == "close" then
				state.finding = false
				state.query = ""
			else
				local count = #matches((latest and latest.items) or {}, state.query)
				if count > 0 then
					state.match = (state.match - 1 + (if msg.find == "next" then 1 else count - 1)) % count + 1
				end
			end
		elseif msg.pick then
			state.menu = false
			local thread = latest
			if not thread then
				return
			end
			if msg.pick == "compact" then
				return state, { { to = "provider:pi", msg = { prompt = "/compact" } } }
			elseif msg.pick == "pin" then
				return state, { { to = "provider:pi", msg = { pin = { id = thread.id, on = not thread.pinned } } } }
			elseif msg.pick == "archive" then
				return state, { { to = "provider:pi", msg = { archive = { id = thread.id, on = true } } } }
			elseif msg.pick == "find" then
				state.finding = true
			end
		end
	end,

	view = function(state: State, app: any)
		local pi = app.pi
		if pi and pi.available == false then
			return empty_state { icon = "circle-alert", title = "Pilot can't start", message = pi.reason }
		end
		local thread = pi and pi.thread
		latest = thread
		if pi == nil then
			return ui.column { grow = true, ui.row { drag = true, height = 52, shrink = false } }
		end
		if not thread then
			return ui.column {
				grow = true,
				ui.row { drag = true, height = 52, shrink = false },
				ui.column {
					grow = true,
					justify = "center",
					empty_state { icon = "message-square", title = "Pick a thread", message = "Or start a new one.", action = "New thread", msg = { start = true } },
				},
			}
		end

		local hits = if state.finding then matches(thread.items, state.query) else {}
		local current = hits[state.match]
		-- Runs fold once over, the last one aside.
		local last_run: string? = nil
		for _, item in thread.items do
			last_run = item.run or last_run
		end
		local rows: { any } = {}
		if thread.earlier > 0 then
			table.insert(rows, ui.row { class = "more-row", justify = "center", ui.button { `Show {thread.earlier} earlier`, variant = "ghost", small = true, msg = { show_more = true }, to = "provider:pi" } })
		end
		for _, item in thread.items do
			local run_open = true
			if item.run or item.k == "run" then
				local id = item.run or item.id
				local choice = state.runs[id]
				run_open = if choice ~= nil then choice else id == last_run
			end
			local hidden = not run_open and (item.k == "tool" or item.k == "think" or (item.k == "text" and not item.answer))
			if not hidden then
				local hit = current == item.id
				local open_thought = state.thoughts[item.id] == true
				if item.k == "run" then
					table.insert(rows, ui.memo(item.id, { item.v, run_open }, function()
						return run_item(item, run_open)
					end))
				else
					table.insert(rows, ui.memo(item.id, { item.v, hit, open_thought, thread.id }, function()
						return build_item(item, thread, state, hit)
					end))
				end
			end
		end
		if #thread.items == 0 then
			table.insert(rows, ui.text { "Ask pi to look into something.", tone = "muted" })
		end

		return ui.column {
			grow = true,
			gap = "none",
			ui.row {
				class = "pane-head",
				drag = true,
				height = 52,
				shrink = false,
				align = "center",
				gap = "small",
				ui.text { thread.workspace, class = "crumb", wrap = false },
				ui.text { "/", class = "crumb" },
				ui.text { thread.title, weight = "semibold", truncate = true },
				thread.branch and ui.row { align = "center", gap = "small", class = "scope-chip", ui.icon { "git-branch" }, ui.text { thread.branch, wrap = false } },
				ui.spacer(),
				ui.text { if thread.status == "running" then "working" else age(math.max(0, pi.now - ((thread.updated or pi.now)))), class = "crumb", wrap = false },
				menu_button {
					icon = "ellipsis",
					open = state.menu,
					toggle = "toggle-menu",
					dismiss = "close-menu",
					align = "end",
					items = menu_items(thread),
				},
			},
			state.finding and ui.row {
				class = "find-float",
				justify = "end",
				shrink = false,
				find_bar { value = state.query, count = #hits, index = if #hits > 0 then state.match else 0, focus = true, change = "query" },
			},
			ui.column {
				grow = true,
				scroll = true,
				follow = true,
				gap = "none",
				ui.column { class = "timeline-sheet", shrink = false, table.unpack(rows) },
			},
			ui.mount { "composer", id = "thread", props = {} },
		}
	end,
}
