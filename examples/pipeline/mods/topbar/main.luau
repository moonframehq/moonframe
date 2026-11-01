-- The bar over the sheet: the sidebar toggle and where you are, then the
-- bell, a search field, the agent panel's toggle and "New chat".

local breadcrumbs = use "breadcrumbs"
local icon_button = use "icon-button"
local kbd = use "kbd"

type State = { query: string, focus: boolean }

return mod {
	region = "top",
	title = "Top bar",

	styles = {
		[".bar"] = { padding = { 0, 14, 0, 8 }, gap = 8 },
		[".bar-search"] = { background = "background", border = true, border_color = "border", radius = 9, padding = { 0, 10 }, gap = 8 },
		[".bar-search-icon"] = { color = "muted-foreground" },
		[".bar-field"] = { background = "#00000000", border = "none", shadow = "none", padding = { 5, 0 } },
		[".bar-new"] = { padding = { 6, 12 }, gap = 6 },
	},

	init = function(previous: State?): State
		return { query = previous and previous.query or "", focus = false }
	end,

	update = function(state: State, msg)
		if msg == "focus" then
			state.focus = true
		elseif type(msg) == "table" and msg.msg == "query" then
			state.query = msg.value
			state.focus = false
			return state, { { to = "board", msg = { query = msg.value } } }
		elseif type(msg) == "table" and msg.msg == "clear" then
			state.query = ""
			return state, { { to = "board", msg = { query = "" } } }
		end
	end,

	view = function(state: State, app)
		return ui.row {
			class = "bar",
			drag = true,
			grow = true,
			height = 52,
			align = "center",
			icon_button { icon = "panel-left", tooltip = "Show or hide the sidebar", msg = "toggle-left", to = "shell" },
			breadcrumbs { crumbs = { { label = "Sales" }, { label = "Pipeline" }, { label = "Q4 2026" } } },
			ui.spacer(),
			icon_button { icon = "bell", tooltip = "Notifications", msg = "focus" },
			ui.row {
				class = "bar-search",
				width = 260,
				height = 34,
				align = "center",
				ui.icon { name = "search", size = "small", class = "bar-search-icon" },
				ui.input {
					id = "search",
					value = state.query,
					placeholder = "Search deals",
					change = "query",
					keys = { escape = "clear" },
					focus = state.focus,
					grow = true,
					class = "bar-field",
				},
				kbd { keys = "cmd-k", small = true, plain = true },
			},
			icon_button { icon = "panel-right", tooltip = "Show or hide the agent", msg = "toggle-agent", to = "shell" },
			icon_button { icon = "history", tooltip = "Earlier chats (not in this example)", msg = "focus" },
			ui.row {
				class = { "dark-button", "bar-new" },
				align = "center",
				msg = "reset",
				to = "agent",
				ui.icon { name = "message-square", size = "small" },
				ui.text { "New chat", weight = "medium" },
			},
		}
	end,
}
