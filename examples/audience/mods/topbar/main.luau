-- The bar over the page: where you are, then search, notifications and
-- your avatar. The crumbs follow the sidebar's page, and Learn's course or
-- guide when one is open (a click on "Learn" goes back to its page).

local breadcrumbs = use "breadcrumbs"
local icon_button = use "icon-button"
local avatar = use "avatar"

type State = { query: string, page: string, learn: any }

-- The pages' names, for the last crumb.
local PAGES = {
	overview = "Overview",
	audience = "Audience",
	campaigns = "Campaigns",
	content = "Content",
	engagement = "Engagement",
	revenue = "Revenue",
	subscribers = "Subscribers",
	automations = "Automations",
	segments = "Segments",
	integrations = "Integrations",
	analytics = "Analytics",
	reports = "Reports",
	learn = "Learn",
	help = "Help Center",
	settings = "System Settings",
}

return mod {
	region = "top",
	title = "Top bar",
	subscribe = { "inkwell/page", "inkwell/learn" },

	styles = {
		[".top"] = { padding = { 0, 18, 0, 18 }, gap = 8 },
		[".top-search"] = { radius = 10, border = true, border_color = "border", background = "background", padding = { 0, 10 }, gap = 6 },
		[".top-search-input"] = { background = "#00000000", border = "none", shadow = "none", padding = { 5, 0 } },
		[".top-box"] = { radius = 10, border = true, border_color = "border", background = "background" },
	},

	init = function(previous: State?): State
		local state: State = previous or { query = "", page = "audience" }
		state.page = state.page or "audience"
		return state
	end,

	update = function(state: State, msg)
		if type(msg) ~= "table" then
			return
		end
		if msg.msg == "query" then
			state.query = msg.value
		elseif msg.topic == "inkwell/page" then
			state.page = msg.value
		elseif msg.topic == "inkwell/learn" then
			state.learn = msg.value
		elseif msg.crumb == "learn" then
			return state, { { to = "learn", msg = { screen = "home" } } }
		end
		return state
	end,

	view = function(state: State)
		local crumbs: { any } = { { label = "Workspace", icon = "layout-dashboard", id = "workspace" } }
		local learn = state.learn
		if state.page == "learn" and learn and learn.title then
			table.insert(crumbs, { label = "Learn", id = "learn" })
			table.insert(crumbs, { label = learn.title })
		else
			table.insert(crumbs, { label = PAGES[state.page] or "Audience" })
		end
		return ui.row {
			class = "top",
			drag = true,
			grow = true,
			height = 64,
			align = "center",
			breadcrumbs { crumbs = crumbs },
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
