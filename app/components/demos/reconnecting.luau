-- A dropped SSH host: Reconnect now counts attempts and the third one
-- connects; Work offline closes it. Drop connection brings it back.
local reconnecting = use "reconnecting"

return mod {
	init = function(previous)
		return previous or { open = true, attempt = 1, note = "" }
	end,

	update = function(state, msg)
		if msg == "reconnect" then
			state.attempt += 1
			if state.attempt > 3 then
				state.open = false
				state.note = "Connected to devbox."
			end
		elseif msg == "offline" then
			state.open = false
			state.note = "Working offline."
		elseif msg == "drop" then
			state.open = true
			state.attempt = 1
			state.note = ""
		end
	end,

	view = function(state)
		return ui.column {
			gap = "normal",
			ui.row {
				gap = "normal",
				align = "center",
				ui.button { "Drop connection", icon = "wifi-off", small = true, variant = "outline", msg = "drop" },
				state.note ~= "" and ui.text { state.note, tone = "muted", size = "small" },
			},
			reconnecting {
				open = state.open,
				host = "devbox",
				reason = "The SSH connection closed: Connection reset by peer.",
				status = `Next attempt in 4s · attempt {state.attempt}`,
				retry = "reconnect",
				secondary = { label = "Work offline", msg = "offline" },
			},
		}
	end,
}
