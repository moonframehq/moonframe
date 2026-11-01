-- The Audience page: a header with the range and actions, a strip of four
-- key figures, growth per day against the previous period beside where
-- signups came from, then locations, devices and engagement tiers. Its
-- figures are `app.inkwell`, sample data from the provider.

local kpi_cell = use "kpi-cell"
local dot_chart = use "dot-chart"
local ratio_row = use "ratio-row"
local share_bar = use "share-bar"
local country_code = use "country-code"
local dropdown = use "dropdown"

type State = { page: string, range: string, picking: boolean, every: string, picking_every: boolean }

local RANGES = { ["7"] = "Last 7 Days", ["30"] = "Last 30 Days", ["90"] = "Last 90 Days" }

return mod {
	region = "main",
	title = "Audience",

	styles = {
		[".page"] = { gap = 0 },
		[".page-head"] = { padding = { 22, 22, 18, 24 }, gap = 10 },
		[".page-title"] = { size = "title", weight = "medium" },
		[".page-subtitle"] = { color = "muted-foreground" },
		[".page-action"] = { radius = 18 },
		[".page-primary"] = { background = "ink", color = "#ffffff", radius = 18 },
		[".kpis"] = { border = "top", border_color = "border", gap = 0 },
		[".kpi-split"] = { border = "left", border_color = "border" },
		[".section"] = { border = "top", border_color = "border", gap = 0 },
		[".block"] = { padding = { 20, 22 }, gap = 14 },
		[".block-split"] = { border = "left", border_color = "border" },
		[".block-title"] = { size = "large", weight = "medium" },
		[".block-subtitle"] = { color = "muted-foreground" },
		[".legend"] = { gap = 14 },
		[".legend-swatch"] = { radius = 2 },
		[".legend-text"] = { size = "small" },
		[".row-rule"] = { background = "border" },
		[".clients"] = { radius = 8, border = true, border_color = "border", padding = { 8, 12 }, gap = 8, background = "muted-foreground/4" },
		[".clients-label"] = { size = "small", color = "muted-foreground", case = "upper" },
		[".tier-dot"] = { radius = 5 },
		[".count"] = { color = "muted-foreground" },
		[".placeholder"] = { padding = 40, gap = 6 },
	},

	init = function(previous: State?): State
		return previous or { page = "audience", range = "30", picking = false, every = "daily", picking_every = false }
	end,

	update = function(state: State, msg)
		if msg == "pick-range" then
			state.picking = not state.picking
		elseif msg == "close-range" then
			state.picking = false
		elseif msg == "pick-every" then
			state.picking_every = not state.picking_every
		elseif msg == "close-every" then
			state.picking_every = false
		elseif type(msg) == "table" then
			if msg.page then
				state.page = msg.page
			elseif msg.range then
				state.range = msg.range
				state.picking = false
			elseif msg.every then
				state.every = msg.every
				state.picking_every = false
			end
		end
	end,

	view = function(state: State, app)
		local data = app.inkwell
		if state.page ~= "audience" then
			return ui.column {
				class = "placeholder",
				grow = true,
				align = "center",
				justify = "center",
				ui.icon { name = "layout-dashboard", size = "title", tone = "muted" },
				ui.text { "Only Audience is drawn in this example.", tone = "muted" },
			}
		end
		if not data then
			return ui.column { class = "placeholder", ui.text { "Loading the sample audience…", tone = "muted" } }
		end

		local ranges: { any } = {}
		for _, id in { "7", "30", "90" } do
			table.insert(ranges, { id = id, label = RANGES[id] })
		end
		local head = ui.row {
			class = "page-head",
			align = "start",
			ui.column {
				gap = "small",
				ui.text { "Audience", class = "page-title" },
				ui.text { "Who reads your newsletter and where they come from.", class = "page-subtitle" },
			},
			ui.spacer(),
			dropdown {
				icon = "calendar",
				options = ranges,
				selected = state.range,
				open = state.picking,
				toggle = "pick-range",
				dismiss = "close-range",
				event = "range",
				align = "end",
			},
			ui.button { "Import", icon = "upload", variant = "outline", class = "page-action", msg = "import" },
			ui.button { "Add subscribers", icon = "plus", class = "page-primary", msg = "add" },
		}

		local kpis = {}
		for index, kpi in data.kpis do
			table.insert(kpis, ui.row {
				class = if index > 1 then "kpi-split" else nil,
				grow = true,
				kpi_cell {
					title = kpi.title,
					icon = kpi.icon,
					tint = kpi.tint,
					value = kpi.value,
					delta = kpi.delta,
					good = kpi.good,
					caption = kpi.caption,
					menu = { kpi = kpi.id },
				},
			})
		end

		local growth = data.growth
		local columns = {}
		for day, current in growth.current do
			table.insert(columns, {
				current = current,
				previous = growth.previous[day],
				tooltip = `{current} new · {growth.previous[day]} before`,
			})
		end
		local everies: { any } = {
			{ id = "daily", label = "Daily" },
			{ id = "weekly", label = "Weekly" },
		}
		local growth_block = ui.column {
			class = "block",
			grow = true,
			ui.row {
				align = "start",
				ui.column {
					gap = "small",
					ui.text { "Audience growth", class = "block-title" },
					ui.text { "New subscribers per day, compared with the previous 30 days.", class = "block-subtitle" },
				},
				ui.spacer(),
				dropdown {
					icon = "calendar",
					options = everies,
					selected = state.every,
					open = state.picking_every,
					toggle = "pick-every",
					dismiss = "close-every",
					event = "every",
					align = "end",
				},
			},
			ui.row {
				class = "legend",
				align = "center",
				ui.row { class = "legend-swatch", width = 4, height = 12, style = { background = "accent" } },
				ui.text { "This period", class = "legend-text" },
				ui.row { class = "legend-swatch", width = 4, height = 12, style = { background = "muted-foreground/20" } },
				ui.text { "Previous period", class = "legend-text" },
			},
			dot_chart { columns = columns, max = growth.max, rows = 8, ticks = growth.ticks, labels = growth.labels },
		}

		local sources = {}
		for index, source in data.sources do
			if index > 1 then
				table.insert(sources, ui.row { class = "row-rule", height = 1 })
			end
			table.insert(sources, ratio_row {
				label = source.label,
				icon = source.icon,
				count = source.count,
				percent = `{source.share}%`,
				share = source.share,
				color = source.color,
				width = 330,
			})
		end
		local sources_block = ui.column {
			class = { "block", "block-split" },
			width = 420,
			shrink = false,
			ui.column {
				gap = "small",
				ui.text { "Acquisition sources", class = "block-title" },
				ui.text { "Where this month's 1,382 signups came from.", class = "block-subtitle" },
			},
			ui.column { gap = "none", table.unpack(sources) },
		}

		local function block(title: string, subtitle: string, split: boolean, ...)
			return ui.column {
				class = if split then { "block", "block-split" } else "block",
				grow = true,
				ui.row {
					align = "start",
					ui.column {
						gap = "small",
						ui.text { title, class = "block-title" },
						ui.text { subtitle, class = "block-subtitle" },
					},
					ui.spacer(),
					ui.button { icon = "ellipsis", variant = "ghost", small = true },
				},
				...,
			}
		end

		local places = {}
		for _, place in data.locations do
			table.insert(places, ratio_row {
				label = place.name,
				leading = country_code { code = place.code },
				count = place.count,
				percent = place.share,
			})
		end

		local device_shares, device_rows = {}, {}
		for _, device in data.devices.rows do
			table.insert(device_shares, { share = device.share, color = device.color, label = device.label })
			table.insert(device_rows, ratio_row {
				label = device.label,
				leading = ui.row { width = 14, height = 14, style = { background = `{device.color}`, radius = 4 } },
				count = device.opens,
				percent = `{device.share}%`,
			})
		end

		local tier_shares, tier_rows = {}, {}
		for _, tier in data.tiers do
			table.insert(tier_shares, { share = tier.share, color = tier.color, label = tier.label })
			table.insert(tier_rows, ratio_row {
				label = tier.label,
				leading = ui.row { class = "tier-dot", width = 10, height = 10, style = { background = tier.color } },
				count = tier.count,
				percent = `{tier.share}%`,
			})
		end

		return ui.column {
			class = "page",
			grow = true,
			scroll = true,
			ui.column {
				gap = "none",
				shrink = false,
				head,
				ui.row { class = "kpis", table.unpack(kpis) },
				ui.row { class = "section", growth_block, sources_block },
				ui.row {
					class = "section",
					block("Top locations", "Share of active subscribers", false, ui.column { gap = "none", table.unpack(places) }),
					block(
						"Reading devices",
						"Where emails were opened",
						true,
						share_bar { shares = device_shares, width = 290 },
						ui.column { gap = "none", table.unpack(device_rows) },
						ui.row {
							class = "clients",
							align = "center",
							ui.text { "Top clients", class = "clients-label" },
							ui.text { data.devices.clients, size = "small", truncate = true },
						}
					),
					block(
						"Engagement tiers",
						"Based on opens and clicks in 90 days",
						true,
						share_bar { shares = tier_shares, width = 290 },
						ui.column { gap = "none", table.unpack(tier_rows) }
					),
				},
			},
		}
	end,
}
