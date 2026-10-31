-- Banners in each tone: × dismisses one, Restart fixes the provider, and
-- "Show all" brings them back. The switch fills them solid.
local banner = use "banner"
local switch = use "switch"

local function fresh()
	return { branch = true, provider = true, update = true, saved = true }
end

return mod {
	init = function(previous)
		return previous or { shown = fresh(), solid = false, restarted = false }
	end,

	update = function(state, msg)
		if msg == "solid" then
			state.solid = not state.solid
		elseif msg == "restart" then
			state.restarted = true
		elseif msg == "reset" then
			state.shown = fresh()
			state.restarted = false
		elseif typeof(msg) == "table" and msg.dismiss then
			state.shown[msg.dismiss] = false
		end
	end,

	view = function(state)
		local shown = state.shown
		return ui.column {
			gap = "normal",
			ui.row {
				align = "center",
				switch { label = "Solid", on = state.solid, msg = "solid", small = true },
				ui.spacer(),
				ui.button { "Show all", small = true, variant = "outline", msg = "reset" },
			},
			shown.branch and banner {
				text = "feat/multi-currency",
				detail = "worktree of ledger-api",
				icon = "git-branch",
				tone = "warning",
				solid = state.solid,
				mono = true,
				dismiss = { dismiss = "branch" },
			},
			shown.provider and (if state.restarted
				then banner { text = "The git provider is running again", icon = "circle-check", tone = "success", solid = state.solid, dismiss = { dismiss = "provider" } }
				else banner {
					text = "The git provider stopped",
					detail = "exit status 1",
					icon = "triangle-alert",
					tone = "danger",
					solid = state.solid,
					action = { label = "Restart", msg = "restart" },
					dismiss = { dismiss = "provider" },
				}),
			shown.update and banner {
				text = "Version 0.9 is ready",
				detail = "restart to update",
				icon = "download",
				tone = "accent",
				solid = state.solid,
				dismiss = { dismiss = "update" },
			},
			shown.saved and banner { text = "Layout saved", icon = "info", tone = "muted", solid = state.solid, dismiss = { dismiss = "saved" } },
		}
	end,
}
