-- Commonplace's shell. The window shows one of three layouts over the same
-- vault, picked in View: Library (places, list, page, links), Journal
-- (today's outline and the inbox) or Workspace (tree, tabs, wiki agent).
-- The choice is kept across launches. Each layout is a mod of its own.

local LAYOUTS = {
	{ id = "library", label = "Library Layout", shortcut = "secondary-alt-1" },
	{ id = "journal", label = "Journal Layout", shortcut = "secondary-alt-2" },
	{ id = "workspace", label = "Workspace Layout", shortcut = "secondary-alt-3" },
}

local function view_menu(): { any }
	local items: { any } = {}
	for _, layout in LAYOUTS do
		table.insert(items, { label = layout.label, msg = { layout = layout.id }, shortcut = layout.shortcut })
	end
	return items
end

local MENUS: { { title: string, items: { any } } } = {
	{ title = "File", items = {
		{ label = "New Note", msg = "new", to = "library", shortcut = "secondary-n" },
		"-",
		{ label = "Import from Notes.app…", msg = "import", to = "library" },
		{ label = "Import from Notes.app with Its Markdown Export…", msg = "import-export", to = "library" },
	} },
	{ title = "View", items = view_menu() },
	{ title = "Window", items = {
		{ label = "Minimize", command = "minimize", shortcut = "secondary-m" },
		{ label = "Zoom", command = "maximize" },
	} },
}

type State = { layout: string }

local function known(id: any): boolean
	for _, layout in LAYOUTS do
		if layout.id == id then
			return true
		end
	end
	return false
end

return mod {
	window = { titlebar = "hidden", traffic_lights = { x = 18, y = 18 }, size = { width = 1320, height = 820 } },

	about = {
		version = "0.1",
		description = "A second brain: your notes, your sources and an LLM-kept wiki, as markdown files.",
		website = "https://github.com/moonframehq/moonframe",
		copyright = "MIT licensed",
	},

	menus = MENUS,

	init = function(previous: State?): State
		if previous and known(previous.layout) then
			return previous
		end
		return { layout = "library" }
	end,

	update = function(state: State, msg)
		if type(msg) == "table" and known(msg.layout) then
			state.layout = msg.layout
		end
	end,

	view = function(state: State, app)
		return ui.column {
			grow = true,
			gap = "none",
			class = "frame",
			ui.mount(state.layout),
			ui.row { float = { at = "bottom-right", inset = 20 }, ui.mount "feedback" },
		}
	end,
}
