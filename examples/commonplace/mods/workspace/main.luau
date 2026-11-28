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
	-- An enhance of the open note running, and what the last action said:
	-- `open` is a note to open from it (a conflict copy), `save` the note
	-- whose save failed.
	enhancing: boolean?,
	notice: { text: string, tone: string?, open: string?, save: string? }?,
	folded: { [string]: boolean? },
	-- Pages of the vault the tree holds: a large vault comes a page at a
	-- time, more as the user asks.
	pages: number,
	-- The first row of the search results' window, moved as they scroll.
	offset: number,
	views: { [string]: any },
}

-- Notes the tree asks for per page, and search results per window.
local TREE_PAGE = 500
local PAGE = 200

-- The view id of the tree's page `page`.
local function page_id(page: number): string
	return if page == 1 then "all" else `all-{page}`
end

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

-- The tree's page `page` of the vault's notes.
local function page_watch(page: number): any
	return { to = "provider:notes", msg = { watch = { id = page_id(page), kind = "list", folder = "", order = "title", offset = (page - 1) * TREE_PAGE, limit = TREE_PAGE } } }
end

-- The search results' window, from `offset`.
local function search_watch(state: State): any
	return { to = "provider:notes", msg = { watch = { id = "search", kind = "search", q = state.query, offset = state.offset, limit = PAGE } } }
end

-- Where the results' window should start so rows `first` to `last` are
-- there; nil when they are, or while a move is on its way.
local function window(view: any, offset: number, first: number, last: number): number?
	if not view or (view.offset or 0) ~= offset then
		return nil
	end
	local count = #(view.rows or {})
	if (first <= offset and offset > 0) or (last > offset + count and count >= PAGE) then
		local start = math.max(0, first - PAGE // 4)
		start -= start % 20
		if start ~= offset then
			return start
		end
	end
	return nil
end

-- Every view this state needs; `again` asks for answers already sent.
local function watches(state: State, again: boolean?): { any }
	local sends: { any } = {
		{ to = "provider:notes", msg = { watch = { id = "health", kind = "health", folder = "wiki", limit = 12 } } },
		{ to = "provider:notes", msg = { watch = { id = "log", kind = "note", path = "log.md" } } },
	}
	for page = 1, state.pages do
		table.insert(sends, page_watch(page))
	end
	if state.query ~= "" then
		table.insert(sends, search_watch(state))
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
	if state.open == path and state.views.note then
		-- Already showing it: keep the page as it is.
		return state, {} :: { any }
	end
	state.open = path
	state.notice = nil
	state.views.note = nil
	state.views.context = nil
	local sends = note_watches(state)
	-- Asked again: the provider sends nothing for a view it has already
	-- answered unchanged (the same note opened again after another).
	for _, send in sends do
		send.msg.again = true
	end
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

type FileRow = { path: string, title: string }

-- The notes of every page loaded, each once, and how many the vault has.
local function loaded(state: State): ({ FileRow }, number)
	local files: { FileRow } = {}
	local have: { [string]: boolean } = {}
	for page = 1, state.pages do
		for _, row in (state.views[page_id(page)] or {}).rows or {} do
			if not have[row.path] then
				have[row.path] = true
				table.insert(files, row)
			end
		end
	end
	return files, (state.views.all or {}).total or #files
end

local function tree_rows(state: State): { any }
	local rows: { any } = {}
	local seen: { [string]: boolean } = {}
	local files, total = loaded(state)
	table.sort(files, function(a: FileRow, b: FileRow)
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
	if #files < total then
		-- The vault has more than the pages loaded: a row to load the next.
		table.insert(rows, { more = true, name = `{#files} of {total} notes · Load more`, depth = 0 })
	end
	return rows
end

local function tree(state: State)
	if state.query ~= "" then
		local view = state.views.search or {}
		local hits: { any } = view.rows or {}
		local offset: number = view.offset or 0
		-- Results come a window at a time; the list grows by a row while
		-- windows keep coming full.
		local count = offset + #hits + (if #hits >= PAGE then 1 else 0)
		return ui.column {
			grow = true,
			gap = "none",
			ui.text { "Results", class = "nav-heading" },
			count == 0 and ui.column { padding = "large", ui.text { "Nothing found.", class = "muted small" } },
			count > 0 and ui.list {
				id = "hits",
				count = count,
				row_height = HIT_ROW,
				grow = true,
				range = { page = true },
				row = function(i)
					local hit = hits[i - offset]
					if not hit then
						return ui.column { class = "note-row", height = HIT_ROW - 4, ui.text { "…", class = "muted" } }
					end
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
				if row.more then
					return ui.row {
						class = "nav-row",
						align = "center",
						height = TREE_ROW - 2,
						msg = "more",
						ui.icon { "ellipsis", class = "nav-icon" },
						ui.row { grow = true, gap = "none", ui.text { row.name, class = "muted", truncate = true } },
					}
				end
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

-- A save's answer: the merged text for the note it was for, and what the
-- user must know: a write that failed (kept and tried again), a conflict.
local function saved(state: State, msg: any)
	local note: any = state.views.note
	if not msg.ok then
		local where = if msg.path == state.open then "" else ` {msg.path}`
		state.notice = {
			text = `Not saved{where}: {msg.error or "the write failed"}{if msg.kept then " (trying again)" else ""}`,
			tone = "danger",
			save = msg.path,
			open = if msg.path == state.open then nil else msg.path,
		}
		return
	end
	if state.notice and state.notice.save == msg.path then
		state.notice = nil
	end
	if msg.outcome == "merged" and note and note.path == msg.path and msg.path == state.open and type(msg.text) == "string" then
		note.text = msg.text
	elseif msg.outcome == "conflict" and msg.copy then
		state.notice = { text = `Also changed elsewhere: their version is in {msg.copy}`, tone = "warning", open = msg.copy }
	end
end

-- What the last action said, in the page's header; a click opens the note
-- it names.
local function notice_row(notice: any): Element?
	if not notice then
		return nil
	end
	return ui.row {
		class = "head-notice",
		align = "center",
		max_width = 360,
		msg = if notice.open then { open = notice.open } else nil,
		tooltip = if notice.open then `Open {notice.open}` else notice.text,
		notice.tone and ui.icon { if notice.tone == "danger" then "circle-alert" else "triangle-alert", tone = notice.tone :: any, size = "small" },
		ui.text { notice.text, tone = (notice.tone or "muted") :: any, size = "small", truncate = true },
	}
end

local function editor(state: State)
	local tabs: { Element } = {}
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
			notice_row(state.notice),
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
			then ui.column { grow = true, scroll = true, note_page { id = "workspace", note = note, width = 640 } }
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
	local entries: { Element } = {}
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
	local dangling: { Element } = {}
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
	local orphans: { Element } = {}
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
		local state: State = previous or { query = "", tabs = {}, folded = {}, pages = 1, offset = 0, views = {} }
		state.views = {}
		state.enhancing = nil
		state.notice = nil
		state.pages = 1
		state.offset = 0
		return state, watches(state, true)
	end,

	update = function(state: State, msg)
		if msg == "new" then
			return state, { { to = "provider:notes", msg = { create = { path = "notes/Untitled.md", text = "", unique = true }, id = "new" } } }
		elseif msg == "enhance" and state.open then
			state.enhancing = true
			state.notice = nil
			return state, { { to = "provider:notes", msg = { enhance = { path = state.open }, id = "enhance" } } }
		elseif msg == "more" then
			state.pages += 1
			return state, { page_watch(state.pages) }
		end
		if type(msg) ~= "table" then
			return
		end
		if msg.view then
			local data = msg.data
			local current = state.views[msg.view]
			-- An answer for the note open before this one, still on its way.
			if (msg.view == "note" or msg.view == "context") and data and data.path and data.path ~= state.open then
				return
			end
			-- The editor's own save coming back carries no text: keep ours
			-- (in `log` too, when log.md is the note being edited).
			if data and data.own and current then
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
			if msg.action == "save" then
				saved(state, msg)
			elseif msg.action == "enhance" then
				state.enhancing = nil
				local done = `{msg.titled or 0} titles, {msg.icons or 0} icons`
				if not msg.ok then
					state.notice = { text = msg.error or "Enhance failed", tone = "danger" }
				elseif msg.conflict then
					-- The note changed meanwhile where the titles went: both kept.
					state.notice = { text = `{done}; also changed elsewhere: their version is in {msg.conflict}`, tone = "warning", open = msg.conflict }
				else
					state.notice = { text = done }
				end
			elseif msg.ok == false then
				state.notice = { text = msg.error or `{msg.action or "That"} failed`, tone = "danger" }
			elseif msg.action == "create" and msg.path then
				return open(state, msg.path)
			end
			return
		end
		if msg.page then
			local start = window(state.views.search, state.offset, msg.first or 1, msg.last or 1)
			if start then
				state.offset = start
				return state, { search_watch(state) }
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
				local next = state.tabs[math.max(1, (at or 1) - 1)]
				state.open = nil
				state.views.note = nil
				state.views.context = nil
				if next then
					return open(state, next)
				end
				-- No tab left: stop the views, so the note opened again later is answered.
				return state, {
					{ to = "provider:notes", msg = { unwatch = "note" } },
					{ to = "provider:notes", msg = { unwatch = "context" } },
				}
			end
		elseif msg.fold then
			state.folded[msg.fold] = not state.folded[msg.fold] or nil
		elseif msg.query then
			state.query = if msg.clear then "" else msg.value or ""
			state.offset = 0
			return state, watches(state)
		elseif msg.create then
			return state, { { to = "provider:notes", msg = { create = { path = `wiki/{msg.create}.md`, text = "" }, id = "create-link" } } }
		elseif msg.edit and state.open then
			if state.views.note then
				state.views.note.text = msg.value
			end
			-- `base`: the text the edit was made on, merged with changes since.
			return state, { { to = "provider:notes", msg = { save = { path = state.open, text = msg.value, base = msg.base }, id = "save" } } }
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
