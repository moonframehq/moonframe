-- Publishes whatever it is told to.
return mod {
	update = function(state, msg)
		return state, { { to = "topic:" .. msg.topic, msg = msg.value } }
	end,
	view = function()
		return ui.text "nav"
	end,
}
