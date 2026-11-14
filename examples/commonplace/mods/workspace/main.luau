-- The Workspace layout: the vault as a tree (or search results), notes in
-- tabs, the page (read and edited in place), and the wiki beside it: what its log
-- says the agent did, and what needs care (links to nowhere, pages
-- nothing links to). A bar across the top holds the search. All from the
-- notes provider's live views.

local note_page = use "note-page"
local icon_button = use "icon-button"

type State = {
	query: string,
	tabs: { string },
	open: string?,
	-- An enhance of the open note running, and what the last one said.
	enhancing: boolean?,
	notice: string?,
	folded: { [string]: boolean? },
	views: { [string]: any },
}

-- The open note's views: its text, and what links to it.
local function note_watches(state: State): { any }
	if not state.open then
		return {}
	end
	return {
		{ to = "provider:notes", msg = { watch = { id = "note", kind = "note", path = state.open } } },
		{ to = "provider:notes", msg = { watch = { id = "context", kind = "context", path = state.open } } },
	}
end

-- Every view this state needs; `again` asks for answers already sent.
local function watches(state: State, again: boolean?): { any }
	local sends: { any } = {
		{ to = "provider:notes", msg = { watch = { id = "all", kind = "list", folder = "", order = "title", limit = 500 } } },
		{ to = "provider:notes", msg = { watch = { id = "health", kind = "health", folder = "wiki", limit = 12 } } },
		{ to = "provider:notes", msg = { watch = { id = "log", kind = "note", path = "log.md" } } },
	}
	if state.query ~= "" then
		table.insert(sends, { to = "provider:notes", msg = { watch = { id = "search", kind = "search", q = state.query, limit = 60 } } })
	end
	for _, send in note_watches(state) do
		table.insert(sends, send)
	end
	if again then
		for _, send in sends do
			send.msg.again = true
		end
	end
	return sends
end

local function open(state: State, path: string)
	if not table.find(state.tabs, path) then
		table.insert(state.tabs, path)
		while #state.tabs > 8 do
			table.remove(state.tabs, 1)
		end
	end
	state.open = path
	state.notice = nil
	state.views.note = nil
	state.views.context = nil
	local sends = note_watches(state)
	-- The agent chat works on the note in view.
	table.insert(sends, { to = "topic:commonplace/open", msg = { path = path } })
	return state, sends
end

local function stem(path: string): string
	return (string.gsub(string.match(path, "([^/]+)$") or path, "%.md$", ""))
end

local function topbar(state: State)
	return ui.row {
		class = "topbar",
		height = 52,
		shrink = false,
		align = "center",
		drag = true,
		ui.row { width = 64 },
		icon_button { icon = "layers", tooltip = "Library layout (⌥⌘1)", size = "small", to = "shell", msg = { layout = "library" } },
		icon_button { icon = "sun", tooltip = "Journal layout (⌥⌘2)", size = "small", to = "shell", msg = { layout = "journal" } },
		ui.spacer(),
		ui.row {
			class = "search-box",
			width = 460,
			align = "center",
			ui.icon { "search", class = "muted" },
			ui.input { id = "workspace-search", value = state.query, placeholder = "Search the vault… (tag:, path:, type:)", change = { query = true }, keys = { escape = { query = true, clear = true } }, grow = true, class = "bare-input" },
		},
		ui.spacer(),
		icon_button { icon = "square-pen", tooltip = "New note", size = "small", msg = "new" },
	}
end

-- The vault's notes as folders and files, from the `all` view: built as
-- plain rows, then drawn a screenful at a time.
local TREE_ROW = 28
local HIT_ROW = 72

-- A search snippet on one line: match marks and line breaks gone.
local function snippet(text: string?): string
	local out = string.gsub(text or "", "[\1\2]", "")
	out = string.gsub(out, "%s+", " ")
	return out
end

local function tree_rows(state: State): { any }
	local rows: { any } = {}
	local seen: { [string]: boolean } = {}
	local files: { any } = table.clone((state.views.all or {}).rows or {})
	table.sort(files, function(a, b)
		return a.path < b.path
	end)
	for _, file in files do
		local parts = string.split(file.path, "/")
		local prefix = ""
		local hidden = false
		for depth = 1, #parts - 1 do
			prefix = if prefix == "" then parts[depth] else `{prefix}/{parts[depth]}`
			if not seen[prefix] and not hidden then
				seen[prefix] = true
				table.insert(rows, { folder = prefix, name = parts[depth], depth = depth - 1, folded = state.folded[prefix] == true })
			end
			hidden = hidden or state.folded[prefix] == true
		end
		if not hidden then
			table.insert(rows, { path = file.path, name = file.title, depth = #parts - 1 })
		end
	end
	return rows
end

local function tree(state: State)
	if state.query ~= "" then
		local hits: { any } = (state.views.search or {}).rows or {}
		return ui.column {
			grow = true,
			gap = "none",
			ui.text { "Results", class = "nav-heading" },
			#hits == 0 and ui.column { padding = "large", ui.text { "Nothing found.", class = "muted small" } },
			#hits > 0 and ui.list {
				id = "hits",
				count = #hits,
				row_height = HIT_ROW,
				grow = true,
				row = function(i)
					local hit = hits[i]
					return ui.column {
						class = "note-row",
						selected = hit.path == state.open,
						msg = { open = hit.path },
						height = HIT_ROW - 4,
						ui.text { hit.title, class = "note-row-title", truncate = true },
						ui.text { snippet(hit.snippet), class = "note-row-excerpt", truncate = true },
					}
				end,
			},
		}
	end
	local rows = tree_rows(state)
	return ui.column {
		grow = true,
		gap = "none",
		ui.text { "Vault", class = "nav-heading" },
		ui.list {
			id = "tree",
			count = #rows,
			row_height = TREE_ROW,
			grow = true,
			row = function(i)
				local row = rows[i]
				local indent = if row.depth > 0 then ui.row { style = { width = row.depth * 14 } } else nil
				if row.folder then
					return ui.row {
						class = "nav-row",
						align = "center",
						height = TREE_ROW - 2,
						msg = { fold = row.folder },
						indent,
						ui.icon { if row.folded then "folder" else "folder-open", class = "nav-icon" },
						ui.row { grow = true, gap = "none", ui.text { row.name, truncate = true } },
					}
				end
				return ui.row {
					class = "nav-row",
					selected = row.path == state.open,
					align = "center",
					height = TREE_ROW - 2,
					msg = { open = row.path },
					indent,
					ui.icon { "file-text", class = "nav-icon" },
					ui.row { grow = true, gap = "none", ui.text { row.name, truncate = true } },
				}
			end,
		},
	}
end

local function editor(state: State)
	local tabs = {}
	for _, path in state.tabs do
		table.insert(
			tabs,
			ui.row {
				class = "note-row",
				selected = path == state.open,
				align = "center",
				gap = "small",
				style = { margin = 0, padding = { 6, 10 } },
				msg = { open = path },
				ui.icon { "file-text", class = "muted" },
				ui.text { stem(path), class = "small", truncate = true },
				ui.row { msg = { close = path }, ui.icon { "close", class = "muted" } },
			}
		)
	end
	local note = state.views.note
	local context = state.views.context or {}
	local info = context.info or {}
	return ui.column {
		class = "card",
		grow = true,
		gap = "none",
		ui.row {
			class = "card-head",
			gap = "small",
			align = "center",
			ui.row { grow = true, gap = "small", scroll = "x", table.unpack(tabs) },
			state.notice and ui.text { state.notice, class = "muted small", truncate = true },
			state.open and ui.row {
				class = "head-action",
				align = "center",
				msg = if state.enhancing then nil else "enhance",
				tooltip = "Fetch page titles and site icons for this note's links",
				ui.icon { name = if state.enhancing then "loader-circle" else "sparkles", spin = state.enhancing == true, size = "small" },
				ui.text { if state.enhancing then "Enhancing…" else "Enhance", class = "small" },
			},
		},
		if note and note.text
			then ui.column { grow = true, scroll = true, align = "center", note_page { id = "workspace", note = note, width = 640 } }
			else ui.column { grow = true, align = "center", justify = "center", ui.text { if state.open then "Opening…" else "Open a note from the tree.", class = "muted" } },
		state.open and ui.row {
			padding = "small",
			border = "top",
			align = "center",
			gap = "normal",
			ui.text { `{#(context.backlinks or {})} backlinks`, class = "muted small" },
			ui.text { `{#(context.mentions or {})} unlinked mentions`, class = "muted small" },
			ui.spacer(),
			ui.text { `{info.words or 0} words`, class = "muted small" },
		},
	}
end

-- log.md's entries, `## [YYYY-MM-DD] op | title`, newest first.
local function log_entries(text: string): { any }
	local out: { any } = {}
	for day, op, title in string.gmatch(text, "## %[(%d%d%d%d%-%d%d%-%d%d)%] (%a+) | ([^\n]*)") do
		table.insert(out, 1, { day = day, op = op, title = title })
	end
	return out
end

local function wiki(state: State)
	local health = state.views.health or {}
	local log = log_entries((state.views.log or {}).text or "")
	local entries = {}
	for index, entry in log do
		if index > 8 then
			break
		end
		table.insert(
			entries,
			ui.row {
				gap = "small",
				ui.text { string.sub(entry.day, 6), class = "muted small", style = { width = 40 } },
				ui.text { entry.op, class = "small", style = { width = 46, color = "accent" } },
				ui.row { grow = true, gap = "none", ui.text { entry.title, class = "small", truncate = true } },
			}
		)
	end
	local dangling = {}
	for _, link in health.unresolved or {} do
		table.insert(
			dangling,
			ui.row {
				class = "ref",
				align = "center",
				msg = { create = link.target },
				ui.row { grow = true, gap = "none", ui.text { link.target, class = "ref-title", truncate = true } },
				ui.text { `{link.count}×`, class = "muted small" },
			}
		)
	end
	local orphans = {}
	for _, page in health.orphans or {} do
		table.insert(orphans, ui.row { class = "ref", msg = { open = page.path }, ui.text { page.title, class = "small", truncate = true } })
	end
	return ui.column {
		class = "card",
		grow = true,
		gap = "none",
		ui.row {
			class = "card-head",
			align = "center",
			ui.icon { "book-open" },
			ui.row { grow = true, gap = "none", ui.text { "Wiki", class = "card-title" } },
		},
		ui.column {
			grow = true,
			scroll = true,
			padding = "large",
			gap = "large",
			ui.column { gap = "small", ui.text { "log.md", class = "section-label" }, #entries == 0 and ui.text { "The agent has not run yet.", class = "muted small" }, table.unpack(entries) },
			ui.column { gap = "small", ui.text { "Links to pages that don't exist", class = "section-label" }, #dangling == 0 and ui.text { "None.", class = "muted small" }, table.unpack(dangling) },
			ui.column { gap = "small", ui.text { "Wiki pages nothing links to", class = "section-label" }, #orphans == 0 and ui.text { "None.", class = "muted small" }, table.unpack(orphans) },
		},
		ui.column {
			padding = "normal",
			gap = "small",
			border = "top",
			ui.text { "Ingest, ask and lint with an agent come next.", class = "muted small", wrap = true },
		},
	}
end

return mod {
	title = "Workspace",

	init = function(previous: State?): (State, { any })
		local state: State = previous or { query = "", tabs = {}, folded = {}, views = {} }
		state.views = {}
		state.enhancing = nil
		state.notice = nil
		return state, watches(state, true)
	end,

	update = function(state: State, msg)
		if msg == "new" then
			return state, { { to = "provider:notes", msg = { create = { path = "notes/Untitled.md", text = "", unique = true }, id = "new" } } }
		elseif msg == "enhance" and state.open then
			state.enhancing = true
			state.notice = nil
			return state, { { to = "provider:notes", msg = { enhance = { path = state.open }, id = "enhance" } } }
		end
		if type(msg) ~= "table" then
			return
		end
		if msg.view then
			local data = msg.data
			local current = state.views[msg.view]
			if msg.view == "note" and data and data.own and current then
				data.text = current.text
			end
			state.views[msg.view] = data
			-- Nothing open yet: start on the note changed last.
			if msg.view == "all" and not state.open and data and data.rows then
				local newest
				for _, row in data.rows do
					if not newest or (row.mtime or 0) > (newest.mtime or 0) then
						newest = row
					end
				end
				if newest then
					return open(state, newest.path)
				end
			end
			if msg.view == "resolve" and data then
				-- Asked once: stop it, or later changes would open it again.
				local stop: any = { to = "provider:notes", msg = { unwatch = "resolve" } }
				if data.path then
					local _, sends = open(state, data.path)
					table.insert(sends, stop)
					return state, sends
				end
				local create: any = { to = "provider:notes", msg = { create = { path = `notes/{data.target}.md`, text = "" }, id = "create-link" } }
				return state, { stop, create }
			end
			return
		end
		if msg.reply ~= nil then
			if msg.action == "create" and msg.ok and msg.path then
				return open(state, msg.path)
			elseif msg.action == "enhance" then
				state.enhancing = nil
				state.notice = if msg.ok
					then `{msg.titled or 0} titles, {msg.icons or 0} icons`
					else msg.error or "Enhance failed"
			elseif msg.action == "save" and msg.outcome == "merged" and state.views.note then
				state.views.note.text = msg.text
			end
			return
		end
		if msg.open then
			return open(state, msg.open)
		elseif msg.close then
			local at = table.find(state.tabs, msg.close)
			if at then
				table.remove(state.tabs, at)
			end
			if state.open == msg.close then
				state.open = state.tabs[math.max(1, (at or 1) - 1)]
				state.views.note = nil
				return state, watches(state)
			end
		elseif msg.fold then
			state.folded[msg.fold] = not state.folded[msg.fold] or nil
		elseif msg.query then
			state.query = if msg.clear then "" else msg.value or ""
			return state, watches(state)
		elseif msg.create then
			return state, { { to = "provider:notes", msg = { create = { path = `wiki/{msg.create}.md`, text = "" }, id = "create-link" } } }
		elseif msg.edit and state.open then
			if state.views.note then
				state.views.note.text = msg.value
			end
			return state, { { to = "provider:notes", msg = { save = { path = state.open, text = msg.value }, id = "save" } } }
		elseif msg.msg == "link" and type(msg.url) == "string" then
			local target = string.match(msg.url, "^note:(.*)$")
			if target then
				return state, { { to = "provider:notes", msg = { watch = { id = "resolve", kind = "resolve", target = target, from = state.open or "" } } } }
			end
			return state, { { to = "app:open", msg = msg.url } }
		end
	end,

	view = function(state: State, app)
		local notes = app.notes
		if not notes or not notes.available then
			return ui.column { grow = true, align = "center", justify = "center", ui.text { (notes and notes.reason) or "Opening the vault…", class = "muted" } }
		end
		return ui.column {
			grow = true,
			gap = "none",
			topbar(state),
			ui.hsplit {
				id = "workspace",
				divider = false,
				ui.pane { size = 250, fixed = true, tree(state) },
				ui.pane { class = "card-pane", editor(state) },
				ui.pane { size = 320, fixed = true, class = "card-pane-last", wiki(state) },
			},
		}
	end,
}
