-- Payments: the Apr to Sep compliance notice getting ready to sign. Where
-- you are, the title and what is left, Preview and Send for approval (held
-- while exceptions are open), five headline figures; the figures as they
-- will be published (how fast invoices were paid, each figure and how it
-- was reached, on time each month, the statement and evidence); and on the
-- right what is ready to send and the scope. From `app.duly` (sample data).

local kpi_meter = use "kpi-meter"
local insight_note = use "insight-note"
local dashboard_card = use "dashboard-card"
local share_bar = use "share-bar"
local sheet = use "sheet"
local key_values = use "key-values"

type State = { figure: string?, month: number?, preview: boolean }

local TONES = { green = "success", red = "danger", grey = "muted-foreground" }

return mod {
	title = "Payments",

	styles = {
		[".d-page"] = { padding = { 22, 30, 30, 30 }, gap = 12 },
		[".crumb"] = { size = "large", color = "muted-foreground" },
		[".crumb-on"] = { size = "large" },
		[".d-page-title"] = { size = "display", weight = "medium" },
		[".d-muted"] = { color = "muted-foreground" },
		[".muted-small"] = { color = "muted-foreground", size = "small" },
		[".mono-small"] = { font = "mono", size = "small" },
		[".mono-muted"] = { font = "mono", size = "small", color = "muted-foreground" },
		[".mono-bold"] = { font = "mono", size = "small", weight = "medium" },
		[".outline-button"] = { background = "background", border = true, border_color = "border", radius = 8, padding = { 7, 12 }, gap = 6, shadow = "0 1px 1px #00000008" },
		[".outline-button:hover"] = { background = "secondary" },
		[".held-button"] = { background = "grey", color = "#ffffff", radius = 8, padding = { 7, 12 }, gap = 6 },
		[".strip"] = { background = "secondary", border = true, border_color = "border", radius = 12, gap = 0 },
		[".strip-rule"] = { background = "border" },
		[".legend-dot"] = { radius = 3 },
		[".head"] = { padding = { 6, 4 }, border = "bottom", border_color = "hairline" },
		[".heading"] = { size = "small", color = "muted-foreground" },
		[".row"] = { padding = { 11, 4 }, border = "bottom", border_color = "hairline", radius = 4, gap = 10 },
		[".row:hover"] = { background = "list-hover" },
		[".row:selected"] = { background = "accent/8" },
		[".hairline"] = { background = "border" },
		[".month"] = { radius = 4, gap = 6, padding = { 4, 0 } },
		[".month:hover"] = { background = "list-hover" },
		[".month-bar"] = { background = "#00000012", radius = 4 },
		[".month-bar-on"] = { background = "#2b8ae6", radius = 4 },
		[".step"] = { padding = { 12, 2 }, gap = 12, border = "bottom", border_color = "hairline", radius = 4 },
		[".step:hover"] = { background = "list-hover" },
		[".step-last"] = { border = "none" },
		[".status"] = { background = "#00000010", radius = 5, padding = { 2, 7 }, gap = 5 },
		[".kv"] = { padding = { 12, 0 }, border = "bottom", border_color = "hairline" },
		[".kv-last"] = { border = "none" },
		[".notice-paper"] = { background = "background", border = true, border_color = "border", radius = 10, padding = 18, gap = 10 },
	},

	init = function(previous: State?): State
		local state: State = previous or { preview = false }
		state.preview = false
		return state
	end,

	update = function(state: State, msg)
		if msg == "preview" then
			state.preview = true
		elseif msg == "close-preview" then
			state.preview = false
		elseif type(msg) == "table" then
			if msg.figure then
				state.figure = if state.figure == msg.figure then nil else msg.figure
			elseif msg.month then
				state.month = msg.month
			elseif msg.go then
				return state, { { to = "sidebar", msg = { page = msg.go } } }
			end
		end
	end,

	view = function(state: State, app)
		local data = app.duly
		if not data or data.available == false then
			return ui.column { grow = true, align = "center", justify = "center", ui.text { if data then data.reason or "No data" else "Loading…", class = "d-muted" } }
		end
		local notice = data.payments

		local head = ui.column {
			gap = "none",
			ui.row {
				drag = true,
				align = "center",
				gap = "normal",
				height = 36,
				ui.icon { name = "file", class = "d-muted" },
				ui.row { msg = { go = "payments" }, ui.text { "Payments", class = "crumb" } },
				ui.text { "/", class = "crumb" },
				ui.text { notice.crumb, class = "crumb-on" },
			},
			ui.row { height = 22 },
			ui.row {
				align = "end",
				ui.column {
					gap = "small",
					ui.text { notice.title, class = "d-page-title" },
					ui.text { notice.subtitle, class = "d-muted" },
				},
				ui.spacer(),
				ui.row {
					gap = "normal",
					ui.row { class = "outline-button", align = "center", msg = "preview", ui.icon { name = "eye", size = "small" }, ui.text { "Preview notice", weight = "medium" } },
					ui.row {
						class = "held-button",
						align = "center",
						tooltip = "Held until the 9 open exceptions are fixed",
						ui.icon { name = "pencil", size = "small" },
						ui.text { "Send for approval", weight = "medium" },
					},
				},
			},
		}

		local cells = {}
		for index, kpi in notice.kpis do
			if index > 1 then
				table.insert(cells, ui.row { class = "strip-rule", width = 1 })
			end
			table.insert(cells, kpi_meter {
				title = kpi.title,
				dot = kpi.dot,
				value = kpi.value,
				mono = kpi.mono,
				caption = kpi.caption,
				caption_color = if kpi.caption_tone == "red" then "danger" elseif kpi.caption_tone == "green" then "success" else nil,
				segments = kpi.bar,
				width = 184,
				msg = if index == 5 then { go = "today" } else { figure = `f{index}` },
			})
		end

		-- How fast invoices were paid.
		local speed = notice.speed
		local legend = {}
		for _, share in speed.shares do
			table.insert(legend, ui.row {
				gap = "small",
				align = "center",
				ui.row { class = "legend-dot", width = 6, height = 6, style = { background = share.color } },
				ui.text { share.label, class = "muted-small" },
				ui.text { share.value, class = "mono-bold" },
			})
		end

		-- Each figure and how it was reached.
		local figures = {}
		for _, figure in notice.figures do
			table.insert(figures, ui.row {
				class = "row",
				align = "center",
				selected = state.figure == figure.id,
				msg = { figure = figure.id },
				ui.row { width = 260, shrink = false, ui.text { figure.figure, size = "small" } },
				ui.row { width = 56, shrink = false, justify = "end", ui.text { figure.value, class = "mono-small" } },
				ui.row { grow = true, ui.text { figure.how, size = "small", class = "d-muted", truncate = true } },
				ui.text { figure.source, class = "mono-small" },
				ui.icon { name = "chevron-right", size = "small", class = "d-muted" },
			})
		end

		-- On time each month; the axis starts at 80%.
		local month = state.month or #notice.months
		local months = {}
		for index, m in notice.months do
			local on = index == month
			table.insert(months, ui.column {
				class = "month",
				grow = true,
				align = "center",
				justify = "end",
				height = 100,
				msg = { month = index },
				tooltip = `{m.label}: {m.text} on time`,
				ui.text { m.text, class = "mono-muted" },
				ui.row { class = if on then "month-bar-on" else "month-bar", width = 102, height = math.floor((m.value - 80) * 3) },
			})
		end
		local month_labels = {}
		for _, m in notice.months do
			table.insert(month_labels, ui.row { grow = true, justify = "center", ui.text { m.label, class = "mono-muted" } })
		end

		local evidence = {}
		for index, item in notice.evidence do
			table.insert(evidence, ui.row {
				class = if index == #notice.evidence then { "kv", "kv-last" } else "kv",
				ui.text { item.label, size = "small", class = "d-muted" },
				ui.spacer(),
				ui.text { item.value, size = "small", weight = "medium" },
			})
		end

		local figures_card = dashboard_card {
			title = "Figures as they will be published",
			icon = "file",
			menu = "figures-menu",
			grow = true,
			ui.row { align = "center", ui.text { speed.title, weight = "medium" }, ui.spacer(), ui.text { speed.rule, class = "mono-muted" } },
			ui.column {
				gap = "small",
				share_bar { shares = speed.shares, width = 680, height = 10 },
				ui.row { gap = "large", table.unpack(legend) },
			},
			ui.column {
				gap = "none",
				ui.row {
					class = "head",
					ui.row { width = 260, ui.text { "Figure", class = "heading" } },
					ui.row { width = 56, justify = "end", ui.text { "Value", class = "heading" } },
					ui.row { grow = true, ui.text { "How it was reached", class = "heading" } },
					ui.text { "Source", class = "heading" },
				},
				table.unpack(figures),
			},
			ui.row { height = 4 },
			ui.row { align = "center", ui.text { "On time each month", weight = "medium" }, ui.spacer(), ui.text { "axis from 80%", class = "mono-muted" } },
			ui.column { gap = "small", ui.row { gap = "normal", align = "end", table.unpack(months) }, ui.row { gap = "normal", table.unpack(month_labels) } },
			ui.row { class = "hairline", height = 1 },
			ui.text { `Approval statement {notice.signer} will sign`, weight = "medium" },
			ui.text { notice.statement, class = "d-muted" },
			ui.text { "Evidence attached", weight = "medium" },
			ui.column { gap = "none", table.unpack(evidence) },
		}

		-- What is ready to send.
		local steps = {}
		for index, step in notice.steps do
			local tone = TONES[step.tone] or "muted-foreground"
			table.insert(steps, ui.row {
				class = if index == #notice.steps then { "step", "step-last" } else "step",
				align = "center",
				msg = if step.tone == "red" then { go = "today" } else nil,
				ui.icon { name = step.icon :: any, size = "small", style = { color = if step.tone == "grey" then "muted-foreground" else tone } },
				ui.column {
					grow = true,
					gap = "none",
					ui.text { step.title, size = "small", weight = "medium" },
					ui.text { step.detail, class = "muted-small" },
				},
				ui.row {
					class = "status",
					align = "center",
					ui.row { class = "legend-dot", width = 5, height = 5, style = { background = tone } },
					ui.text { step.status, size = "small", class = "d-muted" },
				},
			})
		end
		local ready_card = dashboard_card {
			title = "Ready to send",
			icon = "circle-check",
			menu = "ready-menu",
			ui.column { gap = "none", table.unpack(steps) },
			insight_note { text = notice.insight },
		}

		local scope = {}
		for index, item in notice.scope do
			table.insert(scope, ui.row {
				class = if index == #notice.scope then { "kv", "kv-last" } else "kv",
				ui.text { item.label, size = "small", class = "d-muted" },
				ui.spacer(),
				ui.text { item.value, size = "small", weight = "medium" },
			})
		end
		local scope_card = dashboard_card {
			title = "Scope",
			icon = "file-text",
			menu = "scope-menu",
			ui.column { gap = "none", table.unpack(scope) },
		}

		-- The notice as it will read once published.
		local preview_rows = {}
		for _, figure in notice.figures do
			table.insert(preview_rows, { figure.figure, figure.value })
		end
		local preview = if not state.preview then nil else sheet {
			open = true,
			dismiss = "close-preview",
			title = "Preview: " .. notice.title,
			subtitle = notice.crumb,
			icon = "eye",
			width = 460,
			ui.column {
				class = "notice-paper",
				ui.text { "Payment practices notice", weight = "medium", size = "large" },
				ui.text { "Reporting period 1 April to 30 September 2026", class = "muted-small" },
				key_values { rows = preview_rows, width = 260 },
				ui.text { notice.statement, class = "muted-small" },
				ui.text { `Signed: {notice.signer}, Finance director (pending)`, size = "small" },
			},
		}

		return ui.column {
			grow = true,
			scroll = true,
			gap = "none",
			ui.column {
				class = "d-page",
				shrink = false,
				head,
				ui.row { height = 4 },
				ui.row { class = "strip", align = "stretch", table.unpack(cells) },
				ui.row {
					gap = "normal",
					align = "start",
					figures_card,
					ui.column { width = 372, shrink = false, gap = "normal", ready_card, scope_card },
				},
			},
			preview,
		}
	end,
}
