-- The documents of the library's chosen section, newest first, filtered by
-- the search field; a click opens one on the page, a right-click offers
-- Favorite, Move to Trash, or (in the trash) Put Back and Delete.

local document_row = use "document-row"
local context_menu = use "context-menu"
local search_field = use "search-field"
local icon_button = use "icon-button"
local empty_state = use "empty-state"

type State = {
	section: string,
	query: string,
	menu: string?,
	lead: boolean,
}

local TITLES = { all = "All Documents", favorites = "Favorites", trash = "Trash" }

local function shows(doc: any, state: State): boolean
	local section = if state.section == "trash"
		then doc.trashed
		elseif state.section == "favorites" then doc.favorite and not doc.trashed
		else not doc.trashed
	if not section then
		return false
	end
	local query = string.lower(state.query)
	if query == "" then
		return true
	end
	local haystack = string.lower(`{doc.title} {doc.excerpt}`)
	return string.find(haystack, query, 1, true) ~= nil
end

local function actions(doc: any): { any }
	if doc.trashed then
		return {
			{ id = "restore", label = "Put Back", icon = "rotate-ccw" },
			"separator",
			{ id = "purge", label = "Delete Forever", icon = "trash-2", danger = true },
		}
	end
	return {
		{ id = "favorite", label = if doc.favorite then "Remove from Favorites" else "Add to Favorites", icon = "star" },
		"separator",
		{ id = "trash", label = "Move to Trash", icon = "trash-2", danger = true },
	}
end

return mod {
	region = "list",
	title = "Documents",

	styles = {
		[".documents-head"] = { height = 52, padding = { 0, 12, 0, 16 }, gap = 8 },
		[".documents-head-lead"] = { padding = { 0, 12, 0, 84 } },
		[".documents-title"] = { weight = "semibold" },
		[".documents-count"] = { size = "small", color = "muted-foreground" },
		[".documents-search"] = { padding = { 0, 12, 8, 12 } },
		[".documents-hidden"] = { padding = { 12, 16 }, size = "small", color = "muted-foreground" },
		[".documents-list"] = { padding = { 0, 8, 12, 8 }, gap = 2 },
	},

	init = function(previous: State?): State
		return previous or { section = "all", query = "", lead = false }
	end,

	update = function(state: State, msg)
		if type(msg) ~= "table" then
			if msg == "clear" then
				state.query = ""
			elseif msg == "close-menu" then
				state.menu = nil
			end
			return state
		end
		if msg.section then
			state.section = msg.section
		elseif msg.lead ~= nil then
			state.lead = msg.lead
		elseif msg.msg == "query" then
			state.query = msg.value or ""
		elseif msg.menu then
			state.menu = msg.menu
		elseif msg.pick and state.menu then
			local id = state.menu
			state.menu = nil
			return state, { { to = "provider:library", msg = { [msg.pick] = id } } }
		end
		return state
	end,

	view = function(state: State, app)
		local library = app.library
		local documents = library and library.documents or {}
		local current = library and library.current and library.current.id
		local shown: { any } = {}
		for _, doc in documents do
			if shows(doc, state) then
				table.insert(shown, doc)
			end
		end
		local rows: { any } = {}
		for _, doc in shown do
			table.insert(rows, context_menu {
				open = state.menu == doc.id,
				context = { menu = doc.id },
				dismiss = "close-menu",
				items = actions(doc),
				width = 200,
				document_row {
					title = doc.title,
					excerpt = doc.excerpt,
					date = doc.modified,
					words = doc.words,
					favorite = doc.favorite,
					selected = doc.id == current,
					msg = { open = doc.id },
					to = "provider:library",
				},
			})
		end
		local hidden = library and library.hidden or 0
		if hidden > 0 and #rows > 0 then
			table.insert(rows, ui.text {
				`{hidden} older {if hidden == 1 then "document is" else "documents are"} not listed`,
				class = "documents-hidden",
			})
		end
		local body = if #rows > 0
			then ui.column { class = "documents-list", grow = true, scroll = true, ui.column { shrink = false, gap = "none", table.unpack(rows) } }
			else ui.column {
				grow = true,
				justify = "center",
				empty_state {
					icon = if state.section == "trash" then "trash-2" else "file-text",
					title = if state.query ~= "" then "No matches" elseif state.section == "trash" then "The trash is empty" else "Nothing here yet",
					message = if state.query ~= "" then "Nothing in this section says that." else "Documents you write show up here.",
				},
			}
		return ui.column {
			grow = true,
			gap = "none",
			ui.row {
				class = if state.lead then "documents-head documents-head-lead" else "documents-head",
				align = "center",
				drag = true,
				shrink = false,
				ui.text { TITLES[state.section] or "Documents", class = "documents-title" },
				ui.text { tostring(#shown), class = "documents-count" },
				ui.spacer(),
				icon_button { icon = "square-pen", tooltip = "New document (⌘N)", msg = { new = true }, to = "provider:library" },
			},
			ui.row {
				class = "documents-search",
				shrink = false,
				search_field {
					id = "documents-search",
					value = state.query,
					placeholder = "Search",
					icon = true,
					width = 276,
					change = "query",
					cancel = "clear",
				},
			},
			body,
		}
	end,
}
