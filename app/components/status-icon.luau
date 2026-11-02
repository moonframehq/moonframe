-- A task's status as a small round glyph, as issue trackers draw it: a
-- dotted ring (backlog), a ring (todo), a ring with a pie filling up (in
-- progress, in review), a filled disc with a check (done) or a cross
-- (cancelled). Built from boxes and lines, so it stays crisp at any size.

type Props = {
	status: string?,
	progress: number?,
	color: string?,
	size: number?,
	tooltip: string?,
	label: (string | boolean)?,
}

-- Each status's name and colour.
local STATUSES: { [string]: { name: string, color: string } } = {
	backlog = { name = "Backlog", color = "muted-foreground" },
	todo = { name = "Todo", color = "muted-foreground" },
	progress = { name = "In Progress", color = "warning" },
	review = { name = "In Review", color = "accent" },
	done = { name = "Done", color = "success" },
	cancelled = { name = "Cancelled", color = "muted-foreground" },
}

-- What `ui.line` keeps around its plot: the stroke plus 3 pixels.
local STROKE = 1.5
local PAD = STROKE + 3

-- A polyline over a `width` x `height` box at `left`, `top` inside the
-- glyph, its points evenly spaced across it (0 the bottom, 1 the top).
local function stroke(points: { number }, left: number, top: number, width: number, height: number, color: string)
	return ui.row {
		float = { top = top - PAD, left = left - PAD },
		ui.line {
			points = points,
			min = 0,
			max = 1,
			stroke = STROKE,
			width = width + 2 * PAD,
			height = height + 2 * PAD,
			style = { color = color },
		},
	}
end

-- The pie inside a ring: `quarters` of a disc, clockwise from the top.
local function pie(quarters: number, diameter: number, color: string)
	local q = diameter / 2
	-- top, left and rounded corner of each quarter, clockwise from 12 o'clock.
	local QUARTERS: { { top: number, left: number, radius: { number } } } = {
		{ top = 0, left = q, radius = { 0, q, 0, 0 } },
		{ top = q, left = q, radius = { 0, 0, q, 0 } },
		{ top = q, left = 0, radius = { 0, 0, 0, q } },
		{ top = 0, left = 0, radius = { q, 0, 0, 0 } },
	}
	local parts = {}
	for index = 1, quarters do
		local quarter = QUARTERS[index]
		table.insert(parts, ui.row {
			float = { top = quarter.top, left = quarter.left },
			width = q,
			height = q,
			style = { background = color, radius = quarter.radius },
		})
	end
	return ui.row { width = diameter, height = diameter, shrink = false, table.unpack(parts) }
end

-- Dots around a ring of `size`, for the backlog.
local function dotted(size: number, color: string)
	local count = if size >= 16 then 10 else 8
	local dot = math.max(2, size / 7)
	local radius = (size - dot) / 2
	local dots = {}
	for index = 1, count do
		local angle = (index - 1) / count * 2 * math.pi
		table.insert(dots, ui.row {
			float = { top = radius - math.cos(angle) * radius, left = radius + math.sin(angle) * radius },
			width = dot,
			height = dot,
			style = { background = color, radius = dot },
		})
	end
	return dots
end

local function glyph(status: string, size: number, color: string, progress: number?)
	local box: { [any]: any } = { width = size, height = size, shrink = false, align = "center", justify = "center" }
	local parts: { any } = {}
	if status == "backlog" then
		parts = dotted(size, color)
	elseif status == "todo" then
		box.class = "status-icon-ring"
		box.style = { border_color = color }
	elseif status == "progress" or status == "review" then
		box.class = "status-icon-ring"
		box.style = { border_color = color }
		local fraction = progress or (if status == "review" then 0.75 else 0.5)
		local quarters = math.clamp(math.floor(fraction * 4 + 0.5), if fraction > 0 then 1 else 0, 4)
		local inner = size - 2 * math.max(3, math.floor(size / 5 + 0.5))
		table.insert(parts, pie(quarters, inner, color))
	else
		-- done or cancelled: a filled disc with a mark cut out of it.
		box.class = "status-icon-disc"
		box.style = { background = color }
		local mark = "background"
		if status == "done" then
			local width, height = size * 0.5, size * 0.36
			local left, top = (size - width) / 2, (size - height) / 2 + size * 0.02
			table.insert(parts, stroke({ 0.5, 0, 0.5, 1 }, left, top, width, height, mark))
		else
			local side = size * 0.32
			local at = (size - side) / 2
			table.insert(parts, stroke({ 1, 0 }, at, at, side, side, mark))
			table.insert(parts, stroke({ 0, 1 }, at, at, side, side, mark))
		end
	end
	for index, part in parts do
		box[index] = part
	end
	return ui.row(box :: any)
end

return component {
	title = "Status Icon",
	category = "Data",
	icon = "circle-dashed",
	description = "A task's status as a small round glyph: dotted ring (backlog), ring (todo), ring with a pie (in progress, in review), disc with a check (done) or a cross (cancelled).",
	when = "Beside a task's title in a list, a board card or a detail panel, and in the menu that changes it. For an agent's or job's state use `status-dot`; for a check the user ticks use `task-check`; for a priority use `priority-icon`.",
	props = {
		["status"] = "backlog, todo (default), progress, review, done or cancelled",
		["progress"] = "0 to 1: how much of the pie is filled for progress and review, in quarters (default a half, and three quarters for review)",
		["color"] = "any colour instead of the status's own (warning in progress, accent in review, success done, muted otherwise)",
		["size"] = "the glyph's side in pixels (default 14)",
		["tooltip"] = "text under the pointer (default the status's name)",
		["label"] = "`true` shows the status's name after the glyph, a string shows that text",
	},
	styles = {
		[".status-icon"] = { gap = 6 },
		[".status-icon-ring"] = { radius = 999, border = "all" },
		[".status-icon-disc"] = { radius = 999 },
		[".status-icon-label"] = { size = "small" },
	},
	example = { status = "progress", label = true },
	usage = [[local status_icon = use "status-icon"

ui.row { align = "center", gap = "normal",
	status_icon { status = "backlog" },
	status_icon { status = "todo" },
	status_icon { status = "progress", progress = 0.25 },
	status_icon { status = "review" },
	status_icon { status = "done" },
	status_icon { status = "cancelled", label = true },
}]],

	build = function(props: Props)
		local status = props.status or "todo"
		local known = STATUSES[status]
		if known == nil then
			status, known = "todo", STATUSES.todo
		end
		local size = props.size or 14
		local mark = glyph(status, size, props.color or known.color, props.progress)
		local label = if props.label == true then known.name elseif type(props.label) == "string" then props.label else nil
		return ui.row {
			class = "status-icon",
			align = "center",
			hug = true,
			shrink = false,
			tooltip = props.tooltip or known.name,
			mark,
			label and ui.text { label, class = "status-icon-label" },
		}
	end,
}
