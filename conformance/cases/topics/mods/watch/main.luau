-- Hears every topic under tabs/ by a prefix.
return mod {
	subscribe = { "tabs/*" },
	init = function(previous)
		return previous or { heard = {} }
	end,
	update = function(state, msg)
		if type(msg) == "table" and msg.topic then
			table.insert(state.heard, `{msg.topic}={msg.value}`)
		end
	end,
	view = function(state)
		return ui.text { table.concat(state.heard, " ") }
	end,
}
