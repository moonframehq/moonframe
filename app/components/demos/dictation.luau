-- The button in each state, with `speech` standing in for `app.speech`: a
-- preview never touches the microphone.
local dictation = use "dictation"

return mod {
	view = function()
		return ui.row {
			gap = "large",
			align = "center",
			dictation { id = "idle", speech = { available = true, state = "idle" } },
			dictation { id = "recording", speech = { available = true, state = "recording", level = 0.6 } },
			dictation { id = "transcribing", speech = { available = true, state = "transcribing" } },
			dictation { id = "none", speech = { available = false, state = "idle", reason = "No speech engine is ready" } },
		}
	end,
}
