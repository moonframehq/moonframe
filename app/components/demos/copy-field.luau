-- Text to paste elsewhere: click Copy to put it on the clipboard; the
-- button shows "Copied" until you copy the other field.
local copy_field = use "copy-field"

return mod {
	init = function(previous)
		return previous or { copied = "path" :: string? }
	end,

	update = function(state, msg)
		if msg.copied then
			state.copied = msg.copied
		end
	end,

	view = function(state)
		return ui.column {
			gap = "normal",
			copy_field {
				label = "Install",
				text = "curl -fsSL https://moonframe.dev/install.sh | sh",
				copied = state.copied == "install",
				msg = { copied = "install" },
			},
			copy_field {
				label = "Worktree",
				text = "~/code/worktrees/ledger-api/fix-tab-focus",
				button = "Copy path",
				copied = state.copied == "path",
				msg = { copied = "path" },
			},
		}
	end,
}
