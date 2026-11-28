-- Reads a provider's data, and asks it for something.
return mod {
	update = function(state, msg)
		if msg == "ask" then
			return state, { send "provider:clock" { zone = "UTC" } }
		end
	end,
	view = function(state, app)
		local clock = app.clock
		return ui.column {
			ui.text { if clock then clock.time else "waiting" },
			ui.button { "Ask", msg = "ask" },
		}
	end,
}
