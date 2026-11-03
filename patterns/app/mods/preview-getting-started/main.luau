local getting_started = use "getting-started"
local function loose(items: { any }): { any } return items end

return mod {
	init = function() return { expanded = "source", connected = false } end,
	update = function(state, msg)
		if type(msg) == "table" and msg.step then state.expanded = if state.expanded == msg.step then nil else msg.step
		elseif msg == "connect" then state.connected = not state.connected end
	end,
	view = function(state)
		return getting_started {
			title = "Make yourself at home", expanded = state.expanded,
			steps = loose {
				{ id = "workspace", title = "Create your workspace", done = true },
				{ id = "invite", title = "Invite a teammate", done = true, detail = "Share a space with the people you work with." },
				{ id = "source", title = "Connect your first source", done = state.connected,
					detail = "Sample interaction; this does not connect an account.",
					content = ui.button { if state.connected then "Reset example" else "Connect GitHub", icon = "brand-github", msg = "connect" },
				},
				{ id = "workflow", title = "Run your first workflow", detail = "Start small: one trigger and one useful action." },
			},
		}
	end,
}
