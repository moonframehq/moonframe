-- Reacts to each publish on provider/clock, and never reads app.clock.
return mod {
	subscribe = { "provider/clock" },
	init = function(previous)
		return previous or { publishes = 0, last = "none" }
	end,
	update = function(state, msg)
		if type(msg) == "table" and msg.topic == "provider/clock" then
			state.publishes += 1
			state.last = msg.value.time
		end
	end,
	view = function(state)
		return ui.text { `{state.publishes} {state.last}` }
	end,
}
