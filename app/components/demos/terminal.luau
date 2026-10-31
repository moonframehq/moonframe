-- Two shells behind a switch: pick one; the other keeps running while
-- hidden. (In the component browser a terminal is a placeholder.)
local terminal = use "terminal"
local segmented = use "segmented"

return mod {
	init = function(previous)
		return previous or { shell = "home" }
	end,

	update = function(state, msg)
		if typeof(msg) == "table" and msg.select then
			state.shell = msg.select
		end
	end,

	view = function(state)
		return ui.column {
			gap = "small",
			width = 560,
			height = 300,
			segmented {
				selected = state.shell,
				small = true,
				options = {
					{ id = "home", label = "~", icon = "square-terminal" },
					{ id = "project", label = "ledger-api", icon = "git-branch" },
				},
			},
			-- One id per shell: two fixed ids, two shells.
			ui.column {
				grow = true,
				if state.shell == "home"
					then terminal { id = "home", cwd = "~" }
					else terminal { id = "project", cwd = "~/code/ledger-api" },
			},
		}
	end,
}
