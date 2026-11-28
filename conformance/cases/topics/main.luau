-- Hears the page on a topic.
return mod {
	subscribe = { "page" },
	init = function(previous)
		return previous or { page = "home" }
	end,
	update = function(state, msg)
		if type(msg) == "table" and msg.topic == "page" then
			state.page = msg.value
		end
	end,
	view = function(state)
		return ui.text { state.page }
	end,
}
