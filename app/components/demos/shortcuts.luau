-- A shortcut reference: click a row to run it (shown below), Esc or ×
-- closes; the switch draws it inline instead of as a modal.
local shortcuts = use "shortcuts"
local switch = use "switch"

local function item(keys: string, label: string)
	return { keys = keys, label = label, msg = { run = label } }
end

local GROUPS = {
	{
		title = "Workspaces & panes",
		items = {
			item("cmd-shift-n", "New workspace"),
			item("cmd-t", "New tab"),
			item("cmd-d", "Split right"),
			item("cmd-shift-d", "Split down"),
			item("cmd-w", "Close pane"),
		},
	},
	{
		title = "Navigation",
		items = {
			item("cmd-k", "Command palette"),
			item("cmd-p", "Go to workspace"),
			item("cmd-alt-left", "Previous pane"),
			item("cmd-alt-right", "Next pane"),
		},
	},
	{
		title = "Agents",
		items = {
			item("cmd-shift-a", "Start agent"),
			item("cmd-k cmd-r", "Review changes"),
			item("", "Edit worktree note"),
		},
	},
}

return mod {
	init = function(previous)
		return previous or { open = true, inline = false, ran = nil }
	end,

	update = function(state, msg)
		if msg == "open" then
			state.open = true
		elseif msg == "close" then
			state.open = false
		elseif msg == "toggle-inline" then
			state.inline = not state.inline
		elseif type(msg) == "table" and msg.run then
			state.ran = msg.run
		end
	end,

	view = function(state)
		return ui.column {
			gap = "normal",
			ui.row {
				gap = "normal",
				align = "center",
				ui.button { "Keyboard shortcuts", icon = "keyboard", small = true, variant = "outline", msg = "open" },
				switch { on = state.inline, label = "Inline", small = true, msg = "toggle-inline" },
				state.ran ~= nil and ui.text { `Ran: {state.ran}`, tone = "muted", size = "small" },
			},
			shortcuts {
				open = state.open or state.inline,
				inline = state.inline,
				dismiss = "close",
				width = 600,
				subtitle = "Click one to run it",
				groups = GROUPS,
			},
		}
	end,
}
