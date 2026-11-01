-- The bar over the page: the sidebar toggle, where you are (the page and
-- its tab, heard on the `hearth/tab` topic), and Connect on the right.

local breadcrumbs = use "breadcrumbs"
local icon_button = use "icon-button"

type State = { tab: string }

local TABS = { analytics = "Analytics", pulls = "Pull requests", ci = "CI", issues = "Issues", settings = "Settings" }

return mod {
	title = "Top bar",
	subscribe = { "hearth/tab" },

	styles = {
		[".top"] = { padding = { 0, 16, 0, 12 }, gap = 10, border = "bottom", border_color = "border" },
		[".top-rule"] = { background = "border" },
		[".top-connect"] = { radius = 7, border = true, border_color = "border", background = "card", padding = { 5, 11, 5, 9 }, gap = 7 },
		[".top-connect:hover"] = { background = "raised" },
		[".breadcrumbs-current"] = { color = "foreground" },
	},

	init = function(previous: State?): State
		return previous or { tab = "analytics" }
	end,

	update = function(state: State, msg)
		if type(msg) == "table" then
			if msg.topic == "hearth/tab" then
				state.tab = msg.value
			elseif msg.crumb == 1 then
				return state, { { to = "review", msg = { tab = "analytics" } } }
			end
		end
	end,

	view = function(state: State)
		return ui.row {
			class = "top",
			drag = true,
			grow = true,
			height = 61,
			align = "center",
			icon_button { icon = "panel-left", tooltip = "Toggle sidebar", msg = "toggle-left", to = "shell", size = "small" },
			ui.row { class = "top-rule", width = 1, height = 18 },
			breadcrumbs { crumbs = { { label = "Code Review" }, { label = TABS[state.tab] or "Analytics" } } },
			ui.spacer(),
			ui.row {
				class = "top-connect",
				align = "center",
				msg = "connect",
				tooltip = "Connect a repository host",
				ui.icon { name = "plug", size = "small", tone = "accent" },
				ui.text "Connect",
			},
		}
	end,
}
