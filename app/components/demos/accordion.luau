-- A merge request's sections: click a header to open it. "One" closes the
-- others; "Many" lets several stay open.
local accordion = use "accordion"
local segmented = use "segmented"

local function check(icon: string, tone: string, text: string, spin: boolean?)
	return ui.row {
		gap = "small",
		align = "center",
		ui.icon { name = icon :: any, tone = tone :: any, spin = spin },
		ui.text { text, size = "small" },
	}
end

return mod {
	init = function(previous)
		return previous or { mode = "one", open = "checks", many = { checks = true } }
	end,

	update = function(state, msg)
		if typeof(msg) ~= "table" then
			return
		end
		if msg.mode then
			state.mode = msg.mode
		elseif msg.toggle then
			if state.mode == "one" then
				state.open = if state.open == msg.toggle then nil else msg.toggle
			else
				state.many[msg.toggle] = not state.many[msg.toggle]
			end
		end
	end,

	view = function(state)
		return ui.column {
			gap = "normal",
			width = 440,
			segmented {
				event = "mode",
				selected = state.mode,
				small = true,
				options = { { id = "one", label = "One open" }, { id = "many", label = "Many open" } },
			},
			accordion {
				open = if state.mode == "one" then state.open else state.many,
				items = {
					{
						id = "checks",
						title = "Checks",
						summary = "3 of 4 passed",
						icon = "circle-check",
						body = {
							check("check", "success", "lint · cargo clippy"),
							check("check", "success", "test · cargo nextest"),
							check("check", "success", "build · release"),
							check("loader-circle", "warning", "deploy · preview", true),
						},
					},
					{
						id = "files",
						title = "Files changed",
						summary = "2",
						icon = "file-text",
						body = { ui.text { "src/ledger.rs  +12 −3", size = "small" }, ui.text { "src/currency.rs  +40", size = "small" } },
					},
					{ id = "activity", title = "Activity", summary = "updated 3d ago", icon = "calendar", body = ui.text { "penso opened this 7d ago", size = "small", tone = "muted" } },
				},
			},
		}
	end,
}
