-- A process monitor pane: a search, processes grouped by name with heat
-- bars behind CPU and memory (click a group's arrow for its members, a
-- heading to sort), the CPU over the last two minutes, and memory in use.
-- Live from Lantern's `procs` provider (`ps`, `iostat` and `vm_stat` on
-- macOS; the system's own counters on Linux and Windows).

local progress = use "progress"

type State = { query: string, sort: string }

type Row = { pid: number, name: string, count: number, cpu: number, mem: number, open: boolean, child: boolean }

local GB = 1024 * 1024 * 1024
local MB = 1024 * 1024

local function bytes(value: number): string
	if value >= GB then
		return string.format("%.2f GB", value / GB)
	elseif value >= MB then
		return string.format("%d MB", math.floor(value / MB + 0.5))
	end
	return string.format("%d KB", math.floor(value / 1024 + 0.5))
end

local function gb(value: number): string
	return string.format("%.1f GB", value / GB)
end

-- A value right-aligned over a bar as wide as its share.
local function heat(text: string, share: number, width: number, class: string)
	local fill = math.clamp(share, 0, 1) * width
	return ui.row {
		width = width,
		height = 20,
		align = "center",
		justify = "end",
		shrink = false,
		fill >= 2 and ui.row { class = class, float = { top = 1, right = 0 }, width = fill, height = 18 },
		ui.text { text, class = "mon-value" },
	}
end

return mod {
	title = "Processes",

	styles = {
		[".mon-search"] = { padding = { 8, 10, 6, 10 } },
		[".mon-head"] = { padding = { 0, 10, 0, 10 }, gap = 8, border = "bottom", border_color = "border", color = "muted-foreground" },
		[".mon-head-text"] = { size = "small", weight = "medium" },
		[".mon-sort"] = { gap = 2, radius = 4 },
		[".mon-sort:hover"] = { color = "foreground" },
		[".mon-row"] = { padding = { 0, 10, 0, 4 }, gap = 8 },
		[".mon-row:hover"] = { background = "list-hover" },
		[".mon-arrow"] = { color = "muted-foreground", radius = 3 },
		[".mon-arrow:hover"] = { color = "foreground", background = "list-hover" },
		[".mon-pid"] = { color = "muted-foreground" },
		[".mon-times"] = { color = "muted-foreground", size = "small" },
		[".mon-value"] = { font = "mono" },
		[".mon-heat"] = { background = "pumpkin/30", radius = 2 },
		[".mon-heat-mem"] = { background = "#ffffff12", radius = 2 },
		[".mon-stats"] = { padding = { 10, 10, 10, 10 }, gap = 6, border = "top", border_color = "border" },
		[".mon-label"] = { size = "small", color = "muted-foreground", weight = "medium" },
		[".mon-big"] = { weight = "medium" },
		[".mon-graph"] = { color = "pumpkin", background = "pumpkin/22" },
		[".progress-track"] = { background = "#ffffff10" },
		[".progress-fill"] = { background = "ember" },
		[".mon-note"] = { size = "small", color = "muted-foreground" },
		[".mon-pressure"] = { size = "small", color = "ember" },
		[".mon-user"] = { background = "pumpkin", radius = 4 },
		[".mon-system"] = { background = "#b9b0c6", radius = 4 },
	},

	init = function(previous: State?): State
		return previous or { query = "", sort = "cpu" }
	end,

	update = function(state: State, msg)
		if type(msg) ~= "table" then
			if msg == "clear" then
				state.query = ""
				return state, { { to = "provider:procs", msg = { query = "" } } }
			end
			return
		end
		if msg.msg == "query" then
			state.query = msg.value
			return state, { { to = "provider:procs", msg = { query = msg.value } } }
		elseif msg.sort then
			state.sort = msg.sort
			return state, { { to = "provider:procs", msg = { sort = msg.sort } } }
		elseif msg.expand then
			return state, { { to = "provider:procs", msg = { expand = msg.expand } } }
		end
	end,

	view = function(state: State, app)
		local procs = app.procs
		local search = ui.row {
			class = "mon-search",
			shrink = false,
			ui.input {
				id = "query",
				grow = true,
				value = state.query,
				placeholder = "Search process or pid…",
				change = "query",
				keys = { escape = "clear" },
			},
		}
		if not procs then
			return ui.column {
				grow = true,
				gap = "none",
				search,
				ui.column { grow = true, align = "center", justify = "center", ui.text { "Reading processes…", tone = "muted" } },
			}
		elseif not procs.available then
			return ui.column {
				grow = true,
				gap = "none",
				search,
				ui.column { grow = true, align = "center", justify = "center", ui.text { procs.reason or "Unavailable", tone = "muted" } },
			}
		end

		-- The rows: each group, then its members when open.
		local rows: { Row } = {}
		local top_cpu, top_mem = 100, 1
		for _, group in procs.groups do
			top_cpu = math.max(top_cpu, group.cpu)
			top_mem = math.max(top_mem, group.mem)
			table.insert(rows, { pid = group.pid, name = group.name, count = group.count, cpu = group.cpu, mem = group.mem, open = group.open, child = false })
			for _, child in group.children or {} do
				table.insert(rows, { pid = child.pid, name = child.name, count = 1, cpu = child.cpu, mem = child.mem, open = false, child = true })
			end
		end

		local function heading(label: string, key: string?, width: number?)
			local sorted = key ~= nil and procs.sort == key
			return ui.row {
				class = if key then "mon-sort" else nil,
				width = width,
				grow = width == nil,
				justify = if key then "end" else "start",
				align = "center",
				msg = if key then { sort = key } else nil,
				tooltip = if key then `Sort by {string.lower(label)}` else nil,
				ui.text { label, class = "mon-head-text" },
				sorted and ui.icon { name = "chevron-down", size = "small" },
			}
		end

		local list = ui.list {
			id = "procs",
			count = #rows,
			row_height = 23,
			row = function(i)
				local row = rows[i]
				return ui.row {
					class = "mon-row",
					align = "center",
					height = 23,
					ui.row {
						width = 14,
						justify = "center",
						shrink = false,
						row.count > 1 and not row.child and ui.row {
							class = "mon-arrow",
							msg = { expand = row.name },
							tooltip = if row.open then "Hide its processes" else "Show its processes",
							ui.icon { name = if row.open then "chevron-down" else "chevron-right", size = "small" },
						},
					},
					ui.row { width = 52, shrink = false, ui.text { tostring(row.pid), class = "mon-pid" } },
					ui.row {
						grow = true,
						gap = "small",
						align = "center",
						row.child and ui.row { width = 10 },
						ui.text { row.name, truncate = true, tooltip = row.name },
						row.count > 1 and ui.text { `×{row.count}`, class = "mon-times" },
					},
					heat(string.format("%.1f%%", row.cpu), row.cpu / top_cpu, 74, "mon-heat"),
					heat(bytes(row.mem), row.mem / top_mem, 76, "mon-heat-mem"),
				}
			end,
		}

		local memory = procs.memory
		local share = if memory.total > 0 then memory.used / memory.total else 0
		local load = procs.load
		local history = procs.history
		if #history < 2 then
			history = { procs.cpu, procs.cpu }
		end

		local stats = ui.column {
			class = "mon-stats",
			shrink = false,
			ui.row {
				gap = "small",
				align = "center",
				ui.text { "CPU", class = "mon-label" },
				ui.text { `{math.floor(procs.cpu + 0.5)}%`, class = "mon-big" },
			},
			ui.line { points = history, min = 0, max = 100, area = true, baseline = 0, class = "mon-graph", height = 44 },
			ui.row {
				gap = "normal",
				align = "center",
				ui.row { gap = "small", align = "center", ui.row { class = "mon-user", width = 7, height = 7 }, ui.text { `user {procs.user}%`, class = "mon-note" } },
				ui.row { gap = "small", align = "center", ui.row { class = "mon-system", width = 7, height = 7 }, ui.text { `system {procs.system}%`, class = "mon-note" } },
				-- Windows keeps no load average, and the provider sends zeros:
				-- shown, they would read as an idle machine.
				if load[1] == 0 and load[2] == 0 and load[3] == 0
					then nil
					else ui.text {
						string.format("load %.2f · %.2f · %.2f", load[1], load[2], load[3]),
						class = "mon-note",
						truncate = true,
					},
			},
			ui.row { height = 4 },
			ui.row {
				gap = "small",
				align = "center",
				ui.text { "MEMORY", class = "mon-label" },
				ui.text { `{gb(memory.used)} of {gb(memory.total)}`, class = "mon-big" },
				ui.spacer(),
				memory.pressure ~= "normal" and ui.text {
					if memory.pressure == "critical" then "Critical pressure" else "Under pressure",
					class = "mon-pressure",
				},
			},
			progress { value = share, width = 300, height = 6, percent = false },
			ui.row {
				gap = "normal",
				wrap = true,
				ui.text { `active {bytes(memory.active)}`, class = "mon-note" },
				ui.text { `wired {bytes(memory.wired)}`, class = "mon-note" },
				ui.text { `compressed {bytes(memory.compressed)}`, class = "mon-note" },
				ui.text { `swap {bytes(memory.swap)}`, class = "mon-note" },
			},
		}

		return ui.column {
			grow = true,
			gap = "none",
			search,
			ui.row {
				class = "mon-head",
				height = 28,
				align = "center",
				shrink = false,
				ui.row { width = 14 },
				heading("PID", nil, 52),
				heading("PROCESS", nil, nil),
				heading("CPU", "cpu", 74),
				heading("MEMORY", "memory", 76),
			},
			ui.column { grow = true, list },
			stats,
		}
	end,
}
