-- The Leads Report: three pipeline figures in one card, sales revenue by
-- month with one month in the spotlight (click another to move it), leads
-- by status beside web visits, then the hottest deals beside where leads
-- come from. Its figures are `app.leads`, sample data from the provider.

local spotlight_bars = use "spotlight-bars"
local level_row = use "level-row"

type State = { page: string, month: number?, growth: string }

return mod {
	region = "main",
	title = "Leads Report",

	styles = {
		[".page"] = { padding = { 24, 24, 28, 24 }, gap = 24 },
		[".kpis"] = { gap = 0 },
		[".kpi"] = { padding = { 20, 14, 20, 28 }, gap = 6 },
		[".kpi-split"] = { border = "left", border_color = "border" },
		[".label"] = { color = "#cfcfcf" },
		[".figure"] = { size = "display", font = "mono", weight = "medium", color = "foreground" },
		[".delta"] = { gap = 4 },
		[".delta-note"] = { color = "muted-foreground", font = "mono", margin = { left = 8 } },
		[".block"] = { padding = { 20, 24, 24, 28 }, gap = 6 },
		[".chip"] = { background = "chip", radius = 14, padding = { 10, 14 }, gap = 6, border = true, border_color = "#ffffff00" },
		[".chip:hover"] = { background = "#292929" },
		[".chip:selected"] = { border_color = "#3a3a3a" },
		[".chip-label"] = { font = "mono", color = "muted-foreground" },
		[".chart"] = { margin = { top = 30 } },
		[".in"] = { font = "mono", color = "muted-foreground", margin = { left = 10, bottom = 4 } },
		[".levels"] = { margin = { top = 18 }, gap = 6 },
		[".spark"] = { color = "#18c2ff", margin = { top = 22, bottom = 8 } },
		[".source"] = { padding = { 9, 0 }, gap = 10 },
		[".source-rule"] = { background = "border" },
		[".swatch"] = { radius = 3 },
		[".mono-muted"] = { font = "mono", color = "muted-foreground" },
		[".mono-text"] = { font = "mono" },
		[".deal"] = { padding = { 11, 8 }, gap = 12, radius = 10 },
		[".deal:hover"] = { background = "list-hover" },
		[".deal-mark"] = { radius = 9, background = "chip", border = true, border_color = "border" },
		[".stage"] = { radius = 7, padding = { 2, 8 }, gap = 6, size = "small" },
		[".head-row"] = { gap = 8 },
		[".more"] = { color = "muted-foreground", radius = 6, padding = 4 },
		[".more:hover"] = { background = "list-hover", color = "foreground" },
		[".tile"] = { radius = 8, background = "chip", border = true, border_color = "border" },
		[".placeholder"] = { gap = 8 },
	},

	init = function(previous: State?): State
		return {
			page = previous and previous.page or "leads",
			month = previous and previous.month,
			growth = previous and previous.growth or "3m",
		}
	end,

	update = function(state: State, msg)
		if type(msg) == "table" then
			if msg.page then
				state.page = msg.page
			elseif msg.select then
				state.month = msg.select
			elseif msg.growth then
				state.growth = msg.growth
			end
		end
	end,

	view = function(state: State, app)
		local data = app.leads
		if state.page ~= "leads" then
			return ui.column {
				class = "placeholder",
				grow = true,
				align = "center",
				justify = "center",
				ui.icon { name = "network", size = "title", tone = "muted" },
				ui.text { "Only the Leads Report is drawn in this example.", tone = "muted" },
			}
		end
		if not data or not data.available then
			return ui.column {
				grow = true,
				align = "center",
				justify = "center",
				ui.text { "Loading the sample leads…", tone = "muted" },
			}
		end

		-- "12.5% ↓ vs Last Months": red when down, green when up.
		local function delta(text: string, up: boolean, note: string?)
			return ui.row {
				class = "delta",
				align = "center",
				ui.text { text, class = if up then "up" else "down" },
				ui.icon { name = if up then "arrow-up" else "arrow-down", size = "small", class = if up then "up" else "down" },
				note and ui.text { note, class = "delta-note" },
			}
		end

		local function more(id: string)
			return ui.row { class = "more", msg = { more = id }, tooltip = "More", ui.icon { name = "ellipsis", size = "small" } }
		end

		-- The three pipeline figures, one card split in three.
		local kpis = {}
		for index, kpi in data.kpis do
			table.insert(kpis, ui.column {
				class = if index > 1 then { "kpi", "kpi-split" } else "kpi",
				grow = true,
				ui.text { kpi.title, class = "label" },
				ui.text { kpi.value, class = "figure" },
				ui.row { height = 18 },
				delta(kpi.delta, kpi.up, kpi.note),
			})
		end

		-- Sales revenue: the total, growth chips, and the months.
		local revenue = data.revenue
		local month = state.month or revenue.peak
		local bars = {}
		for index, m in revenue.months do
			bars[index] = { label = m.label, value = m.value, tooltip = `{m.label}  {m.amount}` }
		end
		local chips = {}
		for _, g in revenue.growth do
			table.insert(chips, ui.column {
				class = "chip",
				width = 150,
				selected = g.id == state.growth,
				msg = { growth = g.id },
				ui.text { g.label, class = "chip-label" },
				delta(g.delta, g.up),
			})
		end
		local chosen = revenue.months[month]
		local sales = ui.column {
			class = { "card", "block" },
			ui.row {
				align = "start",
				ui.column {
					gap = "small",
					ui.text { "Sales Revenue", class = "label" },
					ui.text { revenue.total, class = "figure" },
					delta(revenue.delta, revenue.up, revenue.note),
				},
				ui.spacer(),
				ui.row { gap = "normal", style = { gap = 14 }, table.unpack(chips) },
			},
			ui.column {
				class = "chart",
				spotlight_bars { bars = bars, selected = month, height = 230 },
			},
			chosen and ui.row {
				align = "center",
				style = { margin = { top = 14 } },
				ui.row { class = "swatch", width = 10, height = 10, style = { gradient = "180deg, #ff9a1f, #ff6400" } },
				ui.text { ` {chosen.label}`, class = "mono-text" },
				ui.text { `  {chosen.amount} booked`, class = "mono-muted" },
			},
		}

		-- Leads by status, each on its own scale.
		local levels = {}
		for _, row in data.status.rows do
			table.insert(levels, level_row {
				label = row.label,
				value = tostring(row.count),
				fraction = row.share,
				color = row.from,
				end_color = row.to,
				width = 400,
				label_width = 120,
				tooltip = `{row.count} leads`,
			})
		end
		local status = ui.column {
			class = { "card", "block" },
			grow = true,
			ui.row { class = "head-row", align = "center", ui.text { "Leads by Status", class = "label" }, ui.spacer(), more("status") },
			ui.row {
				align = "end",
				ui.text { tostring(data.status.total), class = "figure" },
				ui.text { "in Pipeline", class = "in" },
			},
			ui.column { class = "levels", table.unpack(levels) },
		}

		-- Web visits: the figure, its weeks as a curve, and its sources.
		local visits = data.visits
		local sources = {}
		for index, source in visits.sources do
			if index > 1 then
				table.insert(sources, ui.row { class = "source-rule", height = 1 })
			end
			table.insert(sources, ui.row {
				class = "source",
				align = "center",
				ui.row { class = "swatch", width = 10, height = 10, style = { background = source.color } },
				ui.text { source.label },
				ui.spacer(),
				ui.text { source.value, class = "mono-text" },
				ui.row { width = 44, justify = "end", ui.text { `{source.share}%`, class = "mono-muted" } },
			})
		end
		local web = ui.column {
			class = { "card", "block" },
			width = 420,
			shrink = false,
			ui.row { class = "head-row", align = "center", ui.text { "Web Visits", class = "label" }, ui.spacer(), more("visits") },
			ui.text { visits.value, class = "figure" },
			delta(visits.delta, visits.up, visits.note),
			ui.line { points = visits.weeks, min = 30, dots = "last", stroke = 2, height = 90, class = "spark" },
			ui.column { gap = "none", table.unpack(sources) },
		}

		-- The hottest deals, and where leads come from.
		local deals = {}
		for _, deal in data.deals do
			table.insert(deals, ui.row {
				class = "deal",
				align = "center",
				msg = { deal = deal.company },
				ui.row {
					class = "deal-mark",
					width = 32,
					height = 32,
					align = "center",
					justify = "center",
					ui.text { string.sub(deal.company, 1, 1), weight = "medium" },
				},
				ui.column {
					gap = "none",
					grow = true,
					ui.text { deal.company, truncate = true },
					ui.text { deal.contact, size = "small", tone = "muted" },
				},
				ui.row {
					class = "stage",
					align = "center",
					style = { background = `{deal.tone}22`, color = deal.tone },
					ui.row { class = "swatch", width = 6, height = 6, style = { background = deal.tone } },
					ui.text { deal.stage },
				},
				ui.row { width = 90, justify = "end", ui.text { deal.value, class = "mono-text" } },
				ui.row { width = 34, justify = "end", ui.text { deal.age, class = "mono-muted", size = "small" } },
			})
		end
		local hot = ui.column {
			class = { "card", "block" },
			grow = true,
			ui.row { class = "head-row", align = "center", ui.text { "Hot Deals", class = "label" }, ui.spacer(), more("deals") },
			ui.column { gap = "none", style = { margin = { top = 10 } }, table.unpack(deals) },
		}

		local origins = {}
		for _, source in data.sources do
			table.insert(origins, ui.column {
				gap = "small",
				style = { padding = { 8, 0 } },
				ui.row {
					align = "center",
					gap = "normal",
					ui.row {
						class = "tile",
						width = 26,
						height = 26,
						align = "center",
						justify = "center",
						ui.icon { name = source.icon, size = "small", style = { color = source.color } },
					},
					ui.text { source.label },
					ui.spacer(),
					ui.text { source.count, class = "mono-muted" },
					ui.row { width = 40, justify = "end", ui.text { `{source.share}%`, class = "mono-text" } },
				},
				ui.row {
					height = 6,
					style = { background = "#ffffff0d", radius = 3 },
					ui.row { width = math.floor(360 * source.share / 40), style = { gradient = `90deg, {source.color}88, {source.color}`, radius = 3 } },
				},
			})
		end
		local origin = ui.column {
			class = { "card", "block" },
			width = 420,
			shrink = false,
			ui.row { class = "head-row", align = "center", ui.text { "Lead Sources", class = "label" }, ui.spacer(), more("sources") },
			ui.column { gap = "none", style = { margin = { top = 10 } }, table.unpack(origins) },
		}

		return ui.column {
			grow = true,
			scroll = true,
			gap = "none",
			ui.column {
				class = "page",
				shrink = false,
				ui.row { class = { "card", "kpis" }, table.unpack(kpis) },
				sales,
				ui.row { gap = "large", style = { gap = 24 }, status, web },
				ui.row { gap = "large", style = { gap = 24 }, hot, origin },
			},
		}
	end,
}
