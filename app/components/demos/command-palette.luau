-- A command palette over a few commands: type to filter, ↑↓ to move, ↩ to
-- run, Esc to close. The segmented control switches its look; the button
-- opens it again once closed.
local command_palette = use "command-palette"
local segmented = use "segmented"

local COMMANDS: { any } = {
	{ id = "new-tab", label = "New terminal tab", icon = "terminal", shortcut = "cmd-t", group = "Terminal" },
	{ id = "split", label = "Split pane right", icon = "columns-2", shortcut = "cmd-d", group = "Terminal" },
	{ id = "file", label = "Go to file…", icon = "file-text", shortcut = "cmd-p", group = "Navigate" },
	{ id = "workspace", label = "Switch workspace…", icon = "layout-dashboard", group = "Navigate" },
	{ id = "branch", label = "Switch branch…", icon = "git-branch", detail = "main", group = "Git" },
	{ id = "commit", label = "Commit all changes", icon = "git-commit-horizontal", detail = "14 files", group = "Git" },
	{ id = "push", label = "Push to origin", icon = "upload", keywords = "publish remote", group = "Git" },
	{ id = "theme", label = "Toggle light and dark", icon = "sun", shortcut = "cmd-shift-l", group = "View" },
	{ id = "reload", label = "Reload window", icon = "refresh-cw", shortcut = "cmd-r", group = "View" },
}

return mod {
	init = function(previous)
		return previous or { open = true, look = "spotlight", ran = nil }
	end,

	update = function(state, msg)
		if msg == "open" then
			state.open = true
		elseif msg == "close" then
			state.open = false
		elseif type(msg) == "table" and msg.run then
			state.ran = msg.run
			state.open = false
		elseif type(msg) == "table" and msg.select then
			state.look = msg.select
			state.open = true
		end
	end,

	view = function(state)
		return ui.column {
			gap = "normal",
			ui.row {
				gap = "normal",
				align = "center",
				ui.button { "Command palette", icon = "command", variant = "outline", msg = "open" },
				segmented {
					selected = state.look,
					small = true,
					options = {
						{ id = "spotlight", label = "Spotlight" },
						{ id = "classic", label = "Classic" },
						{ id = "compact", label = "Compact" },
						{ id = "cards", label = "Cards" },
					},
				},
			},
			ui.text { if state.ran then `Ran {state.ran}` else "⌘K in an app", tone = "muted", size = "small" },
			command_palette {
				id = "demo",
				open = state.open,
				look = state.look,
				dismiss = "close",
				commands = COMMANDS,
			},
		}
	end,
}
