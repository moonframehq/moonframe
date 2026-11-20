-- A small system diagram on a grid: nodes, links between neighbours, groups
-- around nodes, and tokens that travel the links. The caller names things
-- and places them in grid cells; the component draws them at fractions of
-- its box, so one diagram fits a side panel and a full pane.

type Meter = { text: string, level: number?, tone: string? }
type Node = {
	id: string,
	label: string,
	sublabel: string?,
	icon: string?,
	col: number,
	row: number,
	tone: string?,
	note: string?,
	meter: Meter?,
	planned: boolean?,
}
type Link = { from: string, to: string, label: string? }
type Group = { label: string, nodes: { string } }
type Flow = { path: { string }, tokens: { string }?, lane: number? }
-- Where a node sits: the centre of its cell, as fractions of the box.
type Point = { x: number, y: number, col: number, row: number }
type Size = { w: number, h: number, slot: number, icon: number }
-- One step of a flow's path, `len` cells long.
type Step = { from: Point, to: Point, len: number }
type Props = {
	nodes: { Node },
	links: { Link }?,
	groups: { Group }?,
	flows: { Flow }?,
	columns: number?,
	rows: number?,
	time: number?,
	speed: number?,
	selected: string?,
	event: string?,
	to: string?,
	size: string?,
	height: number?,
}

local SIZES: { [string]: Size } = {
	small = { w = 126, h = 56, slot = 20, icon = 22 },
	normal = { w = 136, h = 60, slot = 24, icon = 26 },
}
local TONES = { accent = true, success = true, warning = true, danger = true }

local function tone_of(name: string?): string?
	return if name and TONES[name] then name else nil
end

-- The space a small chevron icon leaves past its tip, in pixels: the head
-- sits this much into the card's margin, so its tip meets the card's edge.
local HEAD_INSET = 4
-- How far a token on lane 1 or -1 sits from the line, in pixels: two flows
-- on one link stay apart in any box.
local LANE_GAP = 7
-- A token's width and height, in pixels.
local TOKEN = 10

-- A link, as two layers under the nodes. The line runs from centre to
-- centre, so it shows in whatever gap the two cards leave, however narrow.
-- The head sits at the edge of the card it points at; where the cards leave
-- no room for it, it falls under the card it starts from. Its layer is
-- half a cell across, so it scales with the cells. Its word is drawn by
-- `link_label`.
local function link(a: Point, b: Point, size: Size, cols: number, rows: number, into: { any })
	local level = a.row == b.row
	local stack: (any) -> any = if level then ui.row else ui.column
	local forward = if level then b.x > a.x else b.y > a.y
	local head = if level
		then (if forward then "chevron-right" else "chevron-left")
		else (if forward then "chevron-down" else "chevron-up")
	local area = if level
		then { left = math.min(a.x, b.x), top = a.y - 0.25 / rows, width = math.abs(b.x - a.x), height = 0.5 / rows }
		else { left = a.x - 0.25 / cols, top = math.min(a.y, b.y), width = 0.5 / cols, height = math.abs(b.y - a.y) }
	local edge = if level
		then ui.row { width = size.w / 2 - HEAD_INSET, shrink = false }
		else ui.row { height = size.h / 2 - HEAD_INSET, shrink = false }
	local line = if level
		then ui.row { class = "flow-diagram-line", grow = true, height = 2 }
		else ui.column { class = "flow-diagram-line", grow = true, width = 2 }
	table.insert(into, stack { area = area, align = "center", gap = "none", line })
	table.insert(
		into,
		stack {
			area = area,
			align = "center",
			gap = "none",
			not forward and edge,
			forward and stack { grow = true },
			ui.icon { name = head :: any, size = "small", class = "flow-diagram-head" },
			not forward and stack { grow = true },
			forward and edge,
		}
	)
end

-- The word on a link, drawn after the nodes so no card covers it: above the
-- two cards of a level link, centred between them; beside an upright link,
-- near the card it points at.
local function link_label(a: Point, b: Point, size: Size, cols: number, rows: number, label: string)
	local word = ui.text { label, class = "flow-diagram-link-label" }
	if a.row == b.row then
		return ui.column {
			area = { left = math.min(a.x, b.x), top = a.y - 0.5 / rows, width = math.abs(b.x - a.x), height = 0.5 / rows },
			align = "center",
			justify = "end",
			gap = "none",
			word,
			ui.row { height = size.h / 2 + 2 },
		}
	end
	local down = b.y > a.y
	return ui.column {
		area = { left = a.x, top = math.min(a.y, b.y), width = 0.5 / cols, height = math.abs(b.y - a.y) },
		align = "start",
		justify = if down then "end" else "start",
		gap = "none",
		not down and ui.row { height = size.h / 2 + 2 },
		ui.row { class = "flow-diagram-link-label-side", word },
		down and ui.row { height = size.h / 2 + 2 },
	}
end

-- Whether a link can be drawn between `a` and `b`: only level and upright
-- ones can. A slanted link is left out, and tokens do not travel it.
local function drawable(a: Point, b: Point): boolean
	return a.row == b.row or a.col == b.col
end

-- The key of the link between two nodes, the same either way round.
local function pair(a: string, b: string): string
	return if a < b then `{a}\0{b}` else `{b}\0{a}`
end

-- The tokens of one flow, spread evenly along its path at this time. A
-- step's length is the cells it crosses, so a `speed` is the same in any
-- grid. Tokens travel only the drawn links (`linked`, by `pair`): a step
-- with no link, or through a node that does not exist, is left out, and
-- the tokens skip over it.
local function tokens(
	flow: Flow,
	at: { [string]: Point },
	linked: { [string]: boolean },
	seconds: number,
	speed: number,
	cols: number,
	rows: number,
	into: { any }
)
	local list: { string } = flow.tokens or {}
	if #list == 0 then
		return
	end
	local steps: { Step }, total = {}, 0
	local last: Point?, last_id: string? = nil, nil
	for _, id in flow.path do
		local here = at[id]
		if here and last and last_id and linked[pair(last_id, id)] then
			local len = math.abs(here.col - last.col) + math.abs(here.row - last.row)
			if len > 0 then
				table.insert(steps, { from = last, to = here, len = len })
				total += len
			end
		end
		-- An unknown node breaks the path: no step jumps over it.
		last, last_id = here, if here then id else nil
	end
	if total <= 0 then
		return
	end
	local lane = math.clamp(flow.lane or 0, -1, 1)
	for n, tone in list do
		local along = (seconds * speed + (n - 1) * total / #list) % total
		local i = 1
		while i < #steps and along > steps[i].len do
			along -= steps[i].len
			i += 1
		end
		local from, to = steps[i].from, steps[i].to
		local f = along / steps[i].len
		local x, y = from.x + (to.x - from.x) * f, from.y + (to.y - from.y) * f
		local dot = ui.row {
			class = { "flow-diagram-token", `flow-diagram-token-{tone_of(tone) or "plain"}` },
			width = TOKEN,
			height = TOKEN,
			shrink = false,
		}
		-- Two flows on one link go side by side, a fixed number of pixels
		-- apart: a spacer twice the gap on one side moves the centred dot by
		-- the gap. A level step moves it down or up, an upright one across.
		local level = from.row == to.row
		local spacer = lane ~= 0
			and (if level then ui.row { height = 2 * LANE_GAP, shrink = false } else ui.row { width = 2 * LANE_GAP, shrink = false })
		local stack: (any) -> any = if level then ui.column else ui.row
		-- A box of one cell around the point: it scales with the cells.
		table.insert(
			into,
			stack {
				area = { left = x - 0.5 / cols, top = y - 0.5 / rows, width = 1 / cols, height = 1 / rows },
				align = "center",
				justify = "center",
				gap = "none",
				lane > 0 and spacer,
				dot,
				lane < 0 and spacer,
			}
		)
	end
end

local function card(node: Node, size: Size, props: Props)
	local tone = tone_of(node.tone)
	local meter = node.meter
	local meter_tone = meter and tone_of(meter.tone) or "plain"
	local classes, icon_classes = { "flow-diagram-node" }, { "flow-diagram-icon" }
	if tone then
		table.insert(classes, `flow-diagram-node-{tone}`)
		table.insert(icon_classes, `flow-diagram-icon-{tone}`)
	end
	if node.planned then
		table.insert(classes, "flow-diagram-node-planned")
	end
	local selected = props.selected == node.id
	local plate = { "flow-diagram-plate" }
	if node.planned then
		table.insert(plate, "flow-diagram-plate-flat")
	end
	-- An opaque plate under the card: a picked card's tint and a planned
	-- card's fade show the plate, never the lines and tokens under it.
	return ui.column {
		class = plate,
		selected = selected,
		-- At most its size, and never more than its cell.
		grow = true,
		max_width = size.w,
		height = size.h,
		gap = "none",
		msg = { [props.event or "select"] = node.id },
		to = props.to,
		-- A name cut short in a narrow cell shows whole here.
		tooltip = if node.sublabel then `{node.label}: {node.sublabel}` else node.label,
		ui.column {
			class = classes,
			selected = selected,
			grow = true,
			justify = "center",
			gap = "none",
			ui.row {
				class = "flow-diagram-top",
				align = "center",
				ui.row {
					class = icon_classes,
					width = size.icon,
					height = size.icon,
					shrink = false,
					align = "center",
					justify = "center",
					ui.icon { name = (node.icon or "circle") :: any, size = "small" },
				},
				-- No width of its own, only what is free: a narrow card cuts the text short.
				ui.column {
					grow = true,
					width = 0,
					gap = "none",
					ui.text { node.label, class = "flow-diagram-label", truncate = true },
					node.sublabel and not meter and ui.text { node.sublabel, class = "flow-diagram-sublabel", truncate = true },
				},
			},
			meter and ui.row {
				class = "flow-diagram-meter",
				align = "center",
				ui.row {
					class = "flow-diagram-track",
					width = 36,
					height = 4,
					shrink = false,
					gap = "none",
					ui.row {
						class = `flow-diagram-fill-{meter_tone}`,
						width = math.max(2, 36 * math.clamp(meter.level or 0, 0, 1)),
						height = 4,
					},
				},
				ui.row { grow = true, width = 0, ui.text { meter.text, class = { "flow-diagram-meter-text", `flow-diagram-text-{meter_tone}` }, truncate = true } },
			},
		},
	}
end

local EXAMPLE_LINKS: { Link } = {
	{ from = "web", to = "api", label = "HTTPS" },
	{ from = "api", to = "db" },
	{ from = "api", to = "cache" },
}

return component {
	title = "Flow Diagram",
	category = "Data",
	icon = "workflow",
	description = "A small system diagram on a grid: named nodes with an icon and a meter, links between neighbours, a tinted group around nodes, and coloured tokens that travel the links.",
	when = "Explain how the parts of a system talk to each other, and what moves between them now: a request path, a cache miss, a retry, a pipeline. The caller places each node in a grid cell, so the same diagram fits a side panel and a full pane. Pass a growing `time` to move the tokens; leave it out for a still diagram. For a line of commits use `commit-graph`; for a tree use `tree`; for places on a map use `route-map`.",
	props = {
		["nodes"] = "{ id, label, sublabel?, icon?, col, row, tone?, note?, meter? = { text, level?, tone? }, planned? }: `col` and `row` count from 0 (a negative one moves the whole grid over); `tone` is accent | success | warning | danger",
		["links"] = "{ from, to, label? }: two nodes in one row or one column (a slanted link is left out); the head points at `to`; `label` shows above a level link and beside an upright one",
		["groups"] = "{ label, nodes = { ids } }: a tinted box around the cells of its nodes, its label at the top right",
		["flows"] = "{ path = { ids }, tokens = { tones }, lane? }: one token for each tone, spread along the path. Tokens travel only steps with a link in `links` (either way round), skipping any other step; `lane` (-1, 0, 1) keeps two flows on one link apart",
		["columns"] = "the grid's width in cells (default: what the nodes use)",
		["rows"] = "the grid's height in cells (default: what the nodes use)",
		["time"] = "seconds: where the tokens are. The caller owns the clock",
		["speed"] = "how far a token goes each second, in cells along its path (default 0.55): the same in any grid",
		["selected"] = "the id of the node to show as picked",
		["event"] = "key of the message a click on a node sends, default \"select\": { select = id }",
		["to"] = "mod to send the click to, if not the drawing one",
		["size"] = "small | normal: the most a node takes. In a narrow cell it narrows to fit and cuts its name short (the tooltip shows it whole); where two cards leave no room for a link's head, the link shows as a short line in the gap between them",
		["height"] = "pixels (default 120 for each row)",
	},
	styles = {
		[".flow-diagram"] = { background = "muted/40", radius = 10 },
		[".flow-diagram-line"] = { background = "muted-foreground/55" },
		[".flow-diagram-head"] = { color = "muted-foreground" },
		-- On a chip: a word that crosses a group's edge or a line stays clear.
		[".flow-diagram-link-label"] = {
			color = "muted-foreground",
			font_size = 10,
			background = "background",
			radius = 4,
			padding = { 0, 4 },
		},
		[".flow-diagram-link-label-side"] = { padding = { left = 10 } },
		[".flow-diagram-cell"] = { padding = { x = 4 } },
		[".flow-diagram-group"] = { background = "accent/8", border = "all", border_color = "accent/30", radius = 12 },
		[".flow-diagram-group-label"] = { color = "accent", font_size = 10, weight = "medium", padding = { 4, 8 } },
		[".flow-diagram-token"] = { radius = 5, border = "all", border_color = "background" },
		[".flow-diagram-token-plain"] = { background = "muted-foreground" },
		[".flow-diagram-token-accent"] = { background = "accent" },
		[".flow-diagram-token-success"] = { background = "success" },
		[".flow-diagram-token-warning"] = { background = "warning" },
		[".flow-diagram-token-danger"] = { background = "danger" },
		[".flow-diagram-plate"] = { background = "background", radius = 12, shadow = "small" },
		[".flow-diagram-plate:selected"] = { shadow = "none" },
		[".flow-diagram-plate-flat"] = { shadow = "none" },
		[".flow-diagram-node"] = {
			border = "all",
			border_color = "muted-foreground/60",
			radius = 12,
			padding = { x = 9 },
		},
		[".flow-diagram-node:hover"] = { border_color = "accent" },
		[".flow-diagram-node:selected"] = { border_color = "accent", background = "accent/8" },
		[".flow-diagram-node-accent"] = { border_color = "accent" },
		[".flow-diagram-node-success"] = { border_color = "success" },
		[".flow-diagram-node-warning"] = { border_color = "warning" },
		[".flow-diagram-node-danger"] = { border_color = "danger" },
		[".flow-diagram-node-planned"] = { opacity = 0.55 },
		[".flow-diagram-top"] = { gap = 8 },
		[".flow-diagram-icon"] = { radius = 7, background = "muted", color = "muted-foreground" },
		[".flow-diagram-icon-accent"] = { background = "accent/15", color = "accent" },
		[".flow-diagram-icon-success"] = { background = "success/15", color = "success" },
		[".flow-diagram-icon-warning"] = { background = "warning/15", color = "warning" },
		[".flow-diagram-icon-danger"] = { background = "danger/15", color = "danger" },
		[".flow-diagram-label"] = { weight = "medium" },
		[".flow-diagram-sublabel"] = { color = "muted-foreground", font_size = 11 },
		[".flow-diagram-meter"] = { gap = 6, padding = { top = 4 } },
		[".flow-diagram-track"] = { background = "muted", radius = 2, clip = true },
		[".flow-diagram-fill-plain"] = { background = "muted-foreground", radius = 2 },
		[".flow-diagram-fill-accent"] = { background = "accent", radius = 2 },
		[".flow-diagram-fill-success"] = { background = "success", radius = 2 },
		[".flow-diagram-fill-warning"] = { background = "warning", radius = 2 },
		[".flow-diagram-fill-danger"] = { background = "danger", radius = 2 },
		[".flow-diagram-meter-text"] = { font_size = 11, color = "muted-foreground" },
		[".flow-diagram-text-plain"] = { color = "muted-foreground" },
		[".flow-diagram-text-accent"] = { color = "accent" },
		[".flow-diagram-text-success"] = { color = "success" },
		[".flow-diagram-text-warning"] = { color = "warning" },
		[".flow-diagram-text-danger"] = { color = "danger" },
		[".flow-diagram-note"] = { radius = 6, padding = { 1, 7 }, font_size = 11, background = "background", color = "muted-foreground" },
	},
	example = {
		nodes = {
			{ id = "web", label = "Web app", icon = "globe", col = 0, row = 0 },
			{ id = "api", label = "API", sublabel = "request handler", icon = "server", col = 1, row = 0, tone = "accent" },
			{ id = "db", label = "Database", icon = "database", col = 2, row = 0 },
			{ id = "cache", label = "Cache", icon = "clock", col = 1, row = 1, meter = { text = "hit 4 in 5", level = 0.8, tone = "success" } },
		},
		links = EXAMPLE_LINKS,
		groups = { { label = "Backend", nodes = { "api", "db", "cache" } } },
		flows = { { path = { "web", "api", "cache" }, tokens = { "warning", "warning", "success" } } },
		time = 1.2,
		selected = "api",
		event = "node",
	},
	usage = [[local flow_diagram = use "flow-diagram"

flow_diagram {
	nodes = {
		{ id = "web", label = "Web app", icon = "globe", col = 0, row = 0 },
		{ id = "api", label = "API", icon = "server", col = 1, row = 0 },
		{ id = "db", label = "Database", icon = "database", col = 2, row = 0 },
	},
	links = { { from = "web", to = "api", label = "HTTPS" }, { from = "api", to = "db" } },
	flows = { { path = { "web", "api", "db" }, tokens = { "warning", "warning", "danger" } } },
	time = state.seconds,      -- from a clock the mod owns
	selected = state.node,
	event = "node",            -- a click sends { node = id }
}]],

	build = function(props: Props)
		local size = SIZES[props.size or "normal"] or SIZES.normal
		-- A negative cell moves the whole grid over, so every node is inside the box.
		local min_col, min_row = 0, 0
		for _, node in props.nodes do
			min_col, min_row = math.min(min_col, node.col), math.min(min_row, node.row)
		end
		local cols, rows = props.columns or 1, props.rows or 1
		for _, node in props.nodes do
			cols, rows = math.max(cols, node.col - min_col + 1), math.max(rows, node.row - min_row + 1)
		end
		local at: { [string]: Point }, kids: { any } = {}, {}
		for _, node in props.nodes do
			local col, row = node.col - min_col, node.row - min_row
			at[node.id] = { x = (col + 0.5) / cols, y = (row + 0.5) / rows, col = col, row = row }
		end
		for _, group in ipairs(props.groups or {}) do
			local left, top, right, bottom = cols, rows, -1, -1
			for _, id in group.nodes do
				local where = at[id]
				if where then
					left, top = math.min(left, where.col), math.min(top, where.row)
					right, bottom = math.max(right, where.col), math.max(bottom, where.row)
				end
			end
			if right >= 0 then
				table.insert(
					kids,
					ui.column {
						class = "flow-diagram-group",
						area = {
							left = (left + 0.04) / cols,
							top = (top + 0.04) / rows,
							width = (right - left + 0.92) / cols,
							height = (bottom - top + 0.92) / rows,
						},
						-- At the right: a level link's word often sits on the left edge.
						ui.row { justify = "end", ui.text { group.label, class = "flow-diagram-group-label" } },
					}
				)
			end
		end
		local linked: { [string]: boolean } = {}
		for _, each in ipairs(props.links or {}) do
			local a, b = at[each.from], at[each.to]
			-- Only level and upright links can be drawn; a slanted one is left out.
			if a and b and drawable(a, b) then
				link(a, b, size, cols, rows, kids)
				linked[pair(each.from, each.to)] = true
			end
		end
		if props.time then
			for _, flow in ipairs(props.flows or {}) do
				tokens(flow, at, linked, props.time, props.speed or 0.55, cols, rows, kids)
			end
		end
		for _, node in props.nodes do
			local where = at[node.id]
			table.insert(
				kids,
				ui.column {
					key = node.id,
					area = { left = where.x - 0.5 / cols, top = where.y - 0.5 / rows, width = 1 / cols, height = 1 / rows },
					align = "center",
					justify = "center",
					gap = "none",
					-- An empty slot above balances the note below: the card stays on the links.
					ui.row { height = size.slot },
					ui.row { class = "flow-diagram-cell", self_align = "stretch", justify = "center", card(node, size, props) },
					ui.row {
						height = size.slot,
						align = "end",
						justify = "center",
						node.note and ui.row { class = "flow-diagram-note", hug = true, ui.text { node.note } },
					},
				}
			)
		end
		for _, each in ipairs(props.links or {}) do
			local a, b = at[each.from], at[each.to]
			if a and b and each.label and drawable(a, b) then
				table.insert(kids, link_label(a, b, size, cols, rows, each.label))
			end
		end
		return ui.row { class = "flow-diagram", height = props.height or rows * 120, table.unpack(kids) }
	end,
}
