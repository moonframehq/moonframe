-- A config problem: Show more / Show less folds the text, × hides it, Open
-- config.toml is its action. Pick a tone.
local notice = use "notice"
local segmented = use "segmented"

return mod {
	init = function(previous)
		return previous or { shown = true, expanded = false, tone = "warning", opened = false }
	end,

	update = function(state, msg)
		if type(msg) == "table" and msg.toggle then
			state.expanded = not state.expanded
		elseif type(msg) == "table" and msg.select then
			state.tone = msg.select
		elseif msg == "hide" then
			state.shown = false
		elseif msg == "show" then
			state.shown = true
		elseif msg == "open-config" then
			state.opened = true
		end
	end,

	view = function(state)
		return ui.column {
			gap = "normal",
			segmented {
				selected = state.tone,
				small = true,
				options = {
					{ id = "info", label = "Info" },
					{ id = "success", label = "Success" },
					{ id = "warning", label = "Warning" },
					{ id = "danger", label = "Danger" },
				},
			},
			if state.shown
				then notice {
					id = "config",
					tone = state.tone,
					title = "config.toml has 2 problems",
					text = "Line 14: unknown key `sidebar.density`; it is ignored. Line 31: `theme = \"nord-light\"` is not an installed theme, so the default theme is used until you fix it or install it.",
					foldable = true,
					expanded = state.expanded,
					action = { label = "Open config.toml", msg = "open-config" },
					dismiss = "hide",
				}
				else ui.row { ui.button { "Show notice", small = true, variant = "outline", msg = "show" } },
			state.opened and ui.text { "Opened ~/.config/moonframe/settings.toml", tone = "muted", size = "small" },
		}
	end,
}
