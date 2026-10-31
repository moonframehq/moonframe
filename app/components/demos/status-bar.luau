-- A window footer: click the branch to switch it, "needs you" to clear it,
-- Theme to flip it; the switch adds separators.
local status_bar = use "status-bar"
local switch = use "switch"

local BRANCHES = { "main", "feat/multi-currency", "fix/rounding" }

return mod {
	init = function(previous)
		return previous or { branch = 2, blocked = 1, dark = true, separators = false, shortcuts = false }
	end,

	update = function(state, msg)
		if msg == "branch" then
			state.branch = state.branch % #BRANCHES + 1
		elseif msg == "show-blocked" then
			state.blocked = 0
		elseif msg == "toggle-theme" then
			state.dark = not state.dark
		elseif msg == "shortcuts" then
			state.shortcuts = not state.shortcuts
		elseif msg == "separators" then
			state.separators = not state.separators
		end
	end,

	view = function(state)
		local left: { any } = {
			{ icon = "git-branch", text = BRANCHES[state.branch], mono = true, msg = "branch", tooltip = "Switch branch" },
			{ icon = "bot", text = "4 agents" },
		}
		if state.blocked > 0 then
			left[3] = { text = `{state.blocked} needs you`, tone = "danger", msg = "show-blocked" }
		else
			left[3] = { icon = "check", text = "all clear", tone = "success" }
		end
		return ui.column {
			gap = "large",
			switch { label = "Separators", on = state.separators, msg = "separators", small = true },
			status_bar {
				separators = state.separators,
				left = left,
				right = {
					{ icon = "cpu", text = "27%" },
					{ icon = if state.dark then "moon" else "sun", text = if state.dark then "Dark" else "Light", msg = "toggle-theme" },
					{ icon = "keyboard", text = "Shortcuts", tone = if state.shortcuts then "accent" else nil, msg = "shortcuts" },
				} :: { any },
			},
		}
	end,
}
