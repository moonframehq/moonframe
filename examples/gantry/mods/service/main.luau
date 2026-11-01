-- One service's details: its header (name, tags, Live logs, New deploy),
-- the description, and four tabs. Resource shows what it is allocated and
-- uses (CPU, GPU, memory as tick meters, limits you can change), its usage
-- over the day (hover for values, click to pin) and its alerts. Overview,
-- Analytics and Settings show the rest. Drawn while the console's
-- `gantry/nav` topic points at a service; data from `app.telemetry`.

local frame_box = use "frame-box"
local square_button = use "square-button"
local tag = use "tag"
local tick_meter = use "tick-meter"
local series_chart = use "series-chart"
local dropdown = use "dropdown"
local switch = use "switch"
local stepper = use "stepper"
local sheet = use "sheet"

type Alert = { id: string, kind: string, title: string, detail: string, on: boolean, metric: string?, threshold: number? }

type State = {
	page: string,
	service: string,
	tab: string,
	range: string,
	range_open: boolean,
	pinned: number?,
	configure: boolean,
	logs: boolean,
	alerts: { Alert },
	added: number,
	autoscale: boolean,
	zero_downtime: boolean,
	min_replicas: number,
	max_replicas: number,
}

local TABS = { "overview", "resource", "analytics", "settings" }
local RANGES = { ["6h"] = 25, ["12h"] = 49, ["24h"] = 97 }

local RESOURCES = {
	{ key = "cpu", name = "CPU", color = "#3b82f6", icon = "cpu", limit = "Cpu limit" },
	{ key = "gpu", name = "GPU", color = "#22c55e", icon = "monitor", limit = "Gpu limit" },
	{ key = "memory", name = "MEMORY", color = "#f97316", icon = "memory-stick", limit = "Mem limit" },
}

local PRESETS: { Alert } = {
	{ id = "mem", kind = "warning", title = "Memory above 80%", detail = "Triggers when memory > 80% for 10 minutes", on = true, metric = "memory", threshold = 80 },
	{ id = "cpu", kind = "warning", title = "CPU above 85%", detail = "Triggers when CPU > 85% for 5 minutes", on = true, metric = "cpu", threshold = 85 },
	{ id = "errors", kind = "blocked", title = "Error rate above 2%", detail = "Triggers when failed runs > 2% for 5 minutes", on = true },
}

local function default_alerts(): { Alert }
	return {
		{ id = "gpu", kind = "warning", title = "GPU usage above 90%", detail = "Triggers when GPU > 90% for 5 minutes", on = true, metric = "gpu", threshold = 90 },
		{ id = "queue", kind = "blocked", title = "Queue depth above 5,000", detail = "Triggers when pending tool calls > 5,000 for 2 minutes", on = true },
		{ id = "p95", kind = "warning", title = "P95 latency above 800 ms", detail = "Triggers when p95 token latency > 800 ms", on = false, metric = "p95", threshold = 800 },
	}
end

local function tone_for(value: number): string
	return if value >= 75 then "danger" elseif value >= 50 then "warning" else "success"
end

local function slice(list: { any }, count: number): { any }
	local out = {}
	for index = math.max(1, #list - count + 1), #list do
		table.insert(out, list[index])
	end
	return out
end

local function trim(value: number): string
	if value == math.floor(value) then
		return tostring(value)
	end
	return string.format("%.1f", value)
end

return mod {
	region = "main",
	title = "Service",
	subscribe = { "gantry/nav" },

	styles = {
		[".crumbs"] = { padding = { 0, 32 }, gap = 10, border = "bottom", border_color = "border" },
		[".crumb"] = { color = "muted-foreground" },
		[".crumb:hover"] = { color = "foreground" },
		[".crumb-current"] = { color = "bright", weight = "medium" },
		[".page"] = { padding = { 24, 32, 24, 32 }, gap = 0 },
		[".service-tile"] = { background = "#f26b1d", color = "#ffffff" },
		[".service-name"] = { size = "large", color = "bright" },
		[".label"] = { color = "muted-foreground" },
		[".muted"] = { color = "muted-foreground" },
		[".muted-small"] = { color = "muted-foreground", size = "small" },
		[".description"] = { color = "#dcdcdc" },
		[".tabs"] = { border = true, border_color = "border", gap = 0 },
		[".tab"] = { padding = { 0, 12 }, border = "right", border_color = "border", color = "#bdbdbd" },
		[".tab:hover"] = { background = "#1d1d1d" },
		[".tab:selected"] = { background = "#ececec", color = "#111111" },
		[".tab-last"] = { border = "none" },
		[".heading"] = { color = "bright", weight = "medium" },
		[".panel"] = { padding = { 20, 28, 14, 28 }, gap = 0, border = "right", border_color = "border" },
		[".panel-last"] = { border = "none" },
		[".panel-icon"] = { color = "#ffffff" },
		[".stat-value"] = { size = "large", color = "bright" },
		[".limit"] = { border = true, border_color = "border", padding = { 0, 10 }, gap = 8, background = "#141414" },
		[".limit-unit"] = { color = "muted-foreground", size = "small" },
		[".limit-step"] = { color = "muted-foreground", padding = { 0, 4 } },
		[".limit-step:hover"] = { color = "bright", background = "#262626" },
		[".alert-row"] = { padding = { 14, 0 }, gap = 16, border = "bottom", border_color = "border" },
		[".alert-last"] = { border = "none" },
		[".alert-icon"] = { background = "#222222" },
		[".alert-title"] = { color = "bright", size = "large" },
		[".log-line"] = { gap = 12, padding = { 3, 0 } },
		[".log-time"] = { color = "muted-foreground" },
		[".log-info"] = { color = "#7fb0ff" },
		[".log-warn"] = { color = "#eab308" },
		[".log-error"] = { color = "#ef4444" },
		[".log-debug"] = { color = "#8a8a8a" },
		[".kpi"] = { padding = { 16, 20 }, gap = 6, border = "right", border_color = "border" },
		[".kpi-last"] = { border = "none" },
		[".kpi-value"] = { size = "title", color = "bright" },
		[".kv"] = { padding = { 10, 0 }, border = "bottom", border_color = "hairline" },
		[".list-row"] = { padding = { 12, 0 }, gap = 14, border = "bottom", border_color = "hairline" },
		[".setting"] = { padding = { 16, 0 }, gap = 16, border = "bottom", border_color = "hairline" },
		[".danger-button"] = { border = true, border_color = "#ef444466", color = "#ef4444", padding = { 0, 12 }, gap = 8 },
		[".danger-button:hover"] = { background = "#ef444418" },
	},

	init = function(previous: State?): State
		local p: any = previous or {}
		return {
			page = p.page or "service",
			service = p.service or "token-relay",
			tab = p.tab or "resource",
			range = p.range or "24h",
			range_open = false,
			pinned = p.pinned,
			configure = false,
			logs = false,
			alerts = p.alerts or default_alerts(),
			added = p.added or 0,
			autoscale = if p.autoscale == nil then true else p.autoscale,
			zero_downtime = if p.zero_downtime == nil then true else p.zero_downtime,
			min_replicas = p.min_replicas or 6,
			max_replicas = p.max_replicas or 24,
		}
	end,

	update = function(state: State, msg)
		if msg == "logs" then
			state.logs = not state.logs
		elseif msg == "close-logs" then
			state.logs = false
		elseif msg == "deploy" then
			return state, { { to = "provider:telemetry", msg = { deploy = state.service } } }
		elseif msg == "configure" then
			state.configure = not state.configure
		elseif msg == "toggle-range" then
			state.range_open = not state.range_open
		elseif msg == "close-range" then
			state.range_open = false
		elseif msg == "add-alert" then
			state.added += 1
			local preset = PRESETS[(state.added - 1) % #PRESETS + 1]
			local copy = table.clone(preset)
			copy.id = `{preset.id}-{state.added}`
			table.insert(state.alerts, copy)
		elseif msg == "autoscale" then
			state.autoscale = not state.autoscale
		elseif msg == "zero-downtime" then
			state.zero_downtime = not state.zero_downtime
		elseif type(msg) == "table" then
			if msg.topic == "gantry/nav" and type(msg.value) == "table" then
				if msg.value.service ~= state.service then
					state.pinned = nil
				end
				state.page = msg.value.page
				state.service = msg.value.service
			elseif msg.tab then
				state.tab = msg.tab
			elseif msg.pin then
				state.pinned = msg.pin
			elseif msg.pick then
				state.range = msg.pick
				state.range_open = false
				state.pinned = nil
			elseif msg.alert then
				for _, alert in state.alerts do
					if alert.id == msg.alert then
						alert.on = not alert.on
					end
				end
			elseif msg.remove then
				for index, alert in state.alerts do
					if alert.id == msg.remove then
						table.remove(state.alerts, index)
						break
					end
				end
			elseif msg.replicas then
				if msg.replicas == "min" then
					state.min_replicas = math.clamp(state.min_replicas + msg.step, 1, state.max_replicas)
				else
					state.max_replicas = math.clamp(state.max_replicas + msg.step, state.min_replicas, 64)
				end
			end
		end
		return state
	end,

	view = function(state: State, app)
		if state.page ~= "service" then
			return ui.column {}
		end
		local data = app.telemetry
		local service: any = nil
		if data and data.services then
			for _, entry in data.services do
				if entry.id == state.service then
					service = entry
				end
			end
		end

		local crumbs = ui.row {
			class = "crumbs",
			height = 40,
			shrink = false,
			align = "center",
			ui.row {
				class = "crumb",
				align = "center",
				gap = "normal",
				msg = { go = "services" },
				to = "console",
				ui.icon { name = "layout-dashboard", size = "small" },
				ui.text { "Services" },
			},
			ui.text { "/", class = "muted" },
			ui.text { "Service Details", class = "crumb-current" },
		}

		if not service then
			return ui.column {
				grow = true,
				gap = "none",
				crumbs,
				ui.column { grow = true, align = "center", justify = "center", ui.text { if data then "No such service" else "Waiting for telemetry…", class = "muted" } },
			}
		end

		local resources = service.resources
		local deploying = service.status == "deploying"

		-- The header.
		local tags = { tag { label = service.status, tone = if service.status == "active" then "success" elseif deploying then "info" else "warning", solid = true } }
		for _, label in service.tags do
			table.insert(tags, tag { label = label })
		end
		local header = ui.row {
			align = "start",
			gap = "normal",
			ui.row { class = "service-tile", width = 40, height = 40, shrink = false, align = "center", justify = "center", ui.icon { name = "cpu" } },
			ui.column {
				gap = "small",
				grow = true,
				ui.text { service.name, class = "service-name" },
				ui.row { gap = "small", align = "center", table.unpack(tags) },
			},
			ui.row {
				gap = "normal",
				align = "center",
				square_button { label = "Live logs", icon = "external-link", msg = "logs" },
				square_button { label = if deploying then "Deploying" else "New deploy", icon = "plus", variant = "light", msg = "deploy", busy = deploying },
			},
		}

		local description = ui.column {
			gap = "small",
			ui.text { "Description", class = "label" },
			ui.column { max_width = 600, gap = "none", ui.text { service.description, class = "description" } },
		}

		local tab_items = {}
		for index, id in TABS do
			table.insert(tab_items, ui.row {
				class = if index == #TABS then { "tab", "tab-last" } else "tab",
				height = 32,
				align = "center",
				selected = state.tab == id,
				msg = { tab = id },
				ui.text { string.upper(id), wrap = false },
			})
		end
		local tabs = ui.row { class = "tabs", hug = true, table.unpack(tab_items) }

		local function heading(text: string, action: any?): any
			return ui.row { align = "center", height = 32, ui.text { text, class = "heading" }, ui.spacer(), action }
		end

		local function kpis(cells: { { label: string, value: string, note: string } }): any
			local items = {}
			for index, cell in cells do
				table.insert(items, ui.column {
					class = if index == #cells then { "kpi", "kpi-last" } else "kpi",
					grow = true,
					ui.text { string.upper(cell.label), class = "muted-small" },
					ui.text { cell.value, class = "kpi-value" },
					ui.text { cell.note, class = "muted-small" },
				})
			end
			return frame_box { flush = true, ui.row { gap = "none", align = "stretch", table.unpack(items) } }
		end

		local function log_lines(count: number): { any }
			local lines = {}
			for index, line in service.logs do
				if index > count then
					break
				end
				table.insert(lines, ui.row {
					class = "log-line",
					ui.text { line.time, class = "log-time", wrap = false },
					ui.row { width = 52, shrink = false, ui.text { string.upper(line.level), class = `log-{line.level}`, wrap = false } },
					ui.text { line.text, truncate = true },
				})
			end
			if #lines == 0 then
				table.insert(lines, ui.text { "No output while the service is paused.", class = "muted" })
			end
			return lines
		end

		local count = RANGES[state.range] or 97
		local first = #service.history.cpu - count
		local function chart(series: { any }, opts: any): any
			local labels = {}
			for _, label in data.axis do
				if label.index > first then
					table.insert(labels, { index = label.index - first, text = label.text })
				end
			end
			return series_chart {
				width = opts.width,
				height = opts.height or 180,
				max = opts.max or 100,
				ticks = opts.ticks,
				unit = opts.unit,
				series = series,
				labels = labels,
				titles = slice(data.times, count),
				pinned = state.pinned or math.max(1, count - 24),
				legend = opts.legend,
			}
		end

		local range = dropdown {
			value = state.range,
			open = state.range_open,
			toggle = "toggle-range",
			dismiss = "close-range",
			selected = state.range,
			align = "end",
			width = 120,
			options = { { id = "6h", label = "6h" }, { id = "12h", label = "12h" }, { id = "24h", label = "24h" } },
		}

		local body: { any }
		if state.tab == "resource" then
			local panels = {}
			for index, r in RESOURCES do
				local res = resources[r.key]
				local limit_box = ui.row {
					class = "limit",
					height = 32,
					align = "center",
					state.configure and ui.row { class = "limit-step", height = 22, align = "center", msg = { limit = { service = service.id, resource = r.key, by = -1 } }, to = "provider:telemetry", ui.icon { name = "minus", size = "small" } },
					ui.text { tostring(res.limit), wrap = false },
					ui.text { res.limit_unit, class = "limit-unit", wrap = false },
					state.configure and ui.row { class = "limit-step", height = 22, align = "center", msg = { limit = { service = service.id, resource = r.key, by = 1 } }, to = "provider:telemetry", ui.icon { name = "plus", size = "small" } },
				}
				local used = if r.key == "gpu" then `{trim(res.used)} GPUS` elseif r.key == "memory" then `{trim(res.used)} GB` else `{trim(res.used)} CORES`
				table.insert(panels, ui.column {
					class = if index == #RESOURCES then { "panel", "panel-last" } else "panel",
					grow = true,
					ui.row {
						align = "center",
						gap = "normal",
						height = 26,
						ui.row { class = "panel-icon", width = 18, height = 18, align = "center", justify = "center", style = { background = r.color }, ui.icon { name = r.icon :: any, size = "small" } },
						ui.text { r.name, class = "heading" },
						ui.spacer(),
						tag { label = `{res.util}%`, tone = tone_for(res.util), solid = true },
					},
					ui.row { height = 14 },
					ui.row {
						ui.column { grow = true, gap = "small", ui.text { "Allocated", class = "label" }, ui.text { res.label, class = "stat-value", wrap = false } },
						ui.column { grow = true, gap = "small", ui.text { "Used", class = "label" }, ui.text { used, class = "stat-value", style = { color = r.color }, wrap = false } },
						ui.column { grow = true, gap = "small", ui.text { "Utilization", class = "label" }, ui.text { `{res.util}%`, class = "stat-value", wrap = false } },
					},
					ui.row { height = 16 },
					ui.row { ui.text { "Usage", class = "label" }, ui.spacer(), ui.text { `{res.util}%` } },
					ui.row { height = 4 },
					tick_meter { value = res.util / 100, color = r.color, ticks = 96, height = 24 },
					ui.row { height = 12 },
					ui.row { align = "center", ui.text { r.limit }, ui.spacer(), limit_box },
				})
			end

			local h = service.history
			local usage = frame_box {
				width = 760,
				heading("USAGE HISTORY", range),
				chart({
					{ name = "CPU", color = "#3b82f6", points = slice(h.cpu, count) },
					{ name = "GPU", color = "#22c55e", points = slice(h.gpu, count) },
					{ name = "Memory", color = "#f97316", points = slice(h.memory, count) },
				}, { width = 662, height = 180 }),
			}

			local alert_rows = {}
			for index, alert in state.alerts do
				local value = nil
				if alert.metric == "p95" then
					value = service.p95
				elseif alert.metric then
					value = resources[alert.metric].util
				end
				local firing = alert.on and value ~= nil and alert.threshold ~= nil and value >= alert.threshold
				table.insert(alert_rows, ui.row {
					class = if index == #state.alerts then { "alert-row", "alert-last" } else "alert-row",
					align = "center",
					context = { remove = alert.id },
					ui.row {
						class = "alert-icon",
						width = 38,
						height = 38,
						shrink = false,
						align = "center",
						justify = "center",
						ui.icon { name = if alert.kind == "blocked" then "ban" else "triangle-alert", tone = if alert.kind == "blocked" then "danger" else "warning" },
					},
					ui.column {
						gap = "small",
						grow = true,
						ui.row { gap = "normal", align = "center", ui.text { alert.title, class = "alert-title", wrap = false }, firing and tag { label = "Firing", tone = "error", solid = true } },
						ui.text { alert.detail, class = "muted", truncate = true },
					},
					switch { on = alert.on, small = true, msg = { alert = alert.id } },
				})
			end
			local alerts = frame_box {
				grow = true,
				heading("RESOURCE ALERT", square_button { label = "Add alert", icon = "plus", variant = "quiet", msg = "add-alert" }),
				ui.column { gap = "none", table.unpack(alert_rows) },
			}

			body = {
				heading("RESOURCE MANAGEMENT", square_button { label = if state.configure then "Done" else "Configure", icon = if state.configure then "check" else "settings", variant = "quiet", msg = "configure" }),
				ui.row { height = 10 },
				frame_box { flush = true, ui.row { gap = "none", align = "stretch", table.unpack(panels) } },
				ui.row { height = 24 },
				ui.row { gap = "large", align = "stretch", usage, alerts },
			}
		elseif state.tab == "overview" then
			local deploys = {}
			for _, deploy in service.deploys do
				table.insert(deploys, ui.row {
					class = "list-row",
					align = "center",
					tag { label = deploy.version, tone = if deploy.status == "live" then "success" else nil },
					ui.text { deploy.note, truncate = true },
					ui.spacer(),
					ui.text { `{deploy.who} · {deploy.when}`, class = "muted-small", wrap = false },
				})
			end
			local details = {}
			for _, pair in {
				{ "Regions", service.regions },
				{ "Version", service.version },
				{ "Deployed", service.deployed },
				{ "Replicas", tostring(service.replicas) },
				{ "Endpoint", `https://{service.id}.halcyon.run` },
				{ "Runtime", "gantry-runtime 0.31 · linux/amd64" },
			} do
				table.insert(details, ui.row { class = "kv", ui.row { width = 120, ui.text { pair[1], class = "label" } }, ui.text { pair[2], truncate = true } })
			end
			body = {
				kpis {
					{ label = "Requests", value = `{service.rps}/s`, note = "at the edge, now" },
					{ label = "P95 latency", value = `{service.p95} ms`, note = "token to client" },
					{ label = "Error rate", value = `{service.errors}%`, note = "failed runs, 24h" },
					{ label = "Replicas", value = tostring(service.replicas), note = service.regions },
				},
				ui.row { height = 24 },
				ui.row {
					gap = "large",
					align = "start",
					frame_box { grow = true, heading("DEPLOYS"), ui.column { gap = "none", table.unpack(deploys) } },
					frame_box { width = 520, heading("DETAILS"), ui.column { gap = "none", table.unpack(details) } },
				},
				ui.row { height = 24 },
				frame_box { heading("RECENT LOGS", square_button { label = "Live logs", icon = "external-link", variant = "quiet", msg = "logs" }), ui.column { gap = "none", table.unpack(log_lines(8)) } },
			}
		elseif state.tab == "analytics" then
			local rps = slice(service.history.rps, count)
			local p95 = slice(service.history.p95, count)
			local function ceiling(list: { number }, unit: string): (number, { string })
				local most = 1
				for _, value in list do
					most = math.max(most, value)
				end
				local top = math.max(4, math.ceil(most * 1.15 / 4) * 4)
				local ticks = {}
				for step = 4, 0, -1 do
					table.insert(ticks, `{math.round(top * step / 4)}{unit}`)
				end
				return top, ticks
			end
			local rps_max, rps_ticks = ceiling(rps, "")
			local p95_max, p95_ticks = ceiling(p95, "")
			body = {
				ui.row {
					gap = "large",
					align = "stretch",
					frame_box {
						grow = true,
						heading("REQUESTS / S", range),
						chart({ { name = "Requests", color = "#3b82f6", points = rps } }, { width = 560, height = 200, max = rps_max, ticks = rps_ticks, unit = "/s" }),
					},
					frame_box {
						grow = true,
						heading("P95 LATENCY (MS)"),
						chart({ { name = "P95", color = "#f97316", points = p95 } }, { width = 560, height = 200, max = p95_max, ticks = p95_ticks, unit = " ms" }),
					},
				},
				ui.row { height = 24 },
				kpis {
					{ label = "Peak requests", value = `{rps_max}/s`, note = `over {state.range}` },
					{ label = "Error budget", value = `{math.max(0, math.round((1 - service.errors / 2.5) * 100))}%`, note = "left this month" },
					{ label = "Apdex", value = if service.p95 > 800 then "0.71" else "0.96", note = "target 0.90" },
				},
			}
		else
			local function setting(title: string, detail: string, control: any): any
				return ui.row {
					class = "setting",
					align = "center",
					ui.column { gap = "small", grow = true, ui.text { title, class = "heading" }, ui.text { detail, class = "muted" } },
					control,
				}
			end
			body = {
				frame_box {
					heading("GENERAL"),
					ui.column {
						gap = "none",
						setting("Name", "Used in URLs, the CLI and metrics.", ui.input { id = `name-{service.id}`, value = service.name, width = 260, mono = true }),
						setting("Autoscaling", "Add replicas when utilization stays above 70%.", switch { on = state.autoscale, msg = "autoscale", small = true }),
						setting("Minimum replicas", "Kept warm in every region.", stepper { value = state.min_replicas, min = 1, max = state.max_replicas, decrement = { replicas = "min", step = -1 }, increment = { replicas = "min", step = 1 } }),
						setting("Maximum replicas", "The ceiling autoscaling stops at.", stepper { value = state.max_replicas, min = state.min_replicas, max = 64, decrement = { replicas = "max", step = -1 }, increment = { replicas = "max", step = 1 } }),
						setting("Zero-downtime deploys", "Drain each replica before replacing it.", switch { on = state.zero_downtime, msg = "zero-downtime", small = true }),
					},
				},
				ui.row { height = 24 },
				frame_box {
					heading("DANGER ZONE"),
					setting("Pause service", "Stops every replica; requests get 503 until resumed.", ui.row { class = "danger-button", height = 34, align = "center", msg = "pause", ui.icon { name = "circle-pause", size = "small" }, ui.text { "PAUSE", size = "small", weight = "medium" } }),
				},
			}
		end

		local logs = sheet {
			open = state.logs,
			dismiss = "close-logs",
			title = "LIVE LOGS",
			subtitle = `{service.name} · {service.regions}`,
			icon = "terminal",
			width = 620,
			ui.column { gap = "none", table.unpack(log_lines(40)) },
		}

		return ui.column {
			grow = true,
			gap = "none",
			crumbs,
			ui.column {
				grow = true,
				scroll = true,
				gap = "none",
				ui.column {
					class = "page",
					shrink = false,
					header,
					ui.row { height = 14 },
					description,
					ui.row { height = 24 },
					tabs,
					ui.row { height = 22 },
					table.unpack(body),
				},
			},
			logs,
		}
	end,
}
