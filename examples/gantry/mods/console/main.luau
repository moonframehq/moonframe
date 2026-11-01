-- The console: where the window is (a section, or one service), shared on
-- the `gantry/nav` topic for the top bar and the service page, and every
-- page but a service's details: Home, the Services list, Tasks, Docs,
-- Status and Changelog. All from `app.telemetry` (simulated sample data).

local frame_box = use "frame-box"
local square_button = use "square-button"
local tag = use "tag"
local tick_meter = use "tick-meter"

type Nav = { page: string, service: string }
type State = {
	page: string,
	service: string,
	filter: string,
	query: string,
	done: { [string]: boolean },
}

local PAGES = { home = "Home", services = "Services", tasks = "Tasks", docs = "Docs", status = "Status", changelog = "Changelog" }
local PAGE_ICONS = { home = "house", services = "layout-dashboard", tasks = "list-checks", docs = "file-text", status = "activity", changelog = "history" }
local FILTERS = { "all", "active", "degraded", "paused" }

-- How each kind of service is drawn: its first tag picks a tile.
local KINDS: { [string]: { icon: string, color: string } } = {
	["API"] = { icon = "cpu", color = "#f26b1d" },
	["STORAGE"] = { icon = "hard-drive", color = "#3b82f6" },
	["BATCH"] = { icon = "layers", color = "#8b5cf6" },
	["EDGE"] = { icon = "network", color = "#22c55e" },
	["AGENT RUNTIME"] = { icon = "bot", color = "#e11d48" },
}

local STATUS_TONE: { [string]: string } = { active = "success", degraded = "warning", paused = "muted", deploying = "info" }

local DOCS = {
	{ icon = "rocket", title = "Quick start", text = "Create a service from a template and ship its first deploy.", code = "gantry init --template edge-api" },
	{ icon = "upload", title = "Deploying", text = "Roll out a new version region by region, with automatic rollback.", code = "gantry deploy token-relay --canary 10%" },
	{ icon = "cpu", title = "Resource limits", text = "Allocate CPU cores, GPUs and memory per service, and cap bursts.", code = "gantry limits set --gpu 4 --mem 64Gi" },
	{ icon = "bell", title = "Alerts", text = "Fire on utilization, queue depth or latency, for a sustained window.", code = "gantry alerts add gpu>90% --for 5m" },
	{ icon = "terminal", title = "CLI reference", text = "Every command and flag of the gantry CLI, with examples.", code = "gantry help --all" },
	{ icon = "code", title = "HTTP API", text = "The REST API the console is built on: services, deploys, metrics.", code = "GET /v1/services/{id}/metrics?range=24h" },
}

local function service_kind(service: any): { icon: string, color: string }
	return KINDS[service.tags[1]] or KINDS["API"]
end

local function nav_of(state: State): Nav
	return { page = state.page, service = state.service }
end

local function publish(state: State)
	return state, { { to = "topic:gantry/nav", msg = nav_of(state) } }
end


return mod {
	region = "main",
	title = "Console",

	styles = {
		[".crumbs"] = { padding = { 0, 32 }, gap = 10, border = "bottom", border_color = "border" },
		[".crumb"] = { color = "muted-foreground" },
		[".crumb:hover"] = { color = "foreground" },
		[".crumb-current"] = { color = "bright", weight = "medium" },
		[".page"] = { padding = { 24, 32, 32, 32 }, gap = 22 },
		[".page-title"] = { size = "large", weight = "medium", color = "bright" },
		[".heading"] = { weight = "medium", color = "bright" },
		[".muted"] = { color = "muted-foreground" },
		[".muted-small"] = { color = "muted-foreground", size = "small" },
		[".kpi"] = { padding = { 16, 20 }, gap = 6, border = "right", border_color = "border" },
		[".kpi-last"] = { border = "none" },
		[".kpi-value"] = { size = "title", color = "bright" },
		[".seg"] = { border = true, border_color = "border", gap = 0 },
		[".seg-item"] = { padding = { 0, 12 }, border = "right", border_color = "border", color = "#bdbdbd" },
		[".seg-item:hover"] = { background = "#1d1d1d" },
		[".seg-item:selected"] = { background = "#ececec", color = "#111111" },
		[".seg-last"] = { border = "none" },
		[".table-head"] = { padding = { 0, 20 }, gap = 16, border = "bottom", border_color = "border", color = "muted-foreground" },
		[".table-row"] = { padding = { 0, 20 }, gap = 16, border = "bottom", border_color = "hairline" },
		[".table-row:hover"] = { background = "#1b1b1b" },
		[".tile"] = { color = "#ffffff" },
		[".row-name"] = { color = "bright", weight = "medium" },
		[".search"] = { width = 260 },
		[".list-row"] = { padding = { 12, 0 }, gap = 14, border = "bottom", border_color = "hairline" },
		[".check"] = { border = true, border_color = "#4a4a4a" },
		[".check:hover"] = { border_color = "#8a8a8a" },
		[".check-done"] = { background = "#e6e6e6", border_color = "#e6e6e6", color = "#111111" },
		[".struck"] = { color = "muted-foreground" },
		[".code"] = { background = "#111111", border = true, border_color = "hairline", padding = { 8, 10 }, color = "#c9c9c9", size = "small" },
		[".uptime-cell"] = { background = "#22c55e" },
		[".uptime-hot"] = { background = "#eab308" },
		[".uptime-off"] = { background = "#3a3a3a" },
		[".banner-ok"] = { background = "#22c55e14", border = true, border_color = "#22c55e40", padding = { 14, 18 }, gap = 12, color = "#22c55e" },
		[".banner-warn"] = { background = "#eab30814", border = true, border_color = "#eab30840", padding = { 14, 18 }, gap = 12, color = "#eab308" },
		[".dot"] = { radius = 999 },
		[".rail"] = { background = "border" },
	},

	init = function(previous: State?)
		local state: State = {
			page = if previous and (PAGES[previous.page] or previous.page == "service") then previous.page else "service",
			service = if previous then previous.service else "token-relay",
			filter = if previous then previous.filter else "all",
			query = "",
			done = if previous then previous.done or {} else {},
		}
		return publish(state)
	end,

	update = function(state: State, msg)
		if type(msg) ~= "table" then
			return
		end
		if msg.go then
			state.page = msg.go
			return publish(state)
		elseif msg.open then
			state.page = "service"
			state.service = msg.open
			return publish(state)
		elseif msg.filter then
			state.filter = msg.filter
		elseif msg.msg == "query" then
			state.query = msg.value or ""
		elseif msg.msg == "clear" then
			state.query = ""
		elseif msg.done then
			state.done[msg.done] = not state.done[msg.done]
		end
		return state
	end,

	view = function(state: State, app)
		if state.page == "service" then
			return ui.column {}
		end
		local data = app.telemetry
		local services: { any } = if data and data.services then data.services else {}

		local function crumbs(): any
			return ui.row {
				class = "crumbs",
				height = 40,
				shrink = false,
				align = "center",
				ui.icon { name = PAGE_ICONS[state.page] :: any, size = "small", tone = "muted" },
				ui.text { PAGES[state.page] or "", class = "crumb-current" },
			}
		end

		local function page(children: { any }): any
			return ui.column {
				grow = true,
				gap = "none",
				crumbs(),
				ui.column { grow = true, scroll = true, gap = "none", ui.column { class = "page", shrink = false, table.unpack(children) } },
			}
		end

		if not data or data.available == false then
			return page { ui.text { if data then data.reason or "No telemetry" else "Waiting for telemetry…", class = "muted" } }
		end

		local function tile(service: any, size: number): any
			local kind = service_kind(service)
			return ui.row {
				class = "tile",
				width = size,
				height = size,
				shrink = false,
				align = "center",
				justify = "center",
				style = { background = kind.color },
				ui.icon { name = kind.icon :: any, size = "small" },
			}
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

		-- Figures over the whole fleet.
		local healthy, rps, gpu_used, gpu_total, regions = 0, 0, 0, 0, {}
		for _, service in services do
			if service.status == "active" then
				healthy += 1
			end
			rps += service.rps
			local gpu = service.resources.gpu
			gpu_used += gpu.used
			gpu_total += gpu.allocated
			for region in string.gmatch(service.regions, "%a+") do
				regions[region] = true
			end
		end
		local region_count = 0
		for _ in regions do
			region_count += 1
		end
		local fleet = kpis {
			{ label = "Services", value = tostring(#services), note = `across {region_count} regions` },
			{ label = "Healthy", value = `{healthy}/{#services}`, note = if healthy == #services then "all green" else `{#services - healthy} need a look` },
			{ label = "Requests", value = `{rps}/s`, note = "summed at the edge" },
			{ label = "GPU fleet", value = `{math.round(gpu_used / math.max(gpu_total, 1) * 100)}%`, note = `{string.format("%.1f", gpu_used)} of {gpu_total} GPUs busy` },
		}

		local function meter_cell(value: number, color: string): any
			return ui.row {
				width = 132,
				shrink = false,
				align = "center",
				gap = "normal",
				ui.column { grow = true, tick_meter { value = value / 100, color = color, ticks = 28, height = 12 } },
				ui.row { width = 36, justify = "end", ui.text { `{value}%`, size = "small", wrap = false } },
			}
		end

		-- Services.
		if state.page == "services" then
			local query = string.lower(state.query or "")
			local rows: { any } = {}
			for _, service in services do
				local status = service.status
				local wanted = state.filter == "all" or status == state.filter or (state.filter == "active" and status == "deploying")
				local matches = query == "" or string.find(string.lower(service.name .. " " .. table.concat(service.tags, " ")), query, 1, true) ~= nil
				if wanted and matches then
					local tags = {}
					for _, label in service.tags do
						table.insert(tags, tag { label = label })
					end
					local r = service.resources
					table.insert(rows, ui.row {
						class = "table-row",
						height = 64,
						align = "center",
						msg = { open = service.id },
						ui.row {
							grow = true,
							align = "center",
							gap = "normal",
							tile(service, 30),
							ui.column {
								gap = "small",
								grow = true,
								ui.text { service.name, class = "row-name", wrap = false },
								ui.row { gap = "small", table.unpack(tags) },
							},
						},
						ui.row { width = 104, shrink = false, tag { label = status, tone = STATUS_TONE[status] } },
						ui.row { width = 132, shrink = false, ui.text { service.regions, class = "muted-small", wrap = false } },
						meter_cell(r.cpu.util, "#3b82f6"),
						meter_cell(r.gpu.util, "#22c55e"),
						meter_cell(r.memory.util, "#f97316"),
						ui.row { width = 64, shrink = false, justify = "end", ui.text { tostring(service.rps), wrap = false } },
						ui.row { width = 72, shrink = false, justify = "end", ui.text { `{service.p95} ms`, wrap = false } },
						ui.row { width = 84, shrink = false, justify = "end", ui.text { service.deployed, class = "muted-small", wrap = false } },
					})
				end
			end
			if #rows == 0 then
				table.insert(rows, ui.row { padding = "large", ui.text { "No service matches.", class = "muted" } })
			end

			local filters = {}
			for index, id in FILTERS do
				local count = 0
				for _, service in services do
					if id == "all" or service.status == id then
						count += 1
					end
				end
				table.insert(filters, ui.row {
					class = if index == #FILTERS then { "seg-item", "seg-last" } else "seg-item",
					height = 32,
					align = "center",
					selected = state.filter == id,
					msg = { filter = id },
					ui.text { `{string.upper(id)} {count}`, wrap = false },
				})
			end

			local head = ui.row {
				class = "table-head",
				height = 38,
				align = "center",
				ui.row { grow = true, ui.text { "NAME", size = "small" } },
				ui.row { width = 104, shrink = false, ui.text { "STATUS", size = "small" } },
				ui.row { width = 132, shrink = false, ui.text { "REGIONS", size = "small" } },
				ui.row { width = 132, shrink = false, ui.text { "CPU", size = "small" } },
				ui.row { width = 132, shrink = false, ui.text { "GPU", size = "small" } },
				ui.row { width = 132, shrink = false, ui.text { "MEMORY", size = "small" } },
				ui.row { width = 64, shrink = false, justify = "end", ui.text { "RPS", size = "small" } },
				ui.row { width = 72, shrink = false, justify = "end", ui.text { "P95", size = "small" } },
				ui.row { width = 84, shrink = false, justify = "end", ui.text { "DEPLOYED", size = "small" } },
			}

			return page {
				ui.row {
					align = "center",
					ui.column { gap = "small", grow = true, ui.text { "Services", class = "page-title" }, ui.text { "Everything running on the platform, by name. Open one for its resources, usage and alerts.", class = "muted" } },
					ui.input { id = "search", width = 260, value = state.query, placeholder = "Filter services…", change = "query", keys = { escape = "clear" }, mono = true },
					square_button { label = "New service", icon = "plus", variant = "light", msg = "new" },
				},
				fleet,
				ui.row { class = "seg", hug = true, table.unpack(filters) },
				frame_box { flush = true, head, table.unpack(rows) },
			}
		end

		-- Home: the fleet, recent deploys and what needs a look.
		if state.page == "home" then
			local deploys: { any } = {}
			for _, service in services do
				local latest = service.deploys[1]
				if latest then
					table.insert(deploys, ui.row {
						class = "list-row",
						align = "center",
						msg = { open = service.id },
						tile(service, 24),
						ui.text { service.name, class = "row-name", wrap = false },
						tag { label = latest.version },
						ui.text { latest.note, class = "muted", truncate = true },
						ui.spacer(),
						ui.text { `{latest.who} · {latest.when}`, class = "muted-small", wrap = false },
					})
				end
			end
			local attention: { any } = {}
			for _, service in services do
				local reasons = {}
				if service.status ~= "active" then
					table.insert(reasons, string.upper(service.status))
				end
				for _, key in { "cpu", "gpu", "memory" } do
					if service.resources[key].util >= 85 then
						table.insert(reasons, `{string.upper(key)} {service.resources[key].util}%`)
					end
				end
				if #reasons > 0 then
					table.insert(attention, ui.row {
						class = "list-row",
						align = "center",
						msg = { open = service.id },
						ui.icon { name = if service.status == "paused" then "circle-pause" else "triangle-alert", tone = if service.status == "paused" then "muted" else "warning" },
						ui.text { service.name, class = "row-name", wrap = false },
						ui.spacer(),
						ui.text { table.concat(reasons, " · "), class = "muted-small", wrap = false },
					})
				end
			end
			return page {
				ui.column { gap = "small", ui.text { "Good evening, Ines", class = "page-title" }, ui.text { `Halcyon Labs · {data.now} platform time · sample telemetry`, class = "muted" } },
				fleet,
				ui.row {
					gap = "large",
					align = "start",
					frame_box { grow = true, ui.text { "RECENT DEPLOYS", class = "heading" }, ui.column { gap = "none", table.unpack(deploys) } },
					frame_box { width = 460, ui.text { "NEEDS ATTENTION", class = "heading" }, ui.column { gap = "none", table.unpack(attention) } },
				},
			}
		end

		-- Tasks: what the fleet asks of you right now, ticked off here.
		if state.page == "tasks" then
			local tasks: { any } = {}
			local function task(id: string, title: string, detail: string, service: any)
				local done = state.done[id] == true
				table.insert(tasks, ui.row {
					class = "list-row",
					align = "center",
					ui.row {
						class = if done then { "check", "check-done" } else "check",
						width = 18,
						height = 18,
						align = "center",
						justify = "center",
						msg = { done = id },
						done and ui.icon { name = "check", size = "small" },
					},
					ui.column { gap = "none", grow = true, ui.text { title, class = if done then "struck" else "row-name", strike = done }, ui.text { detail, class = "muted-small" } },
					tile(service, 22),
					ui.text { service.name, class = "muted-small", wrap = false },
					square_button { icon = "arrow-right", small = true, msg = { open = service.id }, tooltip = "Open the service" },
				})
			end
			for _, service in services do
				local r = service.resources
				if r.gpu.util >= 80 then
					task(service.id .. "-gpu", "Rebalance GPU allocation", `GPU at {r.gpu.util}% of {r.gpu.label}`, service)
				end
				if r.memory.util >= 80 then
					task(service.id .. "-mem", "Raise the memory limit", `memory at {r.memory.util}% of {r.memory.label}`, service)
				end
				if service.errors >= 1 then
					task(service.id .. "-err", "Look into the error rate", `{service.errors}% of requests failing`, service)
				end
				if service.status == "paused" then
					task(service.id .. "-resume", "Resume the pool", "paused for a kernel image rebuild", service)
				end
				if service.status == "deploying" then
					task(service.id .. "-deploy", "Watch the rollout", "a new version is rolling out", service)
				end
			end
			return page {
				ui.column { gap = "small", ui.text { "Tasks", class = "page-title" }, ui.text { "Raised by the fleet as it runs; tick them off as you go.", class = "muted" } },
				frame_box { ui.column { gap = "none", table.unpack(tasks) } },
			}
		end

		-- Docs.
		if state.page == "docs" then
			local cards: { any } = {}
			for _, doc in DOCS do
				table.insert(cards, frame_box {
					width = 440,
					ui.row { gap = "normal", align = "center", ui.icon { name = doc.icon :: any, tone = "muted" }, ui.text { string.upper(doc.title), class = "heading" } },
					ui.text { doc.text, class = "muted" },
					ui.row { class = "code", ui.text { "$ " .. doc.code, wrap = false } },
				})
			end
			return page {
				ui.column { gap = "small", ui.text { "Docs", class = "page-title" }, ui.text { "Guides for running services on Gantry.", class = "muted" } },
				ui.row { gap = "large", wrap = true, table.unpack(cards) },
			}
		end

		-- Status: a day of each service in 48 half-hour cells.
		if state.page == "status" then
			local rows: { any } = {}
			local troubled = 0
			for _, service in services do
				local h = service.history
				local cells = {}
				for cell = 1, 48 do
					local hot = false
					for sample = (cell - 1) * 2 + 1, cell * 2 do
						if (h.cpu[sample] or 0) >= 90 or (h.gpu[sample] or 0) >= 90 or (h.memory[sample] or 0) >= 90 then
							hot = true
						end
					end
					local class = if service.status == "paused" then "uptime-off" elseif hot then "uptime-hot" else "uptime-cell"
					table.insert(cells, ui.row { class = class, grow = true, height = 26, tooltip = data.times[cell * 2] or "" })
				end
				if service.status ~= "active" then
					troubled += 1
				end
				table.insert(rows, ui.column {
					class = "list-row",
					msg = { open = service.id },
					ui.row {
						align = "center",
						gap = "normal",
						tile(service, 22),
						ui.text { service.name, class = "row-name" },
						tag { label = service.status, tone = STATUS_TONE[service.status] },
						ui.spacer(),
						ui.text { service.regions, class = "muted-small" },
					},
					ui.row { gap = "small", table.unpack(cells) },
				})
			end
			local banner = if troubled == 0
				then ui.row { class = "banner-ok", align = "center", ui.icon "circle-check", ui.text { "ALL SYSTEMS OPERATIONAL", weight = "medium" } }
				else ui.row { class = "banner-warn", align = "center", ui.icon "triangle-alert", ui.text { `{troubled} OF {#services} SERVICES NOT FULLY OPERATIONAL`, weight = "medium" } }
			return page {
				ui.column { gap = "small", ui.text { "Status", class = "page-title" }, ui.text { "The last 24 hours, half an hour per cell: yellow where a resource went past 90%.", class = "muted" } },
				banner,
				frame_box { ui.column { gap = "none", table.unpack(rows) } },
			}
		end

		-- Changelog: every service's deploys, newest first per service.
		local entries: { any } = {}
		for _, service in services do
			for _, deploy in service.deploys do
				table.insert(entries, ui.row {
					class = "list-row",
					align = "start",
					ui.column { width = 10, align = "center", padding = "small", ui.row { class = "dot", width = 8, height = 8, style = { background = if deploy.status == "live" then "#22c55e" else "#4a4a4a" } } },
					ui.column {
						gap = "small",
						grow = true,
						ui.row { gap = "normal", align = "center", tag { label = deploy.version, tone = if deploy.status == "live" then "success" else nil }, ui.text { service.name, class = "row-name" }, ui.text { deploy.note, class = "muted" } },
						ui.text { `{deploy.who} · {deploy.when}`, class = "muted-small" },
					},
				})
			end
		end
		return page {
			ui.column { gap = "small", ui.text { "Changelog", class = "page-title" }, ui.text { "What shipped, service by service.", class = "muted" } },
			frame_box { ui.column { gap = "none", table.unpack(entries) } },
		}
	end,
}
