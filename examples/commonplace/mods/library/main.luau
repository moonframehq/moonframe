-- The Library layout: places and tags, a list of notes, the page (read and
-- edited in place), and a card of what links to it. Everything comes
-- from the notes provider: the mod asks for what it shows as live views
-- (`watch`), which the provider answers again whenever the vault changes.

local note_page = use "note-page"
local icon_button = use "icon-button"
local segmented = use "segmented"

type State = {
	section: string,
	tag: string?,
	query: string,
	open: string?,
	tab: string,
	-- An enhance of the open note running, and what the last action said:
	-- `open` is a note to open from it (a conflict copy), `save` the note
	-- whose save failed.
	enhancing: boolean?,
	notice: { text: string, tone: string?, open: string?, save: string? }?,
	-- The first row of the list's window: a vault's notes come a page at a
	-- time, moved as the list scrolls.
	offset: number,
	views: { [string]: any },
}

-- Rows the list asks for at a time.
local PAGE = 200

local SECTIONS: { any } = {
	{ id = "inbox", label = "Inbox", icon = "inbox", folder = nil },
	{ id = "today", label = "Today", icon = "sun", folder = nil },
	{ id = "all", label = "All notes", icon = "layers", folder = "" },
	{ id = "notes", label = "Notes", icon = "file-text", folder = "notes" },
	{ id = "journal", label = "Journal", icon = "calendar", folder = "journal" },
	{ id = "wiki", label = "Wiki", icon = "book-open", folder = "wiki" },
	{ id = "raw", label = "Sources", icon = "archive", folder = "raw" },
}

local function folder_of(section: string): string
	for _, entry in SECTIONS do
		if entry.id == section and entry.folder then
			return entry.folder
		end
	end
	return "notes"
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

-- The list's view: the section's notes or the search's, one page from
-- `offset`.
local function list_watch(state: State): any
	local list: any
	if state.query ~= "" or state.tag then
		local q = state.query
		if state.tag then
			q = `tag:{state.tag} {q}`
		end
		list = { id = "list", kind = "search", q = q, offset = state.offset, limit = PAGE }
	else
		list = { id = "list", kind = "list", folder = folder_of(state.section), offset = state.offset, limit = PAGE }
	end
	return { to = "provider:notes", msg = { watch = list } }
end

-- Where the list's window should start so rows `first` to `last` are
-- there; nil when they are, or while a move is on its way.
local function window(view: any, offset: number, first: number, last: number): number?
	if not view or (view.offset or 0) ~= offset then
		return nil
	end
	local count = #(view.rows or {})
	local more = if view.total then view.total > offset + count else count >= PAGE
	if (first <= offset and offset > 0) or (last > offset + count and more) then
		local start = math.max(0, first - PAGE // 4)
		start -= start % 20
		if start ~= offset then
			return start
		end
	end
	return nil
end

-- The views this state needs, as messages to the provider. `again` asks
-- for answers already sent (after a reload, when the mod has none).
local function watches(state: State, again: boolean?): { any }
	local sends: { any } = { list_watch(state) }
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

-- A snippet or excerpt on one line: match marks and line breaks gone.
local function plain(text: string?): string
	local out = string.gsub(text or "", "[\1\2]", "")
	out = string.gsub(out, "%s+", " ")
	return out
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
		max_width = 420,
		msg = if notice.open then { open = notice.open } else nil,
		tooltip = if notice.open then `Open {notice.open}` else notice.text,
		notice.tone and ui.icon { if notice.tone == "danger" then "circle-alert" else "triangle-alert", tone = notice.tone :: any, size = "small" },
		ui.text { notice.text, tone = (notice.tone or "muted") :: any, size = "small", truncate = true },
	}
end

local function sidebar(state: State, notes: any)
	local counts = notes.counts or {}
	local rows: { Element } = {}
	for _, section in SECTIONS do
		local count = if section.folder then counts[if section.folder == "" then "all" else section.folder] else nil
		table.insert(
			rows,
			ui.row {
				class = "nav-row",
				selected = state.section == section.id and not state.tag,
				align = "center",
				msg = { section = section.id },
				ui.icon { section.icon :: any, class = "nav-icon" },
				ui.row { grow = true, gap = "none", ui.text { section.label } },
				count and count > 0 and ui.text { `{count}`, class = "nav-count" },
			}
		)
	end
	local tags: { Element } = {}
	for index, tag in notes.tags or {} do
		if index > 10 then
			break
		end
		table.insert(
			tags,
			ui.row {
				class = "nav-row",
				selected = state.tag == tag.tag,
				align = "center",
				msg = { tag = tag.tag },
				ui.icon { "hash", class = "nav-icon" },
				ui.row { grow = true, gap = "none", ui.text { tag.tag, truncate = true } },
				ui.text { `{tag.count}`, class = "nav-count" },
			}
		)
	end
	local import = notes.import
	local importing = import and import.running
	return ui.column {
		grow = true,
		gap = "none",
		ui.row { height = 52, drag = true, shrink = false },
		ui.row {
			class = "nav",
			ui.row {
				class = "search-box",
				grow = true,
				align = "center",
				ui.icon { "search", class = "muted" },
				ui.input { id = "library-search", value = state.query, placeholder = "Search", change = { query = true }, keys = { escape = { query = true, clear = true } }, grow = true, class = "bare-input" },
			},
		},
		ui.column { class = "nav", table.unpack(rows) },
		#tags > 0 and ui.text { "Tags", class = "nav-heading" },
		ui.column { class = "nav", table.unpack(tags) },
		ui.spacer(),
		import and ui.column {
			padding = "normal",
			gap = "small",
			ui.text {
				if importing then `{import.phase} {import.done}/{import.total}` elseif import.error then import.error else `Imported {import.report and import.report.created_count or 0} notes from Notes.app`,
				class = "muted small",
				wrap = true,
			},
		},
		ui.row {
			padding = "normal",
			align = "center",
			ui.icon { "book-open", class = "muted" },
			ui.row { grow = true, gap = "none", ui.text { if notes.vault then `{notes.vault.name} · {counts.all or 0}` else "No vault", class = "muted small", truncate = true } },
			icon_button { icon = "download", tooltip = "Import from Notes.app", size = "small", msg = "import", disabled = importing },
		},
	}
end

-- Rows of the note list: title, a line of excerpt, the path.
local ROW_HEIGHT = 84

local function list(state: State)
	local view = state.views.list or {}
	local searching = state.query ~= "" or state.tag ~= nil
	local rows: { any } = view.rows or {}
	local offset: number = view.offset or 0
	-- The whole list, of which only a page is loaded; a search, which
	-- knows no total, grows by a row while pages keep coming full.
	local count: number = view.total or (offset + #rows + (if #rows >= PAGE then 1 else 0))
	local title = if state.tag then `#{state.tag}` elseif searching then "Search" else "Notes"
	for _, section in SECTIONS do
		if section.id == state.section and not searching then
			title = section.label
		end
	end
	return ui.column {
		class = "card",
		grow = true,
		gap = "none",
		ui.row {
			class = "card-head",
			align = "center",
			ui.row { grow = true, gap = "none", ui.text { title, class = "card-title" } },
			view.total and ui.text { `{view.total}`, class = "muted small" },
			icon_button { icon = "square-pen", tooltip = "New note", size = "small", msg = "new" },
		},
		count == 0 and ui.column { padding = "large", ui.text { if searching then "Nothing found." else "No notes here yet.", class = "muted" } },
		count > 0 and ui.list {
			id = "notes",
			count = count,
			row_height = ROW_HEIGHT,
			grow = true,
			range = { page = true },
			row = function(i)
				local note = rows[i - offset]
				if not note then
					-- Outside the page loaded: it comes as the window moves.
					return ui.column { class = "note-row", height = ROW_HEIGHT - 6, ui.text { "…", class = "muted" } }
				end
				return ui.column {
					class = "note-row",
					selected = note.path == state.open,
					msg = { open = note.path },
					height = ROW_HEIGHT - 6,
					ui.text { note.title, class = "note-row-title", truncate = true },
					ui.text { plain(note.excerpt or note.snippet), class = "note-row-excerpt", truncate = true },
					ui.text { note.path, class = "muted small", truncate = true },
				}
			end,
		},
	}
end

local function editor(state: State)
	local note = state.views.note
	return ui.column {
		class = "card",
		grow = true,
		gap = "none",
		ui.row {
			class = "card-head",
			align = "center",
			ui.row { grow = true, gap = "none", ui.text { state.open or "", class = "muted small", truncate = true } },
			notice_row(state.notice),
			state.open and ui.row {
				class = "head-action",
				align = "center",
				msg = if state.enhancing then nil else "enhance",
				tooltip = "Fetch page titles and site icons for this note's links",
				ui.icon { name = if state.enhancing then "loader-circle" else "sparkles", spin = state.enhancing == true, size = "small" },
				ui.text { if state.enhancing then "Enhancing…" else "Enhance", class = "small" },
			},
			state.open and icon_button { icon = "trash-2", tooltip = "Move to the trash", size = "small", msg = "trash" },
		},
		if note and note.text
			then ui.column { grow = true, scroll = true, note_page { id = "library", note = note } }
			else ui.column { grow = true, align = "center", justify = "center", ui.text { if state.open then "Opening…" else "Pick a note.", class = "muted" } },
	}
end

local function context(state: State)
	local view = state.views.context or {}
	local refs: { Element } = {}
	for _, ref in view.backlinks or {} do
		table.insert(refs, ui.column { class = "ref", msg = { open = ref.path }, ui.text { ref.title, class = "ref-title" }, ui.text { ref.context, class = "ref-context", lines = 2 } })
	end
	local mentions: { Element } = {}
	for _, ref in view.mentions or {} do
		table.insert(mentions, ui.column { class = "ref", msg = { open = ref.path }, ui.text { ref.title, class = "ref-title" }, ui.text { plain(ref.snippet), class = "ref-context", lines = 2 } })
	end
	local heads: { Element } = {}
	local info = view.info or {}
	for _, head in info.headings or {} do
		table.insert(heads, ui.text { head[2], class = "small", style = { padding = { 0, 0, 0, (head[1] - 1) * 12 } } })
	end
	local body
	if state.tab == "outline" then
		body = ui.column { gap = "small", #heads == 0 and ui.text { "No headings.", class = "muted small" }, table.unpack(heads) }
	else
		body = ui.column {
			gap = "large",
			ui.column { gap = "small", ui.text { "Backlinks", class = "section-label" }, #refs == 0 and ui.text { "Nothing links here yet.", class = "muted small" }, table.unpack(refs) },
			#mentions > 0 and ui.column { gap = "small", ui.text { "Unlinked mentions", class = "section-label" }, table.unpack(mentions) },
		}
	end
	return ui.column {
		class = "card",
		grow = true,
		gap = "none",
		ui.row {
			class = "card-head",
			segmented {
				small = true,
				grow = true,
				event = "tab",
				selected = state.tab,
				options = { { id = "links", label = "Links", count = #refs }, { id = "outline", label = "Outline", count = nil } } :: { any },
			},
		},
		ui.column { grow = true, scroll = true, padding = "large", body },
	}
end

return mod {
	title = "Library",

	init = function(previous: State?): (State, { any })
		local state: State = previous or { section = "notes", query = "", tab = "links", offset = 0, views = {} }
		-- Views come again from the provider; keep none from before.
		state.views = {}
		state.enhancing = nil
		state.notice = nil
		state.offset = 0
		return state, watches(state, true)
	end,

	update = function(state: State, msg)
		if type(msg) ~= "table" then
			if msg == "new" then
				return state, { { to = "provider:notes", msg = { create = { path = "notes/Untitled.md", text = "", unique = true }, id = "new" } } }
			elseif msg == "enhance" and state.open then
				state.enhancing = true
				state.notice = nil
				return state, { { to = "provider:notes", msg = { enhance = { path = state.open }, id = "enhance" } } }
			elseif msg == "trash" and state.open then
				local path = state.open
				state.open = nil
				state.views.note = nil
				return state, { { to = "provider:notes", msg = { trash = path, id = "trash" } } }
			elseif msg == "import" then
				return state, { { to = "provider:notes", msg = { import = { source = "apple-notes" }, id = "import" } } }
			elseif msg == "import-export" then
				-- The folder Notes.app exported to (File > Export as > Markdown):
				-- its bodies and attachments, with Notes.app's folders and dates.
				return state, { { to = "app:pick", msg = { kind = "folder", prompt = "Choose the folder of Notes.app's Markdown export", reply = { picked_export = true } } } }
			end
			return
		end
		if msg.picked_export then
			if msg.paths and msg.paths[1] then
				return state, { { to = "provider:notes", msg = { import = { source = "apple-notes", export = msg.paths[1] }, id = "import" } } }
			end
			return
		end
		if msg.view then
			local data = msg.data
			local current = state.views[msg.view]
			-- An answer for the note open before this one, still on its way.
			if (msg.view == "note" or msg.view == "context") and data and data.path and data.path ~= state.open then
				return
			end
			-- The editor's own save coming back carries no text: keep ours.
			if data and data.own and current then
				data.text = current.text
			end
			state.views[msg.view] = data
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
			if msg.view == "list" and not state.open and data and data.rows and data.rows[1] then
				return open(state, data.rows[1].path)
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
			elseif (msg.action == "create" or msg.action == "today") and msg.path then
				return open(state, msg.path)
			end
			return
		end
		if msg.page then
			local start = window(state.views.list, state.offset, msg.first or 1, msg.last or 1)
			if start then
				state.offset = start
				return state, { list_watch(state) }
			end
			return
		end
		if msg.section then
			state.section = msg.section
			state.tag = nil
			state.query = ""
			state.offset = 0
			if msg.section == "inbox" then
				return open(state, "inbox.md")
			elseif msg.section == "today" then
				return state, { { to = "provider:notes", msg = { today = true, id = "today" } } }
			end
			return state, watches(state)
		elseif msg.tag then
			state.tag = if state.tag == msg.tag then nil else msg.tag
			state.offset = 0
			return state, watches(state)
		elseif msg.query then
			state.query = if msg.clear then "" else msg.value or ""
			state.offset = 0
			return state, watches(state)
		elseif msg.open then
			return open(state, msg.open)
		elseif msg.tab then
			state.tab = msg.tab
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
		if not notes then
			return ui.column { grow = true, align = "center", justify = "center", ui.text { "Opening the vault…", class = "muted" } }
		end
		if not notes.available then
			return ui.column { grow = true, align = "center", justify = "center", ui.text { notes.reason or "No vault.", class = "muted" } }
		end
		return ui.hsplit {
			id = "library",
			divider = false,
			ui.pane { size = 236, fixed = true, sidebar(state, notes) },
			ui.pane { size = 320, fixed = true, class = "card-pane", ui.column { grow = true, gap = "none", ui.row { height = 12, drag = true, shrink = false }, list(state) } },
			ui.pane { class = "card-pane", ui.column { grow = true, gap = "none", ui.row { height = 12, drag = true, shrink = false }, editor(state) } },
			ui.pane { size = 300, fixed = true, class = "card-pane-last", ui.column { grow = true, gap = "none", ui.row { height = 12, drag = true, shrink = false }, context(state) } },
		}
	end,
}
