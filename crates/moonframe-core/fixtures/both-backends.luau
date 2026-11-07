-- One shell both renderers run, in `crates/moonframe/src/e2e/both_backends.rs`
-- and `crates/moonframe-rune/src/tests.rs`: a real component and a field, so
-- a change that lets the backends drift apart fails on the one that drifted.
local collapsible = use "collapsible"

return mod {
	init = function(previous)
		return previous or { open = false, name = "" }
	end,

	update = function(state, msg)
		if type(msg) ~= "table" then
			return
		end
		if msg.toggle then
			state.open = not state.open
		elseif msg.msg == "name" then
			state.name = msg.value
		end
	end,

	view = function(state)
		return ui.column {
			grow = true,
			collapsible {
				title = "Details",
				id = "details",
				open = state.open,
				ui.text { "The body shows" },
			},
			ui.row {
				class = "both-name",
				ui.input { id = "name", change = "name", placeholder = "Name", grow = true },
			},
			ui.text { `Hello {state.name}` },
		}
	end,
}
