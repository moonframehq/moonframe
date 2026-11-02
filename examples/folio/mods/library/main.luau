-- The library: All Documents, Favorites and Trash, with their counts, and
-- a New Document row. It sits under the traffic lights, so its first row
-- is a draggable strip with room for them. Choosing a section tells the
-- documents list.

type State = { section: string }

local SECTIONS: { { id: string, label: string, icon: any } } = {
	{ id = "all", label = "All Documents", icon = "file-text" },
	{ id = "favorites", label = "Favorites", icon = "star" },
	{ id = "trash", label = "Trash", icon = "trash-2" },
}

-- How many documents each section holds.
local function counts(documents: { any })
	local all, favorites, trash = 0, 0, 0
	for _, doc in documents do
		if doc.trashed then
			trash += 1
		else
			all += 1
			if doc.favorite then
				favorites += 1
			end
		end
	end
	return { all = all, favorites = favorites, trash = trash }
end

return mod {
	region = "left",
	title = "Library",

	styles = {
		[".library"] = { padding = { 0, 10, 12, 10 }, gap = 2 },
		[".library-lights"] = { height = 52 },
		[".library-heading"] = { padding = { 6, 10, 6, 10 }, size = "small", weight = "medium", color = "muted-foreground" },
		[".library-row"] = { padding = { 7, 10 }, radius = 8, gap = 10, transition = "fade fast" },
		[".library-row:hover"] = { background = "list-hover" },
		[".library-row:selected"] = { background = "list-active" },
		[".library-icon"] = { color = "muted-foreground" },
		[".library-count"] = { size = "small", color = "muted-foreground" },
		[".library-new"] = { padding = { 7, 10 }, radius = 8, gap = 10, color = "muted-foreground" },
		[".library-new:hover"] = { background = "list-hover", color = "foreground" },
		[".library-folder"] = { padding = { 0, 10 }, size = "small", color = "muted-foreground", opacity = 0.7 },
	},

	init = function(previous: State?): State
		return previous or { section = "all" }
	end,

	update = function(state: State, msg)
		if type(msg) == "table" and msg.section then
			state.section = msg.section
			return state, { { to = "documents", msg = { section = msg.section } } }
		end
		return state
	end,

	view = function(state: State, app)
		local library = app.library
		local tally = counts(library and library.documents or {})
		local rows: { any } = {}
		for _, section in SECTIONS do
			table.insert(rows, ui.row {
				class = "library-row",
				align = "center",
				selected = state.section == section.id,
				msg = { section = section.id },
				ui.icon { name = section.icon, class = "library-icon" },
				ui.text { section.label },
				ui.spacer(),
				ui.text { tostring(tally[section.id] or 0), class = "library-count" },
			})
		end
		return ui.column {
			class = "library",
			grow = true,
			ui.row { class = "library-lights", drag = true, shrink = false },
			ui.text { "Library", class = "library-heading" },
			ui.column { gap = "none", table.unpack(rows) },
			ui.spacer(),
			ui.row {
				class = "library-new",
				align = "center",
				msg = { new = true },
				to = "provider:library",
				tooltip = "New document (⌘N)",
				ui.icon "square-pen",
				ui.text "New Document",
			},
			library and library.folder and ui.text {
				library.folder,
				class = "library-folder",
				truncate = true,
				tooltip = "Your documents are markdown files in this folder",
			},
		}
	end,
}
