-- The Overview: a time range over a grid of figures (within SLA, latency
-- and lag against their limits, workflows active and paused, success
-- against errors, what fails most), then the live feed of runs, or the
-- schedule of what runs next. Its figures are `app.vigil`: sample data
-- the provider simulates live, a run starting every few seconds and the
-- running ones finishing. The range and Re-run are messages to it.

local threshold_line = use "threshold-line"
local share_bar = use "share-bar"
local menu = use "menu"
local kbd = use "kbd"

type State = { page: string, range: string, tab: string, actions: boolean }

local RANGES = { "1H", "4H", "24H", "72H", "7D", "30D" }

-- The grid: four columns of one width; wide cards span two.
local COLUMN = 250
local GAP = 6
local WIDE = COLUMN * 2 + GAP
local PAD = 20
local CONTENT = COLUMN * 4 + GAP * 3

local STATUS = {
	failed = { icon = "circle-x", label = "Failed", class = "status-failed" },
	running = { icon = "clock", label = "Running", class = "status-running" },
	success = { icon = "circle-check", label = "Success", class = "status-success" },
}

return mod {
	region = "main",
	title = "Overview",
	subscribe = { "provider/vigil" },

	styles = {
		[".page"] = { padding = { 22, 0, 0, 0 }, gap = 0 },
		[".range"] = { padding = { 0, 16 }, radius = 17 },
		[".range:hover"] = { background = "list-hover" },
		[".range:selected"] = { background = "chip" },
		[".range-text"] = { color = "label", weight = "medium" },
		[".range-text:on"] = { color = "foreground" },
		[".pill"] = { background = "chip", radius = 17, padding = { 0, 10, 0, 14 }, gap = 6 },
		[".pill:hover"] = { background = "chip-hover" },
		[".pill-text"] = { weight = "medium" },
		[".pill-icon"] = { color = "muted-foreground" },

		[".card"] = { background = "card", radius = 20, padding = { 18, PAD, 20, PAD }, gap = 0 },
		[".card-title"] = { color = "label" },
		[".card-open"] = { radius = 6, padding = 2 },
		[".card-open:hover"] = { background = "list-hover" },
		[".card-open-icon"] = { color = "muted-foreground" },
		[".figure"] = { size = "hero", weight = "normal" },
		[".key"] = { color = "muted-foreground" },
		[".value"] = { color = "foreground" },
		[".split-number"] = { color = "foreground" },
		[".split-word"] = { color = "muted-foreground" },
		[".errored-name"] = { weight = "medium" },
		[".errored-kind"] = { color = "muted-foreground" },
		[".errored-failed"] = { color = "danger" },
		[".errored-slash"] = { color = "muted-foreground" },
		[".errored-track"] = { background = "#4a4a4a", radius = 2 },
		[".errored-fill"] = { background = "danger", radius = 2 },

		[".tab"] = { padding = { 0, 14 }, radius = 18, gap = 8 },
		[".tab:hover"] = { background = "list-hover" },
		[".tab:selected"] = { background = "chip" },
		[".tab-text"] = { color = "label", weight = "medium" },
		[".tab-text:on"] = { color = "foreground" },
		[".live-halo"] = { radius = 8, transition = "900ms ease-in-out" },
		[".live-halo:on"] = { width = 14, height = 14, background = "danger/35" },
		[".live-halo:off"] = { width = 6, height = 6, background = "danger/0" },
		[".live-dot"] = { background = "danger", radius = 3 },

		[".table"] = { background = "card", radius = { 20, 20, 0, 0 }, padding = { 6, PAD, 0, PAD }, gap = 0 },
		[".table-head"] = { color = "label" },
		[".table-row"] = { border = "top", border_color = "#ffffff12", transition = "slide-fade fast ease-out" },
		[".table-first"] = { border = "none" },
		[".cell-muted"] = { color = "muted-foreground" },
		[".status-failed"] = { color = "danger" },
		[".status-running"] = { color = "label" },
		[".status-success"] = { color = "muted-foreground" },
		[".status-icon-success"] = { color = "#bdbdbd" },
		[".rerun"] = { background = "accent", radius = 18, padding = { 0, 14, 0, 12 }, gap = 6, color = "#ffffff" },
		[".rerun:hover"] = { background = "#ff8c5c" },
		[".rerun-text"] = { weight = "medium", color = "#ffffff" },
		[".round"] = { background = "chip", radius = 17 },
		[".round:hover"] = { background = "chip-hover" },
		[".sent"] = { color = "muted-foreground", gap = 6 },

		[".placeholder"] = { padding = 40, gap = 8 },
		[".actions-menu"] = { background = "#303030", radius = 14, padding = 6, border = true, border_color = "#ffffff14", shadow = "0 12px 32px #00000080", margin = { 0, 24, 64, 0 } },
	},

	init = function(previous: State?): State
		local state: State = previous or { page = "overview", range = "1H", tab = "live", actions = false }
		state.actions = false
		return state, { { to = "provider:vigil", msg = { range = state.range } } }
	end,

	update = function(state: State, msg)
		if msg == "actions" then
			state.actions = not state.actions
		elseif msg == "close-actions" then
			state.actions = false
		elseif type(msg) == "table" then
			if msg.topic == "provider/vigil" then
				-- The provider starts on 1H: put it back on the range kept.
				local value = msg.value
				if value and value.available and value.range ~= state.range then
					return state, { { to = "provider:vigil", msg = { range = state.range } } }
				end
				return state
			elseif msg.page then
				state.page = msg.page
			elseif msg.range then
				state.range = msg.range
				return state, { { to = "provider:vigil", msg = { range = msg.range } } }
			elseif msg.tab then
				state.tab = msg.tab
			elseif msg.rerun then
				return state, { { to = "provider:vigil", msg = { rerun = msg.rerun } } }
			elseif msg.pick then
				state.actions = false
				if msg.pick == "live" or msg.pick == "upcoming" then
					state.tab = msg.pick
				elseif string.sub(msg.pick, 1, 6) == "rerun:" then
					return state, { { to = "provider:vigil", msg = { rerun = string.sub(msg.pick, 7) } } }
				else
					state.range = msg.pick
					return state, { { to = "provider:vigil", msg = { range = msg.pick } } }
				end
			end
		end
	end,

	view = function(state: State, app)
		if state.page ~= "overview" then
			return ui.column {
				class = "placeholder",
				grow = true,
				align = "center",
				justify = "center",
				ui.icon { name = "layout-grid", size = "title", tone = "muted" },
				ui.text { "Only the Overview is drawn in this example.", tone = "muted" },
			}
		end
		local data = app.vigil
		if not data or not data.available then
			return ui.column { class = "placeholder", ui.text { "Loading the sample workflows…", tone = "muted" } }
		end

		local function open_button(target: string)
			return ui.row {
				class = "card-open",
				msg = { page = target },
				to = "sidebar",
				tooltip = "Open",
				ui.icon { name = "arrow-up-right", class = "card-open-icon" },
			}
		end
		local function head(title: string, target: string)
			return ui.row {
				align = "center",
				height = 24,
				ui.text { title, class = "card-title" },
				ui.spacer(),
				open_button(target),
			}
		end
		local function pairs_table(rows: { { string } }, gap: number)
			local keys, values = {}, {}
			for _, pair in rows do
				table.insert(keys, ui.text { pair[1], class = "key", wrap = false })
				table.insert(values, ui.text { pair[2], class = "value", wrap = false })
			end
			return ui.row {
				style = { gap = gap },
				ui.column { style = { gap = 4 }, table.unpack(keys) },
				ui.column { style = { gap = 4 }, align = "end", table.unpack(values) },
			}
		end
		local function split_line(left_number: string, left_word: string, right_number: string, right_word: string)
			return ui.row {
				align = "center",
				ui.row { gap = "small", ui.text { left_number, class = "split-number" }, ui.text { left_word, class = "split-word" } },
				ui.spacer(),
				ui.row { gap = "small", ui.text { right_number, class = "split-number" }, ui.text { right_word, class = "split-word" } },
			}
		end

		-- The range and the way to Analytics.
		local ranges = {}
		for _, id in RANGES do
			local on = id == state.range
			table.insert(ranges, ui.row {
				class = "range",
				height = 34,
				align = "center",
				selected = on,
				msg = { range = id },
				ui.text { id, class = "range-text", state = if on then "on" else nil },
			})
		end
		local top = ui.row {
			align = "center",
			height = 34,
			ui.row { gap = "small", table.unpack(ranges) },
			ui.spacer(),
			ui.row {
				class = "pill",
				height = 34,
				align = "center",
				msg = { page = "analytics" },
				to = "sidebar",
				ui.text { "Analytics", class = "pill-text" },
				ui.icon { name = "arrow-up-right", size = "small" },
			},
		}

		local sla = data.sla
		local sla_card = ui.column {
			class = "card",
			width = WIDE,
			height = 229,
			head("% Within SLA", "analytics"),
			ui.row {
				align = "start",
				style = { margin = { 2, 0, 0, 0 } },
				ui.text { sla.value, class = "figure" },
				ui.spacer(),
				ui.row { style = { margin = { 14, 0, 0, 0 } }, pairs_table({ { "Latency", sla.latency }, { "Freshness", sla.freshness } }, 70) },
			},
			ui.spacer(),
			threshold_line {
				points = sla.points,
				threshold = 95,
				label = "95%",
				breach = "below",
				width = WIDE - 2 * PAD,
				height = 70,
				tooltip = "Share of runs within SLA; 95% is the target",
			},
		}

		local function limit_card(title: string, figure: any, threshold: number, label: string, tip: string)
			return ui.column {
				class = "card",
				width = COLUMN,
				height = 229,
				head(title, "analytics"),
				ui.row {
					align = "start",
					style = { margin = { 2, 0, 0, 0 } },
					ui.text { figure.value, class = "figure" },
					ui.spacer(),
					ui.row { style = { margin = { 14, 0, 0, 0 } }, pairs_table({ { "P99", figure.p99 }, { "P95", figure.p95 } }, 18) },
				},
				ui.spacer(),
				threshold_line {
					points = figure.points,
					threshold = threshold,
					label = label,
					breach = "above",
					width = COLUMN - 2 * PAD,
					height = 62,
					tooltip = tip,
				},
			}
		end

		local flows = data.workflows
		local total_card = ui.column {
			class = "card",
			width = COLUMN,
			height = 228,
			head("Total workflows", "workflows"),
			ui.text { tostring(flows.total), class = "figure", style = { margin = { 2, 0, 0, 0 } } },
			ui.spacer(),
			split_line(tostring(flows.active), "active", tostring(flows.paused), "paused"),
			ui.row { height = 12 },
			share_bar {
				width = COLUMN - 2 * PAD,
				height = 4,
				shares = {
					{ share = flows.active, color = "success", label = `{flows.active} active` },
					{ share = flows.paused, color = "#5a5a5a", label = `{flows.paused} paused` },
				},
			},
		}

		local success = data.success
		local success_card = ui.column {
			class = "card",
			width = COLUMN,
			height = 228,
			head("Workflow success", "runs"),
			ui.text { success.rate, class = "figure", style = { margin = { 2, 0, 0, 0 } } },
			ui.spacer(),
			split_line(success.success, "success", success.errors, "errors"),
			ui.row { height = 12 },
			share_bar {
				width = COLUMN - 2 * PAD,
				height = 4,
				shares = {
					{ share = success.share, color = "success", label = `{success.success} succeeded` },
					{ share = 1 - success.share, color = "danger", label = `{success.errors} failed` },
				},
			},
		}

		local errored_rows = {}
		for _, row in data.errored do
			table.insert(errored_rows, ui.column {
				style = { gap = 6 },
				ui.row {
					align = "center",
					ui.text { row.name, class = "errored-name" },
					ui.spacer(),
					ui.row {
						gap = "small",
						tooltip = `{row.runs} runs, {row.failed} failed`,
						ui.text { row.runs },
						ui.text { "/", class = "errored-slash" },
						ui.text { row.failed, class = "errored-failed" },
					},
				},
				ui.row {
					align = "center",
					ui.text { "└", class = "errored-kind", style = { margin = { 0, 8, 0, 6 } } },
					ui.text { row.kind, class = "errored-kind" },
					ui.spacer(),
					ui.row {
						class = "errored-track",
						width = 52,
						height = 4,
						ui.row { class = "errored-fill", width = math.max(6, 52 * row.share), height = 4 },
					},
					ui.row { width = 40, justify = "end", ui.text { row.count } },
				},
			})
		end
		local errored_card = ui.column {
			class = "card",
			width = WIDE,
			height = 228,
			head("Most errored", "runs"),
			ui.column { style = { gap = 12, margin = { 14, 0, 0, 0 } }, table.unpack(errored_rows) },
		}

		local grid = ui.column {
			style = { gap = GAP },
			ui.row {
				style = { gap = GAP },
				sla_card,
				limit_card("Avg latency", data.latency, 2, "2S", "Average latency; the limit is 2 seconds"),
				limit_card("Average lag", data.lag, 300, "5M", "How far data trails its source; the limit is 5 minutes"),
			},
			ui.row { style = { gap = GAP }, total_card, success_card, errored_card },
		}

		-- Live feed and the schedule, with a dot that breathes each tick.
		local live = state.tab == "live"
		local breathing = if data.tick % 2 == 0 then "on" else "off"
		local tabs = ui.row {
			align = "center",
			height = 36,
			ui.row {
				class = "tab",
				height = 36,
				align = "center",
				selected = live,
				msg = { tab = "live" },
				ui.row {
					width = 14,
					height = 14,
					align = "center",
					justify = "center",
					ui.row { class = "live-halo", state = breathing },
					ui.row { class = "live-dot", width = 6, height = 6, float = { top = 4, left = 4 } },
				},
				ui.text { "Live feed", class = "tab-text", state = if live then "on" else nil },
			},
			ui.row {
				class = "tab",
				height = 36,
				align = "center",
				selected = not live,
				msg = { tab = "upcoming" },
				ui.text { "Upcoming schedule", class = "tab-text", state = if live then nil else "on" },
			},
			ui.spacer(),
			ui.row {
				class = "pill",
				height = 34,
				align = "center",
				msg = { page = "runs" },
				to = "sidebar",
				ui.text { "Runs & Logs", class = "pill-text" },
				ui.icon { name = "arrow-up-right", size = "small" },
			},
		}

		-- One table row: fixed columns, then what fills, then the actions.
		local function cells(first: boolean, key: string?, a: any, b: any, c: any, d: any, e: any, f: any)
			return ui.row {
				class = if first then { "table-row", "table-first" } else "table-row",
				key = key,
				height = if key then 58 else 50,
				shrink = false,
				align = "center",
				gap = "none",
				ui.row { width = 144, a },
				ui.row { width = 150, b },
				ui.row { width = 62, justify = "end", c },
				ui.row { width = 52 },
				ui.row { width = 136, d },
				ui.row { grow = true, e },
				ui.row { width = 160, justify = "end", gap = "small", align = "center", f },
			}
		end
		local function status(name: string)
			local look = STATUS[name] or STATUS.success
			return ui.row {
				gap = "small",
				align = "center",
				ui.icon { name = look.icon :: any, class = if name == "success" then "status-icon-success" else look.class },
				ui.text { look.label, class = look.class },
			}
		end

		local rows = {}
		if live then
			table.insert(rows, cells(true, nil,
				ui.text { "Started", class = "table-head" },
				ui.text { "Workflow", class = "table-head" },
				ui.text { "Duration", class = "table-head" },
				ui.text { "Status", class = "table-head" },
				ui.text { "Error", class = "table-head" },
				nil))
			for index, run in data.feed do
				local actions = nil
				if run.status == "failed" and not run.retried then
					actions = ui.row {
						gap = "small",
						align = "center",
						ui.row {
							class = "rerun",
							height = 34,
							align = "center",
							msg = { rerun = run.id },
							tooltip = `Run {run.workflow} again`,
							ui.icon { name = "rotate-cw", size = "small", class = "rerun-text" },
							ui.text { "Re-run", class = "rerun-text" },
						},
						ui.row {
							class = "round",
							width = 34,
							height = 34,
							align = "center",
							justify = "center",
							msg = { page = "runs" },
							to = "sidebar",
							tooltip = "Open the run's logs",
							ui.icon { name = "arrow-up-right", size = "small" },
						},
					}
				elseif run.status == "failed" then
					actions = ui.row { class = "sent", align = "center", ui.icon { name = "check", size = "small" }, ui.text { "Re-run started", class = "cell-muted" } }
				end
				table.insert(rows, cells(index == 1, run.id,
					ui.text { run.started, wrap = false },
					ui.text { run.workflow, wrap = false, truncate = true },
					ui.text { run.duration, wrap = false },
					status(run.status),
					ui.text { run.error, truncate = true },
					actions))
			end
		else
			table.insert(rows, cells(true, nil,
				ui.text { "Next run", class = "table-head" },
				ui.text { "Workflow", class = "table-head" },
				ui.text { "", class = "table-head" },
				ui.text { "Last run", class = "table-head" },
				ui.text { "Schedule", class = "table-head" },
				nil))
			for index, slot in data.upcoming do
				table.insert(rows, cells(index == 1, "u-" .. string.lower((string.gsub(slot.id, "%W", "-"))),
					ui.text { slot.next, wrap = false },
					ui.text { slot.workflow, wrap = false, truncate = true },
					nil,
					status(slot.last),
					ui.text { slot.every, class = "cell-muted" },
					nil))
			end
		end
		local feed = ui.column {
			class = "table",
			grow = true,
			ui.column { grow = true, scroll = true, gap = "none", table.unpack(rows) },
		}

		local quick: { any } = {}
		for _, run in data.feed do
			if run.status == "failed" and not run.retried then
				table.insert(quick, { id = "rerun:" .. run.id, label = `Re-run {run.workflow}`, icon = "rotate-cw" })
				break
			end
		end
		table.insert(quick, { id = if live then "upcoming" else "live", label = if live then "Show upcoming schedule" else "Show live feed", icon = "clock" })
		table.insert(quick, "separator")
		table.insert(quick, { heading = "Range" })
		for _, id in { "1H", "24H", "7D" } do
			table.insert(quick, { id = id, label = `Last {id}`, icon = "calendar", checked = id == state.range })
		end

		return ui.row {
			grow = true,
			justify = "center",
			ui.overlay {
				open = state.actions,
				dismiss = "close-actions",
				place = "bottom-end",
				width = 260,
				class = "actions-menu",
				content = ui.column { gap = "small", menu { items = quick }, ui.row { justify = "end", kbd { keys = "secondary-/", small = true } } },
			},
			ui.column {
				class = "page",
				width = CONTENT,
				shrink = false,
				top,
				ui.row { height = 16 },
				grid,
				ui.row { height = 34 },
				tabs,
				ui.row { height = 14 },
				feed,
			},
		}
	end,
}
