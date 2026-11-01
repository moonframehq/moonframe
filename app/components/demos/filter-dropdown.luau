-- Three filters over an activity log: kinds (several at once), one author,
-- and a period. A chip with a filter on turns accent; its × clears it.
local filter_dropdown = use "filter-dropdown"

local KINDS: { any } = {
	{ id = "orders", label = "Orders", icon = "file-text" },
	{ id = "products", label = "Products", icon = "layers" },
	{ id = "customers", label = "Customers", icon = "user" },
	{ id = "apps", label = "Apps", icon = "plug" },
}

local AUTHORS: { any } = {
	{ id = "nadia", label = "Nadia Okafor" },
	{ id = "theo", label = "Theo Lind" },
	{ id = "priya", label = "Priya Raman" },
}

local PERIODS: { any } = {
	{ id = "1", label = "Last 24 hours" },
	{ id = "7", label = "Last 7 days" },
	{ id = "30", label = "Last 30 days" },
	{ id = "all", label = "All time" },
}

type State = { open: string?, kinds: { string }, author: string, period: string }

return mod {
	init = function(previous: State?): State
		return previous or { open = "kinds", kinds = { "orders" }, author = "", period = "7" }
	end,

	update = function(state: State, msg)
		if msg == "close" then
			state.open = nil
		elseif type(msg) ~= "table" then
			return
		elseif msg.open then
			state.open = if state.open == msg.open then nil else msg.open
		elseif msg.kind then
			-- Several kinds: a pick toggles one and the menu stays open; the
			-- "Any" row ("") clears them.
			if msg.kind == "" then
				state.kinds = {}
			else
				local at = table.find(state.kinds, msg.kind)
				if at then
					table.remove(state.kinds, at)
				else
					table.insert(state.kinds, msg.kind)
				end
			end
		elseif msg.author then
			state.author = msg.author
			state.open = nil
		elseif msg.period then
			state.period = msg.period
			state.open = nil
		elseif msg.clear then
			if msg.clear == "kinds" then
				state.kinds = {}
			else
				state.author = ""
			end
		end
	end,

	view = function(state: State)
		return ui.column {
			gap = "normal",
			ui.row {
				gap = "normal",
				align = "center",
				filter_dropdown {
					icon = "square-check",
					placeholder = "Any activities",
					any = "Any activities",
					multiple = true,
					options = KINDS,
					selected = state.kinds,
					event = "kind",
					open = state.open == "kinds",
					toggle = { open = "kinds" },
					dismiss = "close",
					clear = { clear = "kinds" },
				},
				filter_dropdown {
					icon = "user",
					placeholder = "Any author",
					any = "Any author",
					options = AUTHORS,
					selected = state.author,
					event = "author",
					open = state.open == "author",
					toggle = { open = "author" },
					dismiss = "close",
					clear = { clear = "author" },
				},
				-- A period is always set, so it keeps the resting look.
				filter_dropdown {
					icon = "calendar",
					options = PERIODS,
					selected = state.period,
					plain = true,
					event = "period",
					open = state.open == "period",
					toggle = { open = "period" },
					dismiss = "close",
				},
			},
			ui.row { height = 200 },
		}
	end,
}
