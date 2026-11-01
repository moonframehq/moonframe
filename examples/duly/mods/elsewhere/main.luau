-- The pages this sample does not fill (Rules, Performance, Notices, Public
-- record, Settings, new views): a title and an empty state that leads back
-- to Today. Mounted by the shell as an instance per page.

local empty_state = use "empty-state"

return mod {
	title = "Elsewhere",

	update = function(state, msg)
		if msg == "today" then
			return state, { { to = "sidebar", msg = { page = "today" } } }
		end
	end,

	view = function(_state, _app, props)
		local title = props and props.title or "Page"
		return ui.column {
			grow = true,
			gap = "none",
			style = { padding = { 34, 30, 30, 30 } },
			ui.row { drag = true, ui.text { title, size = "display", weight = "medium" } },
			ui.column {
				grow = true,
				align = "center",
				justify = "center",
				empty_state {
					icon = "inbox",
					title = `{title} is not in this sample`,
					message = "Duly shows sample data for Today, Contracts, Payments and Reports.",
					action = "Back to Today",
					msg = "today",
				},
			},
		}
	end,
}
