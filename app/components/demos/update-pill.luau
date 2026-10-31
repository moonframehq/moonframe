-- An update offer: Restart installs it, What changed shows the notes, ×
-- hides it. The switch toggles the downloading state.
local update_pill = use "update-pill"
local switch = use "switch"

return mod {
	init = function(previous)
		return previous or { shown = true, downloading = false, status = "" }
	end,

	update = function(state, msg)
		if msg == "restart" then
			state.status = "Restarting into Moonframe 1.4…"
		elseif msg == "show-notes" then
			state.status = "Release notes for 1.4 opened."
		elseif msg == "later" then
			state.shown = false
			state.status = "Reminder snoozed until tomorrow."
		elseif msg == "show" then
			state.shown = true
			state.status = ""
		elseif msg == "toggle-downloading" then
			state.downloading = not state.downloading
		end
	end,

	view = function(state)
		return ui.column {
			gap = "normal",
			ui.row { switch { on = state.downloading, label = "Still downloading", small = true, msg = "toggle-downloading" } },
			if state.shown
				then ui.row {
					update_pill {
						name = "Moonframe",
						version = "1.4",
						downloading = state.downloading,
						notes = "show-notes",
						dismiss = "later",
					},
				}
				else ui.row { ui.button { "Show again", small = true, variant = "outline", msg = "show" } },
			state.status ~= "" and ui.text { state.status, tone = "muted", size = "small" },
		}
	end,
}
