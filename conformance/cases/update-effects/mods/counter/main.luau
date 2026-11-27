-- update changing the state in place, replacing it, and sending on; init
-- sending once loaded.
return mod {
	init = function(previous)
		return previous or { count = 0 }, { send "status" { count = 0, from = "init" } }
	end,

	update = function(state, msg)
		if msg == "add" then
			state.count += 1
		elseif msg == "reset" then
			return { count = 100 }
		elseif msg == "tell" then
			return state, { send "status" { count = state.count, from = "update" }, { to = "status", msg = "ping" } }
		end
	end,

	view = function(state)
		return ui.row { ui.text { `{state.count}` }, ui.button { "Add", msg = "add" } }
	end,
}
