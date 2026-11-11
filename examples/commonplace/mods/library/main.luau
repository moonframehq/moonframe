-- The Library layout: places and tags, a list of notes, the page (to read,
-- or its source to edit), and a card of what links to it. Everything comes
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
	mode: string,
	tab: string,
	views: { [string]: any },
}

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

-- The views this state needs, as messages to the provider. `again` asks
-- for answers already sent (after a reload, when the mod has none).
local function watches(state: State, again: boolean?): { any }
	local list: any
	if state.query ~= "" or state.tag then
		local q = state.query
		if state.tag then
			q = `tag:{state.tag} {q}`
		end
		list = { id = "list", kind = "search", q = q, limit = 100 }
	else
		list = { id = "list", kind = "list", folder = folder_of(state.section), limit = 200 }
	end
	local sends: { any } = { { to = "provider:notes", msg = { watch = list } } }
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
	state.open = path
	state.views.note = nil
	state.views.context = nil
	return state, note_watches(state)
end

-- A snippet or excerpt on one line: match marks and line breaks gone.
local function plain(text: string?): string
	local out = string.gsub(text or "", "[\1\2]", "")
	out = string.gsub(out, "%s+", " ")
	return out
end

local function sidebar(state: State, notes: any)
	local counts = notes.counts or {}
	local rows = {}
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
	local tags = {}
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
		#rows == 0 and ui.column { padding = "large", ui.text { if searching then "Nothing found." else "No notes here yet.", class = "muted" } },
		#rows > 0 and ui.list {
			id = "notes",
			count = #rows,
			row_height = ROW_HEIGHT,
			grow = true,
			row = function(i)
				local note = rows[i]
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
			state.open and segmented { small = true, selected = state.mode, options = { { id = "page", label = "Page" }, { id = "source", label = "Edit" } } },
			state.open and icon_button { icon = "trash-2", tooltip = "Move to the trash", size = "small", msg = "trash" },
		},
		if note and note.text
			then ui.column { grow = true, scroll = state.mode ~= "source", align = if state.mode == "source" then "stretch" else "center", note_page { note = note, mode = state.mode } }
			else ui.column { grow = true, align = "center", justify = "center", ui.text { if state.open then "Opening…" else "Pick a note.", class = "muted" } },
	}
end

local function context(state: State)
	local view = state.views.context or {}
	local refs = {}
	for _, ref in view.backlinks or {} do
		table.insert(refs, ui.column { class = "ref", msg = { open = ref.path }, ui.text { ref.title, class = "ref-title" }, ui.text { ref.context, class = "ref-context", lines = 2 } })
	end
	local mentions = {}
	for _, ref in view.mentions or {} do
		table.insert(mentions, ui.column { class = "ref", msg = { open = ref.path }, ui.text { ref.title, class = "ref-title" }, ui.text { plain(ref.snippet), class = "ref-context", lines = 2 } })
	end
	local heads = {}
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
		local state: State = previous or { section = "notes", query = "", mode = "page", tab = "links", views = {} }
		-- Views come again from the provider; keep none from before.
		state.views = {}
		return state, watches(state, true)
	end,

	update = function(state: State, msg)
		if type(msg) ~= "table" then
			if msg == "new" then
				return state, { { to = "provider:notes", msg = { create = { path = "notes/Untitled.md", text = "", unique = true }, id = "new" } } }
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
			-- The editor's own save coming back carries no text: keep ours.
			if msg.view == "note" and data and data.own and current then
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
			if (msg.action == "create" or msg.action == "today") and msg.ok and msg.path then
				return open(state, msg.path)
			elseif msg.action == "save" and msg.outcome == "merged" and state.views.note then
				state.views.note.text = msg.text
			end
			return
		end
		if msg.section then
			state.section = msg.section
			state.tag = nil
			state.query = ""
			if msg.section == "inbox" then
				return open(state, "inbox.md")
			elseif msg.section == "today" then
				return state, { { to = "provider:notes", msg = { today = true, id = "today" } } }
			end
			return state, watches(state)
		elseif msg.tag then
			state.tag = if state.tag == msg.tag then nil else msg.tag
			return state, watches(state)
		elseif msg.query then
			state.query = if msg.clear then "" else msg.value or ""
			return state, watches(state)
		elseif msg.open then
			return open(state, msg.open)
		elseif msg.select then
			state.mode = msg.select
		elseif msg.tab then
			state.tab = msg.tab
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
