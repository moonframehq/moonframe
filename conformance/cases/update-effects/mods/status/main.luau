return mod {
	init = function(previous)
		return previous or { heard = {} }
	end,

	update = function(state, msg)
		if type(msg) == "table" then
			table.insert(state.heard, `{msg.from}:{msg.count}`)
		else
			table.insert(state.heard, msg)
		end
	end,

	view = function(state)
		return ui.text { table.concat(state.heard, ","), tone = "muted" }
	end,
}
