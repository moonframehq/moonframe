-- The bar over the page: where you are, then search, notifications and
-- your avatar.

local breadcrumbs = use "breadcrumbs"
local icon_button = use "icon-button"
local avatar = use "avatar"

type State = { query: string }

local CRUMBS: { any } = { { label = "Workspace", icon = "layout-dashboard" }, { label = "Audience" } }

return mod {
	region = "top",
	title = "Top bar",

	styles = {
		[".top"] = { padding = { 0, 18, 0, 18 }, gap = 8 },
		[".top-search"] = { radius = 10, border = true, border_color = "border", background = "background", padding = { 0, 10 }, gap = 6 },
		[".top-search-input"] = { background = "#00000000", border = "none", shadow = "none", padding = { 5, 0 } },
		[".top-box"] = { radius = 10, border = true, border_color = "border", background = "background" },
	},

	init = function(previous: State?): State
		return previous or { query = "" }
	end,

	update = function(state: State, msg)
		if type(msg) == "table" and msg.msg == "query" then
			state.query = msg.value
		end
	end,

	view = function(state: State)
		return ui.row {
			class = "top",
			drag = true,
			grow = true,
			height = 64,
			align = "center",
			breadcrumbs { crumbs = CRUMBS },
			ui.spacer(),
			ui.row {
				class = "top-search",
				width = 200,
				height = 36,
				align = "center",
				ui.icon { name = "search", size = "small", tone = "muted" },
				ui.input { id = "search", value = state.query, placeholder = "Search", change = "query", grow = true, class = "top-search-input" },
			},
			ui.row { class = "top-box", icon_button { icon = "bell", tooltip = "Notifications", msg = "bell" } },
			avatar { name = "Robin Vale", shape = "square" },
		}
	end,
}
