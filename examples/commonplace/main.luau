-- Commonplace's shell. The window shows one of three layouts over the same
-- vault, picked in View: Library (places, list, page, links), Journal
-- (today's outline and the inbox) or Workspace (tree, tabs, wiki agent).
-- The choice is kept across launches. Each layout is a mod of its own. The
-- agent chat (mods/chat) opens beside whichever is shown, with ⌘J.

local LAYOUTS = {
	{ id = "library", label = "Library Layout", shortcut = "secondary-alt-1" },
	{ id = "journal", label = "Journal Layout", shortcut = "secondary-alt-2" },
	{ id = "workspace", label = "Workspace Layout", shortcut = "secondary-alt-3" },
}

local function view_menu(): { any }
	local items: { any } = {}
	for _, layout in LAYOUTS do
		table.insert(items, {
			label = layout.label,
			msg = { layout = layout.id },
			shortcut = layout.shortcut,
			-- Ticked while it is the layout shown.
			checked = { layout = layout.id },
		})
	end
	table.insert(items, "-")
	table.insert(items, { label = "Agent Chat", msg = "chat", shortcut = "secondary-j", checked = { chat = true } })
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

type State = { layout: string, chat: boolean }

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
			previous.chat = previous.chat == true
			return previous
		end
		return { layout = "library", chat = false }
	end,

	update = function(state: State, msg)
		if msg == "chat" then
			state.chat = not state.chat
		elseif type(msg) ~= "table" then
			return
		elseif known(msg.layout) then
			state.layout = msg.layout
		elseif msg.chat ~= nil then
			state.chat = msg.chat == true
		elseif msg.link then
			-- A note the agent named: the layout shown opens it.
			return state, { { to = state.layout, msg = msg.link } }
		end
	end,

	view = function(state: State, app)
		return ui.column {
			grow = true,
			gap = "none",
			class = "frame",
			ui.row {
				grow = true,
				gap = "none",
				ui.column { grow = true, ui.mount(state.layout) },
				state.chat and ui.column { width = 380, shrink = false, ui.mount "chat" },
			},
			ui.row {
				float = { at = "bottom-right", inset = 20 },
				gap = "small",
				align = "center",
				not state.chat and ui.row {
					class = "chat-toggle",
					msg = "chat",
					tooltip = "Agent chat (⌘J)",
					align = "center",
					justify = "center",
					width = 32,
					height = 32,
					ui.icon "message-square",
				},
				ui.mount "feedback",
			},
		}
	end,
}
