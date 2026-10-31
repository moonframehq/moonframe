-- A shell picker: click the button to open or close it, pick a shell, or
-- use ↑ ↓ and Enter while it is open; Escape or a click outside closes it.
local dropdown = use "dropdown"

local OPTIONS: { any } = {
	{ heading = "Installed" },
	{ id = "zsh", label = "zsh", detail = "/bin/zsh" },
	{ id = "fish", label = "fish", detail = "/opt/homebrew/bin/fish" },
	{ id = "bash", label = "bash", detail = "/bin/bash" },
	{ id = "nu", label = "nushell", detail = "/opt/homebrew/bin/nu" },
	"separator",
	{ id = "custom", label = "Custom…", icon = "settings", shortcut = "cmd-," },
}

-- Indexes of the options a key can land on (not headings or separators).
local PICKABLE = {}
for index, option in OPTIONS do
	if type(option) == "table" and option.id then
		table.insert(PICKABLE, index)
	end
end

return mod {
	init = function(previous)
		return previous or { open = true, shell = "zsh", highlighted = 2 }
	end,

	update = function(state, msg)
		if msg == "toggle-shell" then
			state.open = not state.open
		elseif msg == "close-shell" then
			state.open = false
		elseif msg == "up" or msg == "down" then
			local at = table.find(PICKABLE, state.highlighted) or 1
			at = math.clamp(at + (if msg == "up" then -1 else 1), 1, #PICKABLE)
			state.highlighted = PICKABLE[at]
		elseif msg == "choose" then
			state.shell = OPTIONS[state.highlighted].id
			state.open = false
		elseif msg.pick then
			state.shell = msg.pick
			state.open = false
		end
	end,

	view = function(state)
		return ui.column {
			gap = "normal",
			dropdown {
				label = "Shell",
				icon = "terminal",
				open = state.open,
				toggle = "toggle-shell",
				dismiss = "close-shell",
				keys = { up = "up", down = "down", enter = "choose" },
				highlighted = state.highlighted,
				selected = state.shell,
				width = 270,
				options = OPTIONS,
			},
			ui.row { height = 200 },
		}
	end,
}
