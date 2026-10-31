-- Files with actions: right-click a file to open its menu, then pick an
-- action (↑ ↓ and Enter work too). "Move to Trash" removes the file.
local context_menu = use "context-menu"

local ITEMS: { any } = {
	{ id = "open", label = "Open", icon = "external-link", shortcut = "enter" },
	{ id = "copy-path", label = "Copy path", icon = "copy", shortcut = "cmd-alt-c" },
	{ id = "reveal", label = "Reveal in Finder", icon = "folder-open" },
	"separator",
	{ id = "delete", label = "Move to Trash", icon = "trash-2", shortcut = "cmd-backspace", danger = true },
}

local PICKABLE = { 1, 2, 3, 5 }

return mod {
	init = function(previous)
		return previous
			or {
				files = { "README.md", "Cargo.toml", "src/main.rs" },
				menu = "src/main.rs" :: string?,
				highlighted = 1,
				log = "Right-click a file",
			}
	end,

	update = function(state, msg)
		local function run(action: string)
			local file = state.menu
			state.menu = nil
			if not file then
				return
			end
			if action == "delete" then
				local index = table.find(state.files, file)
				if index then
					table.remove(state.files, index)
				end
				state.log = `Moved {file} to the Trash`
			else
				state.log = `{action}: {file}`
			end
		end
		if msg == "close-menu" then
			state.menu = nil
		elseif msg == "menu-up" or msg == "menu-down" then
			local at = table.find(PICKABLE, state.highlighted) or 1
			at = math.clamp(at + (if msg == "menu-up" then -1 else 1), 1, #PICKABLE)
			state.highlighted = PICKABLE[at]
		elseif msg == "menu-choose" then
			run(ITEMS[state.highlighted].id)
		elseif msg.menu then
			state.menu = msg.menu
			state.highlighted = 1
		elseif msg.pick then
			run(msg.pick)
		end
	end,

	view = function(state)
		local rows = {}
		for _, file in state.files do
			table.insert(
				rows,
				context_menu {
					open = state.menu == file,
					context = { menu = file },
					dismiss = "close-menu",
					keys = { up = "menu-up", down = "menu-down", enter = "menu-choose" },
					highlighted = state.highlighted,
					items = ITEMS,
					ui.item { file, detail = "~/code/ledger-api" },
				}
			)
		end
		return ui.column {
			gap = "normal",
			ui.column { gap = "none", width = 280, table.unpack(rows) },
			ui.text { state.log, tone = "muted", size = "small" },
			ui.row { height = 160 },
		}
	end,
}
