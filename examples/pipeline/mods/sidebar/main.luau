-- The sidebar: the workspace, its sections (Pipeline and Meetings are in
-- this example), the quarter's quota, and who is signed in. Picking a page
-- publishes it on `tally/page`, which the shell and the top bar follow.

local avatar = use "avatar"
local icon_button = use "icon-button"

type State = { selected: string }

-- The pages this example has; the other rows only light up.
local PAGES: { [string]: boolean } = { pipeline = true, meetings = true }

local SECTIONS = {
	{
		title = "Workspace",
		items = {
			{ id = "home", label = "Home", icon = "house" },
			{ id = "inbox", label = "Inbox", icon = "inbox" },
		},
	},
	{
		title = "Sales",
		items = {
			{ id = "pipeline", label = "Pipeline", icon = "layout-dashboard" },
			{ id = "deals", label = "Deals", icon = "building-2" },
			{ id = "contacts", label = "Contacts", icon = "user" },
			{ id = "companies", label = "Companies", icon = "building-2" },
			{ id = "meetings", label = "Meetings", icon = "calendar" },
			{ id = "activities", label = "Activities", icon = "activity" },
			{ id = "forecast", label = "Forecast", icon = "chart-pie" },
		},
	},
	{
		title = "Insights",
		items = {
			{ id = "reports", label = "Reports", icon = "chart-column" },
			{ id = "settings", label = "Settings", icon = "settings-2" },
		},
	},
}

local function money(value: number): string
	if value >= 1000000 then
		return string.format("$%.2fM", value / 1000000)
	end
	return string.format("$%dK", math.floor(value / 1000 + 0.5))
end

return mod {
	region = "left",
	title = "Sidebar",

	styles = {
		[".side"] = { padding = { 0, 10, 10, 12 }, gap = 2 },
		[".side-brand"] = { padding = { 10, 8 }, gap = 10, radius = 10 },
		[".side-brand:hover"] = { background = "list-hover" },
		[".side-logo"] = { background = "foreground", color = "background", radius = 8 },
		[".side-heading"] = { size = "small", weight = "medium", color = "muted-foreground", padding = { 14, 8, 6, 8 } },
		[".side-row"] = { padding = { 7, 10 }, radius = 8, gap = 10, color = "muted-foreground" },
		[".side-row:hover"] = { background = "list-hover" },
		[".side-row:selected"] = { background = "background", color = "foreground", shadow = "0 1px 2px #0000000f", border = true, border_color = "border" },
		[".side-label"] = { weight = "medium" },
		[".side-quota"] = { background = "background", radius = 12, border = true, border_color = "border", padding = 12, gap = 8 },
		[".side-track"] = { background = "secondary", radius = 3 },
		[".side-fill"] = { background = "accent", radius = 3 },
		[".side-small"] = { size = "small", color = "muted-foreground" },
		[".side-user"] = { padding = { 10, 4 }, gap = 10 },
	},

	init = function(previous: State?): State
		local state = previous or { selected = "pipeline" }
		-- Start the shell and the top bar on the page last shown.
		return state, { { to = "topic:tally/page", msg = if PAGES[state.selected] then state.selected else "pipeline" } }
	end,

	update = function(state: State, msg)
		if type(msg) == "table" and msg.select and PAGES[msg.select] then
			state.selected = msg.select
			return state, { { to = "topic:tally/page", msg = msg.select } }
		end
	end,

	view = function(state: State, app)
		local deals = app.deals
		local window = app.window
		local rows = {}
		for _, section in SECTIONS do
			table.insert(rows, ui.text { string.upper(section.title), class = "side-heading" })
			for _, item in section.items do
				table.insert(rows, ui.row {
					class = "side-row",
					align = "center",
					selected = state.selected == item.id,
					msg = { select = item.id },
					tooltip = if PAGES[item.id] then nil else "Only the pipeline and meetings are in this example",
					ui.icon { name = item.icon :: any },
					ui.text { item.label, class = "side-label" },
				})
			end
		end

		local quota = deals and deals.quota
		local share = if quota then math.min(1, quota.won / quota.target) else 0
		return ui.column {
			grow = true,
			gap = "none",
			ui.row { height = if window.platform == "macos" and not window.fullscreen then 36 else 8, drag = true },
			ui.column {
				class = "side",
				grow = true,
				ui.row {
					class = "side-brand",
					align = "center",
					ui.row {
						class = "side-logo",
						width = 28,
						height = 28,
						align = "center",
						justify = "center",
						ui.icon { name = "sparkles", size = "small" },
					},
					ui.column {
						gap = "none",
						ui.text { "Tally", weight = "bold" },
						ui.text { "Sales workspace", class = "side-small" },
					},
					ui.spacer(),
					ui.icon { name = "chevrons-up-down", size = "small", tone = "muted" },
				},
				ui.column { grow = true, scroll = true, gap = "none", ui.column { gap = "none", shrink = false, table.unpack(rows) } },
				quota and ui.column {
					class = "side-quota",
					ui.row {
						align = "center",
						gap = "small",
						ui.icon { name = "circle-dot", size = "small" },
						ui.text { "Q4 quota", weight = "medium" },
						ui.spacer(),
						ui.text { `{math.floor(share * 100 + 0.5)}%`, class = "side-small" },
					},
					ui.row {
						class = "side-track",
						width = 196,
						height = 6,
						ui.row { class = "side-fill", width = 196 * share, height = 6 },
					},
					ui.text {
						`{money(quota.won)} of {money(quota.target)} closed · {quota.days_left} days left`,
						class = "side-small",
						wrap = true,
					},
				},
				ui.row {
					class = "side-user",
					align = "center",
					avatar { name = "Maya Kim", initial = "MK", shape = "circle" },
					ui.column {
						gap = "none",
						grow = true,
						ui.text { "Maya Kim", weight = "medium" },
						ui.text { "maya@tally.example", class = "side-small", truncate = true },
					},
					icon_button { icon = "ellipsis", tooltip = "Account", size = "small", msg = { select = "settings" } },
				},
			},
		}
	end,
}
