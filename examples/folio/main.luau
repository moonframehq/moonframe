-- Folio's shell: the library (sections), the documents in the chosen one,
-- and the page, side by side; ⌘1 and ⌘2 fold the first two away to leave
-- the page alone. The title bar is hidden: the leftmost pane's header leaves
-- room for the traffic lights, so the shell tells the panes which is first.

local side_panels = use "side-panels"

local MENUS: { { title: string, items: { any } } } = {
	{ title = "File", items = {
		{ label = "New Document", msg = { new = true }, to = "provider:library", shortcut = "secondary-n" },
		"-",
		{ label = "Move to Trash", msg = { trash_current = true }, to = "provider:library", shortcut = "secondary-backspace" },
	} },
	{ title = "View", items = {
		{ label = "Focus Mode: Sentence, Paragraph, Off", msg = "cycle-dim", to = "editor", shortcut = "secondary-d" },
		{ label = "Typewriter Mode", msg = "typewriter", to = "editor", shortcut = "secondary-t" },
		{ label = "Preview", msg = "preview", to = "editor", shortcut = "secondary-r" },
		{ label = "Switch Font", msg = "font", to = "editor", shortcut = "secondary-shift-f" },
		"-",
		{ label = "Show or Hide the Library", msg = "toggle-left", shortcut = "secondary-1" },
		{ label = "Show or Hide the Documents", msg = "toggle-middle", shortcut = "secondary-2" },
	} },
	{ title = "Window", items = {
		{ label = "Minimize", command = "minimize", shortcut = "secondary-m" },
		{ label = "Zoom", command = "maximize" },
	} },
}

type State = { left: boolean, middle: boolean }

-- Which pane is leftmost, for the panes that draw the traffic lights' room.
local function first(state: State)
	local lead = if state.left then "library" elseif state.middle then "documents" else "editor"
	return {
		{ to = "documents", msg = { lead = lead == "documents" } },
		{ to = "editor", msg = { lead = lead == "editor" } },
	}
end

return mod {
	window = {
		titlebar = "hidden",
		traffic_lights = { x = 18, y = 20 },
		size = { width = 1240, height = 800 },
	},

	about = {
		version = "0.1",
		description = "A quiet place to write: a library of markdown documents, focus mode and typewriter scrolling.",
		website = "https://github.com/moonframehq/moonframe",
		copyright = "MIT licensed",
	},

	menus = MENUS,

	init = function(previous: State?): State
		return previous or { left = true, middle = true }
	end,

	update = function(state: State, msg)
		if msg == "toggle-left" then
			state.left = not state.left
			return state, first(state)
		elseif msg == "toggle-middle" then
			state.middle = not state.middle
			return state, first(state)
		elseif msg == "focus-page" then
			-- From the page's own button: fold both, or bring both back.
			local open = not (state.left or state.middle)
			state.left, state.middle = open, open
			return state, first(state)
		end
		return state
	end,

	view = function(state: State)
		local body = side_panels {
			id = "folio",
			left = state.left,
			middle = state.middle,
			sidebar_size = 220,
			list_size = 300,
			sidebar = ui.region "left",
			list = ui.region "list",
			main = ui.region "main",
		}
		return ui.column {
			grow = true,
			gap = "none",
			body,
			ui.row { class = "feedback-corner", float = { at = "bottom-right", inset = 20 }, ui.mount "feedback" },
		}
	end,
}
