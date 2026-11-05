-- Click the field and press a shortcut: it shows as key caps. Escape gives
-- up, and the × clears it.
local shortcut_field = use "shortcut-field"

return mod {
	init = function(previous)
		return previous or { keys = "cmd-shift-p" :: string?, recording = false }
	end,

	update = function(state, msg)
		if msg == "record" then
			state.recording = true
		elseif msg == "stop" then
			state.recording = false
		elseif msg == "clear" then
			state.keys = nil
		elseif type(msg) == "table" and msg.keys then
			state.keys = msg.keys
			state.recording = false
		end
	end,

	view = function(state)
		return shortcut_field {
			value = state.keys, recording = state.recording,
			record = "record", change = "keys", cancel = "stop", clear = "clear",
		}
	end,
}
