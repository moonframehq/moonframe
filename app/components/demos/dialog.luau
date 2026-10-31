-- Remove a worktree: the button opens the dialog; Cancel, ×, Esc or ⌘↩ close it.
-- The segmented control switches the dialog's size.
local dialog = use "dialog"
local segmented = use "segmented"

return mod {
	init = function(previous)
		return previous or { open = true, size = "medium", removed = false }
	end,

	update = function(state, msg)
		if msg == "open" then
			state.open = true
			state.removed = false
		elseif msg == "close" then
			state.open = false
		elseif msg == "remove" then
			state.open = false
			state.removed = true
		elseif type(msg) == "table" and msg.select then
			state.size = msg.select
		end
	end,

	view = function(state)
		return ui.column {
			gap = "normal",
			ui.row {
				gap = "normal",
				align = "center",
				ui.button { "Remove worktree…", icon = "trash-2", variant = "outline", msg = "open" },
				segmented {
					selected = state.size,
					small = true,
					options = {
						{ id = "small", label = "Small" },
						{ id = "medium", label = "Medium" },
						{ id = "large", label = "Large" },
					},
				},
			},
			ui.text {
				if state.removed then "feature/inspect removed." else "feature/inspect · ~/code/app-inspect",
				tone = "muted",
				size = "small",
			},
			dialog {
				open = state.open,
				dismiss = "close",
				size = state.size,
				title = "Remove worktree",
				subtitle = "feature/inspect · ~/code/app-inspect",
				icon = "folder-git-2",
				submit = "remove",
				note = "⌘↩ to confirm",
				buttons = {
					{ label = "Cancel", msg = "close", variant = "ghost" },
					{ label = "Remove worktree", msg = "remove", variant = "danger" },
				},
				ui.text { "The worktree folder and its untracked files are deleted. The branch stays.", size = "small" },
				ui.text { "3 files changed, 1 untracked", size = "small", tone = "muted" },
			},
		}
	end,
}
