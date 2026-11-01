-- The board: the quarter's title and controls, a strip of figures (quota,
-- weighted pipeline, win rate, cycle, revenue by month), then the deals in
-- columns by stage, or as a list. Luau views cannot take drag and drop, so
-- a card moves from its menu (right-click it): another stage, won, at risk.

local avatar_stack = use "avatar-stack"
local segmented = use "segmented"
local mini_bars = use "mini-bars"
local pill = use "pill"
local kanban_column = use "kanban-column"
local board_card = use "board-card"
local menu = use "menu"
local table_view = use "table"

type State = { view: string, menu: string?, query: string, owner: string? }

local function money(value: number): string
	if value >= 1000000 then
		return string.format("$%.2fM", value / 1000000)
	elseif value >= 10000 then
		return string.format("$%dK", math.floor(value / 1000 + 0.5))
	end
	return string.format("$%d", value)
end

local function amount(value: number): string
	local text = tostring(math.floor(value))
	local grouped = string.reverse((string.gsub(string.reverse(text), "(%d%d%d)", "%1,")))
	return "$" .. (string.gsub(grouped, "^,", ""))
end

return mod {
	region = "main",
	title = "Board",

	styles = {
		[".board"] = { padding = { 22, 22, 0, 26 }, gap = 18 },
		[".board-title"] = { size = "title", weight = "bold" },
		[".board-sub"] = { color = "muted-foreground" },
		[".board-button"] = { border = true, border_color = "border", radius = 8, padding = { 6, 12 }, gap = 6, background = "background" },
		[".board-button:hover"] = { background = "list-hover" },
		[".board-new"] = { padding = { 7, 12 }, gap = 6 },
		[".kpis"] = { border = true, border_color = "border", radius = 12, gap = 0 },
		[".kpi"] = { padding = { 14, 18 }, gap = 6 },
		[".kpi-rule"] = { background = "border" },
		[".kpi-label"] = { size = "small", color = "muted-foreground" },
		[".kpi-value"] = { size = "title", weight = "medium" },
		[".kpi-of"] = { size = "small", color = "muted-foreground" },
		[".kpi-note"] = { size = "small", color = "muted-foreground" },
		[".kpi-dot"] = { radius = 3 },
		[".kpi-track"] = { background = "secondary", radius = 3, gap = 2 },
		[".kpi-won"] = { background = "accent", radius = 3 },
		[".kpi-commit"] = { background = "accent/35", radius = 3 },
		[".lanes"] = { gap = 10, padding = { 0, 0, 16, 0 } },
	},

	init = function(previous: State?): State
		return previous or { view = "board", menu = nil, query = "", owner = nil }
	end,

	update = function(state: State, msg)
		if msg == "new-deal" then
			return state, { { to = "provider:deals", msg = { add = { stage = "lead" } } } }
		elseif msg == "close-menu" then
			state.menu = nil
			return
		end
		if type(msg) ~= "table" then
			return
		end
		if msg.select then
			state.view = msg.select
		elseif msg.view then
			state.view = msg.view
		elseif msg.query ~= nil then
			state.query = msg.query
		elseif msg.menu then
			state.menu = if state.menu == msg.menu then nil else msg.menu
		elseif msg.pick then
			-- From a card's menu: "<id> <action> <stage?>".
			local id, action, stage = string.match(msg.pick, "^(%S+) (%S+) ?(%S*)$")
			state.menu = nil
			if action == "move" then
				return state, { { to = "provider:deals", msg = { move = { id = id, stage = stage } } } }
			elseif action == "won" or action == "risk" then
				return state, { { to = "provider:deals", msg = { [action] = id } } }
			end
		elseif msg.add then
			return state, { { to = "provider:deals", msg = { add = { stage = msg.add } } } }
		end
	end,

	view = function(state: State, app)
		local data = app.deals
		if not data then
			return ui.column { class = "board", ui.text { "Opening the pipeline…", tone = "muted" } }
		end
		local colors = {}
		local people = {}
		for _, owner in data.owners do
			colors[owner.initials] = owner.color
			table.insert(people, { name = owner.name })
		end
		local query = string.lower(state.query)
		local function shown(deal: any): boolean
			return query == ""
				or string.find(string.lower(deal.company), query, 1, true) ~= nil
				or string.find(string.lower(deal.summary), query, 1, true) ~= nil
		end

		local quota = data.quota
		local figures = data.figures
		local won_share = quota.won / quota.target
		local commit_share = quota.commit / quota.target
		local function kpi(...)
			return ui.column { class = "kpi", width = 150, shrink = false, ... }
		end
		local rule = ui.row { class = "kpi-rule", width = 1 }
		local strip = ui.row {
			class = "kpis",
			shrink = false,
			scroll = "x",
			ui.column {
				class = "kpi",
				width = 250,
				shrink = false,
				ui.text { "Quota attainment", class = "kpi-label" },
				ui.row {
					align = "end",
					gap = "small",
					ui.text { money(quota.won), class = "kpi-value" },
					ui.text { `of {money(quota.target)}`, class = "kpi-of" },
				},
				ui.row {
					class = "kpi-track",
					width = 214,
					height = 6,
					ui.row { class = "kpi-won", width = 214 * won_share, height = 6 },
					ui.row { class = "kpi-commit", width = 214 * math.min(commit_share, 1 - won_share), height = 6 },
				},
				ui.row {
					gap = "normal",
					align = "center",
					ui.row { class = "kpi-dot", width = 6, height = 6, style = { background = "accent" } },
					ui.text { `Won {math.floor(won_share * 100 + 0.5)}%`, class = "kpi-note" },
					ui.row { class = "kpi-dot", width = 6, height = 6, style = { background = "accent/35" } },
					ui.text { `Commit {math.floor(commit_share * 100 + 0.5)}%`, class = "kpi-note" },
				},
			},
			rule,
			kpi(
				ui.text { "Weighted pipeline", class = "kpi-label" },
				ui.text { money(figures.weighted), class = "kpi-value" },
				ui.row { gap = "small", align = "center", pill { label = figures.weighted_delta, tone = "success" }, ui.text { "vs Q3", class = "kpi-note" } }
			),
			rule,
			kpi(
				ui.text { "Win rate", class = "kpi-label" },
				ui.text { figures.win_rate, class = "kpi-value" },
				ui.row { gap = "small", align = "center", pill { label = figures.win_delta, tone = "success" }, ui.text { "last 90 days", class = "kpi-note" } }
			),
			rule,
			kpi(
				ui.text { "Avg. sales cycle", class = "kpi-label" },
				ui.text { `{figures.cycle_days} days`, class = "kpi-value" },
				ui.row { gap = "small", align = "center", pill { label = figures.cycle_delta, tone = "danger" }, ui.text { "vs Q3", class = "kpi-note" } }
			),
			rule,
			ui.column {
				class = "kpi",
				width = 210,
				shrink = false,
				ui.row { ui.text { "Closed revenue", class = "kpi-label" }, ui.spacer(), ui.text { "$K / month", class = "kpi-note" } },
				mini_bars { bars = data.months, height = 34, bar = 18 },
			},
		}

		local body
		if state.view == "list" then
			local rows = {}
			local stage_names = {}
			for _, stage in data.stages do
				stage_names[stage.id] = stage.label
			end
			for _, deal in data.deals do
				if shown(deal) then
					table.insert(rows, {
						id = deal.id,
						company = deal.company,
						summary = deal.summary,
						stage = stage_names[deal.stage],
						amount = amount(deal.amount),
						date = deal.date,
						owner = deal.owner,
						chance = `{deal.chance}%`,
					})
				end
			end
			body = table_view {
				grow = true,
				columns = {
					{ key = "company", title = "Company" },
					{ key = "summary", title = "What", muted = true },
					{ key = "stage", title = "Stage", width = 120 },
					{ key = "amount", title = "Amount", width = 110, align = "right", mono = true },
					{ key = "date", title = "Close", width = 80, muted = true },
					{ key = "owner", title = "Owner", width = 70 },
					{ key = "chance", title = "Chance", width = 80, align = "right" },
				},
				rows = rows,
			}
		else
			local lanes = {}
			for _, stage in data.stages do
				local cards = {}
				for _, deal in data.deals do
					if deal.stage == stage.id and shown(deal) then
						local status = deal.status
						local choices: { any } = { { heading = "Move to" } }
						for _, other in data.stages do
							if other.id ~= deal.stage then
								table.insert(choices, { id = `{deal.id} move {other.id}`, label = other.label })
							end
						end
						table.insert(choices, "separator")
						if status ~= "won" then
							table.insert(choices, { id = `{deal.id} won`, label = "Mark won", icon = "check" })
							table.insert(choices, {
								id = `{deal.id} risk`,
								label = if status == "risk" then "Clear at risk" else "Flag at risk",
								icon = "triangle-alert",
							})
						end
						local card = board_card {
							title = deal.company,
							subtitle = deal.summary,
							badge = if status == "won" then "Won" elseif status == "risk" then "At risk" else `{deal.chance}%`,
							badge_tone = if status == "won" then "success" elseif status == "risk" then "danger" else nil,
							amount = amount(deal.amount),
							date = deal.date,
							owner = deal.owner,
							owner_color = colors[deal.owner],
							tone = if status == "risk" then "danger" else nil,
							context = { menu = deal.id },
							msg = { menu = deal.id },
						}
						-- Wrapped only while its menu is open: a popover's anchor
						-- keeps its content's width, and a card fills its column.
						if state.menu == deal.id then
							card = ui.popover {
								open = true,
								dismiss = "close-menu",
								width = 200,
								content = menu { items = choices },
								card,
							}
						end
						table.insert(cards, card)
					end
				end
				table.insert(lanes, kanban_column {
					title = stage.label,
					color = stage.color,
					count = stage.count,
					total = money(stage.total),
					add = { add = stage.id },
					add_label = "Add deal",
					table.unpack(cards),
				})
			end
			body = ui.row {
				class = "lanes",
				grow = true,
				scroll = "both",
				align = "start",
				table.unpack(lanes),
			}
		end

		local open = 0
		for _, stage in data.stages do
			if stage.id ~= "won" then
				open += stage.count
			end
		end
		return ui.column {
			class = "board",
			grow = true,
			ui.row {
				align = "start",
				gap = "normal",
				ui.column {
					gap = "none",
					grow = true,
					ui.text { "Q4 Pipeline", class = "board-title" },
					ui.text { `{open} open deals · {money(figures.weighted)} weighted · All reps`, class = "board-sub", truncate = true },
				},
				avatar_stack { people = people, count = #people, most = 4 },
				segmented {
					small = true,
					selected = state.view,
					options = {
						{ id = "board", label = "Board", icon = "layout-dashboard" },
						{ id = "list", label = "List", icon = "list" },
					},
				},
				ui.row {
					class = "board-button",
					align = "center",
					tooltip = "Type in the search field to filter",
					msg = "focus",
					to = "topbar",
					ui.icon { name = "filter", size = "small" },
					ui.text { "Filter" },
				},
				ui.row {
					class = { "dark-button", "board-new" },
					align = "center",
					msg = "new-deal",
					ui.icon { name = "plus", size = "small" },
					ui.text { "New deal", weight = "medium" },
				},
			},
			strip,
			body,
		}
	end,
}
