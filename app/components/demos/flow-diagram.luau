-- A cache in front of a database: four requests in five are answered by the
-- cache, one goes on to the database. Drag the time to move the tokens (a
-- mod with a clock passes its own `time`); click a node to pick it.
local flow_diagram = use "flow-diagram"
local segmented = use "segmented"
local slider = use "slider"

local NODES: { any } = {
	{ id = "web", label = "Web app", sublabel = "browser", icon = "globe", col = 0, row = 0 },
	{ id = "api", label = "API", sublabel = "request handler", icon = "server", col = 1, row = 0 },
	{ id = "db", label = "Database", icon = "database", col = 2, row = 0, meter = { text = "1 in 5 reads", level = 0.2 } },
	{
		id = "cache",
		label = "Cache",
		icon = "clock",
		col = 1,
		row = 1,
		tone = "success",
		note = "new",
		meter = { text = "hit 4 in 5", level = 0.8, tone = "success" },
	},
	{ id = "queue", label = "Job queue", sublabel = "not built yet", icon = "list-ordered", col = 2, row = 1, planned = true },
}

local LINKS: { any } = {
	{ from = "web", to = "api", label = "HTTPS" },
	{ from = "api", to = "db", label = "on a miss" },
	{ from = "api", to = "cache", label = "read" },
	{ from = "cache", to = "queue" },
}

local GROUPS = { { label = "Backend", nodes = { "api", "db", "cache", "queue" } } }

local FLOWS: { any } = {
	{ path = { "web", "api", "cache" }, tokens = { "warning", "warning", "warning", "warning" }, lane = -1 },
	{ path = { "cache", "api", "web" }, tokens = { "success", "success", "success", "success" }, lane = 1 },
	{ path = { "api", "db" }, tokens = { "danger" }, lane = -1 },
}

local SIZES = {
	{ id = "normal", label = "Normal" },
	{ id = "small", label = "Small" },
}

return mod {
	init = function(previous)
		return previous or { time = 1.2, size = "normal", node = "cache" }
	end,

	update = function(state, msg)
		if type(msg) ~= "table" then
			return
		end
		if msg.time and msg.value then
			state.time = msg.value
		elseif msg.size then
			state.size = msg.size
		elseif msg.node then
			state.node = if state.node == msg.node then nil else msg.node
		end
	end,

	view = function(state)
		return ui.column {
			gap = "normal",
			width = 680,
			ui.row {
				align = "center",
				gap = "normal",
				slider {
					label = "Time",
					value = state.time,
					min = 0,
					max = 20,
					step = 0.1,
					decimals = 1,
					unit = " s",
					change = { time = true },
					inline = true,
					width = 220,
				},
				segmented { options = SIZES, selected = state.size, small = true, event = "size" },
			},
			flow_diagram {
				nodes = NODES,
				links = LINKS,
				groups = GROUPS,
				flows = FLOWS,
				time = state.time,
				size = state.size,
				selected = state.node,
				event = "node",
			},
			ui.text { if state.node then `Picked: {state.node}` else "Click a node to pick it", tone = "muted" },
		}
	end,
}
