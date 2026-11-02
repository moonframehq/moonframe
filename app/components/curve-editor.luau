-- A tone curve the user shapes by dragging points: press on the curve's
-- square to add a point and drag it, drag a point to move it between its
-- neighbours, double-click one to remove it. The curve runs smoothly
-- through the points without overshooting (monotone cubic), over an
-- optional histogram. The mod keeps the points; this sends them as they
-- change.

type Point = { number }

type Props = {
	points: { Point },
	change: any,
	size: number?,
	color: string?,
	values: { number }?,
	grid: number?,
	id: string?,
	disabled: boolean?,
}

type State = {
	dragging: number?,
}

-- A line keeps this much room inside its box (its stroke plus 3 pixels),
-- so the square the points live in is inset by it.
local STROKE = 1.5
local INSET = STROKE + 3
-- How near a press must be to a point to take it, and a point's size.
local REACH = 10
local HANDLE = 8
-- The closest two points may come along x.
local GAP = 0.01
-- Samples drawn along the curve.
local SAMPLES = 96

-- Slopes for a monotone cubic through `points` (Fritsch and Carlson), so
-- the curve never overshoots between them.
local function slopes(points: { Point }): { number }
	local count = #points
	local delta, slope = {}, {}
	for index = 1, count - 1 do
		local run = points[index + 1][1] - points[index][1]
		delta[index] = if run > 0 then (points[index + 1][2] - points[index][2]) / run else 0
	end
	slope[1] = delta[1] or 0
	slope[count] = delta[count - 1] or 0
	for index = 2, count - 1 do
		if delta[index - 1] * delta[index] <= 0 then
			slope[index] = 0
		else
			slope[index] = (delta[index - 1] + delta[index]) / 2
		end
	end
	for index = 1, count - 1 do
		if delta[index] == 0 then
			slope[index], slope[index + 1] = 0, 0
		else
			local a, b = slope[index] / delta[index], slope[index + 1] / delta[index]
			local length = a * a + b * b
			if length > 9 then
				local scale = 3 / math.sqrt(length)
				slope[index] = scale * a * delta[index]
				slope[index + 1] = scale * b * delta[index]
			end
		end
	end
	return slope
end

-- The curve's value at `x`, flat past its first and last point.
local function at(points: { Point }, slope: { number }, x: number): number
	local count = #points
	if count == 0 then
		return x
	end
	if x <= points[1][1] then
		return points[1][2]
	end
	if x >= points[count][1] then
		return points[count][2]
	end
	local index = 1
	while points[index + 1][1] < x do
		index += 1
	end
	local left, right = points[index], points[index + 1]
	local run = right[1] - left[1]
	local t = (x - left[1]) / run
	local t2, t3 = t * t, t * t * t
	return (2 * t3 - 3 * t2 + 1) * left[2]
		+ (t3 - 2 * t2 + t) * run * slope[index]
		+ (-2 * t3 + 3 * t2) * right[2]
		+ (t3 - t2) * run * slope[index + 1]
end

local function copy(points: { Point }): { Point }
	local out = table.create(#points)
	for index, point in points do
		out[index] = { point[1], point[2] }
	end
	return out
end

-- `change` with `points` and `phase` added.
local function message(change: any, points: { Point }, phase: string): any
	local msg = {}
	if typeof(change) == "table" then
		for key, field in change do
			msg[key] = field
		end
	else
		msg.msg = change
	end
	msg.points = points
	msg.phase = phase
	return msg
end

return component {
	title = "Curve Editor",
	category = "Forms",
	icon = "spline",
	description = "A square graph with a smooth curve through draggable points: press to add one, drag to move it between its neighbours, double-click to remove it; an optional histogram shows behind it.",
	when = "A tone or response curve the user shapes by hand: a photo's gradation curve per channel, an easing, a gain curve. For one value along a range use `slider`; for a fixed set of levels over a histogram use `histogram` with markers.",
	props = {
		["points"] = "the curve's points, { x, y } pairs from 0 to 1, sorted by x; at least two",
		["change"] = "message sent as the points change, with `points` and `phase` (down, move, up) added; record undo on `phase == \"up\"`",
		["size"] = "the square's side in pixels (default 200)",
		["color"] = "the curve's colour (default accent)",
		["values"] = "counts drawn faintly behind it, such as the channel's histogram",
		["grid"] = "lines across each way (default 3, quarters)",
		["id"] = "keeps its drag apart from another curve editor's in the same mod (default \"main\")",
		["disabled"] = "shows the curve without taking presses",
	},
	styles = {
		[".curve-editor"] = {
			background = "muted-foreground/8",
			radius = 6,
			border = "all",
			border_color = "border",
		},
		[".curve-editor-grid"] = { background = "muted-foreground/15" },
		[".curve-editor-handle"] = {
			background = "background",
			border = "all",
			border_color = "foreground",
			radius = 999,
		},
		[".curve-editor-handle:selected"] = { background = "accent", border_color = "accent" },
	},
	example = {
		points = { { 0, 0 }, { 0.3, 0.22 }, { 0.7, 0.82 }, { 1, 1 } },
		size = 180,
	},

	init = function(previous: State?): State
		return previous or { dragging = nil }
	end,

	update = function(state: State, msg: any, props: Props)
		if typeof(msg) ~= "table" or not msg.curve or props.disabled then
			return
		end
		local points = copy(props.points)
		local x = math.clamp(msg.x, 0, 1)
		local y = math.clamp(1 - msg.y, 0, 1)
		if msg.phase == "down" then
			-- The nearest point within reach, in pixels.
			local nearest, best = nil, REACH
			for index, point in points do
				local distance = math.sqrt(
					((point[1] - x) * msg.width) ^ 2 + ((1 - point[2] - (1 - y)) * msg.height) ^ 2
				)
				if distance <= best then
					nearest, best = index, distance
				end
			end
			if nearest and msg.clicks == 2 and nearest > 1 and nearest < #points then
				table.remove(points, nearest)
				state.dragging = nil
				return state, { { to = "parent", msg = message(props.change, points, "up") } }
			end
			if not nearest then
				local at = #points + 1
				for index, point in points do
					if point[1] > x then
						at = index
						break
					end
				end
				-- Only where it fits strictly between its neighbours, GAP
				-- from each: the points stay sorted with no two at one x.
				local low = if at > 1 then points[at - 1][1] + GAP else 0
				local high = if at <= #points then points[at][1] - GAP else 1
				if high < low then
					state.dragging = nil
					return state
				end
				table.insert(points, at, { math.clamp(x, low, high), y })
				nearest = at
			end
			state.dragging = nearest
		elseif msg.phase == "up" then
			state.dragging = nil
			return state, { { to = "parent", msg = message(props.change, points, "up") } }
		end
		local index = state.dragging
		if not index or not points[index] then
			return state
		end
		local low = if index > 1 then points[index - 1][1] + GAP else 0
		local high = if index < #points then points[index + 1][1] - GAP else 1
		-- Squeezed between close neighbours, it moves only up and down.
		local along = if high < low then points[index][1] else math.clamp(x, low, high)
		points[index] = { along, y }
		return state, { { to = "parent", msg = message(props.change, points, msg.phase) } }
	end,

	build = function(props: Props, kept: any)
		local state: State = kept or {}
		local size = props.size or 200
		local inner = size - 2 * INSET
		local points = props.points
		local slope = slopes(points)
		local samples = table.create(SAMPLES)
		for index = 1, SAMPLES do
			samples[index] = math.clamp(at(points, slope, (index - 1) / (SAMPLES - 1)), 0, 1)
		end
		local color = props.color or "accent"
		local layers = {}
		if props.values and #props.values > 1 then
			table.insert(layers, ui.row {
				float = "fill",
				ui.line {
					points = props.values,
					min = 0,
					area = true,
					stroke = STROKE,
					width = size,
					height = size,
					style = { color = "muted-foreground/30", background = "muted-foreground/15" },
				},
			})
		end
		table.insert(layers, ui.row {
			float = "fill",
			ui.line {
				points = { 0, 1 },
				min = 0,
				max = 1,
				dashed = true,
				stroke = 1,
				width = size,
				height = size,
				style = { color = "muted-foreground/35" },
			},
		})
		table.insert(layers, ui.row {
			float = "fill",
			ui.line {
				points = samples,
				min = 0,
				max = 1,
				stroke = STROKE,
				width = size,
				height = size,
				style = { color = color },
			},
		})
		-- The grid, then the points, in the inset square the curve uses.
		local marks = {}
		local lines = props.grid or 3
		for step = 1, lines do
			local fraction = step / (lines + 1)
			table.insert(marks, ui.row { class = "curve-editor-grid", area = { left = fraction, top = 0, height = 1 }, width = 1 })
			table.insert(marks, ui.row { class = "curve-editor-grid", area = { left = 0, top = fraction, width = 1 }, height = 1 })
		end
		for index, point in points do
			table.insert(marks, ui.row {
				area = { left = point[1], top = 1 - point[2] },
				width = 0,
				height = 0,
				ui.row {
					class = "curve-editor-handle",
					selected = state.dragging == index,
					style = props.color and { border_color = props.color } or nil,
					float = { left = -HANDLE / 2, top = -HANDLE / 2 },
					width = HANDLE,
					height = HANDLE,
				},
			})
		end
		local live = not props.disabled and props.change ~= nil
		table.insert(layers, ui.column {
			float = { top = INSET, left = INSET },
			width = inner,
			height = inner,
			pointer = if live then { curve = true } else nil,
			cursor = if live then "crosshair" else nil,
			table.unpack(marks),
		})
		return ui.row {
			class = "curve-editor",
			width = size,
			height = size,
			table.unpack(layers),
		}
	end,
}
