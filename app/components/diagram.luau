-- A freeform diagram canvas: shapes placed in world pixels, connectors that
-- follow them, selection (click, shift-click, marquee), drag, resize, new
-- shapes and connectors, pan and zoom.
--
-- The document is the parent's: it passes `shapes`, `links` and `selected`
-- and applies what this sends it (`moved`, `resized`, `created`,
-- `connected`, `selected`, `deleted`, `edited`). The instance keeps only how
-- the canvas is viewed (pan and zoom) and the gesture under way, so a drag
-- draws from the parent's shapes plus the pointer's travel, and one message
-- on release changes the document. The Rust `ui.diagram` keeps these props
-- and messages.

type Shape = {
	id: string,
	kind: string,
	x: number,
	y: number,
	w: number,
	h: number,
	label: string?,
	fill: string?,
}
type Link = { id: string, from: string, to: string, label: string?, dashed: boolean? }
type Props = {
	id: string,
	shapes: { Shape },
	links: { Link }?,
	selected: { string }?,
	tool: string?,
	version: number?,
}
type View = { x: number, y: number, zoom: number }
type Drag = {
	kind: string,
	ids: { string }?,
	id: string?,
	corner: string?,
	sx: number,
	sy: number,
	dx: number,
	dy: number,
	add: boolean?,
	vx: number?,
	vy: number?,
}
type State = { view: View, width: number, height: number, drag: Drag?, editing: string?, zoom_told: number? }
-- A shape's box on screen, in pixels from the canvas's top-left corner.
type Box = { x: number, y: number, w: number, h: number }

local MIN_ZOOM, MAX_ZOOM = 0.1, 4
-- The smallest a shape may be resized to, in world pixels.
local MIN_SIZE = 12
-- Pixels a press must travel before it counts as a drag, not a click.
local SLOP = 3
-- The size a click (rather than a drag) gives each kind of new shape.
local SIZES: { [string]: { number } } = {
	box = { 140, 64 },
	pill = { 150, 64 },
	-- A rotated square is beyond Luau: a decision draws as a pill here and
	-- as a diamond in the Rust view.
	diamond = { 150, 72 },
	ellipse = { 120, 72 },
	text = { 120, 32 },
	sticky = { 150, 120 },
}
-- Tools that place a new shape, by the kind they make.
local MAKES: { [string]: string } =
	{ box = "box", pill = "pill", diamond = "diamond", ellipse = "ellipse", text = "text", sticky = "sticky" }

local KEYS = {
	backspace = "delete",
	delete = "delete",
	escape = "escape",
	["secondary-a"] = "select-all",
	["secondary-d"] = "duplicate",
	up = { nudge = { 0, -1 } },
	down = { nudge = { 0, 1 } },
	left = { nudge = { -1, 0 } },
	right = { nudge = { 1, 0 } },
	["shift-up"] = { nudge = { 0, -10 } },
	["shift-down"] = { nudge = { 0, 10 } },
	["shift-left"] = { nudge = { -10, 0 } },
	["shift-right"] = { nudge = { 10, 0 } },
	["secondary-0"] = { zoom_to = 1 },
	["secondary-="] = { zoom_by = 1.25 },
	["secondary--"] = { zoom_by = 0.8 },
}
-- Single-letter tool keys (V, H, R...) type text, which a row's keys never
-- take: the app puts them in a menu, which may (see examples/diagram).

-- The resize handles: which corner, where on the box, and the pointer over it.
local CORNERS: { { name: string, fx: number, fy: number, cursor: string } } = {
	{ name = "nw", fx = 0, fy = 0, cursor = "nwse-resize" },
	{ name = "ne", fx = 1, fy = 0, cursor = "nesw-resize" },
	{ name = "sw", fx = 0, fy = 1, cursor = "nesw-resize" },
	{ name = "se", fx = 1, fy = 1, cursor = "nwse-resize" },
}

local function to_parent(msg: any): { any }
	return { { to = "parent", msg = msg } }
end

local function set_of(list: { string }?): { [string]: boolean }
	local out = {}
	for _, id in list or {} do
		out[id] = true
	end
	return out
end

local function round(n: number): number
	return math.floor(n + 0.5)
end

-- The shape whose box holds a world point, topmost first.
local function hit(shapes: { Shape }, x: number, y: number): Shape?
	for i = #shapes, 1, -1 do
		local s = shapes[i]
		if x >= s.x and x <= s.x + s.w and y >= s.y and y <= s.y + s.h then
			return s
		end
	end
	return nil
end

-- A shape's box in world pixels while `drag` is under way: moved by the
-- pointer's travel, or resized from the corner held.
local function live_rect(s: Shape, drag: Drag?, moving: { [string]: boolean }, zoom: number): (number, number, number, number)
	local x, y, w, h = s.x, s.y, s.w, s.h
	if not drag then
		return x, y, w, h
	end
	local ddx, ddy = drag.dx / zoom, drag.dy / zoom
	if drag.kind == "move" and moving[s.id] then
		return x + ddx, y + ddy, w, h
	end
	if drag.kind == "resize" and drag.id == s.id then
		local corner = drag.corner or "se"
		local west, north = string.find(corner, "w") ~= nil, string.find(corner, "n") ~= nil
		local right, bottom = x + w, y + h
		if west then
			x = math.min(x + ddx, right - MIN_SIZE)
			w = right - x
		else
			w = math.max(MIN_SIZE, w + ddx)
		end
		if north then
			y = math.min(y + ddy, bottom - MIN_SIZE)
			h = bottom - y
		else
			h = math.max(MIN_SIZE, h + ddy)
		end
	end
	return x, y, w, h
end

-- An elbow between two boxes on screen: out of the side facing the other
-- box, across, and into the facing side of the other (or, when the boxes
-- are apart both ways, a single bend into its top or bottom). Returns the flat
-- points and where a label sits (the middle of the elbow).
local function route(a: Box, b: Box): ({ number }, number, number)
	local acx, acy = a.x + a.w / 2, a.y + a.h / 2
	local bcx, bcy = b.x + b.w / 2, b.y + b.h / 2
	local gap_x = math.max(b.x - (a.x + a.w), a.x - (b.x + b.w))
	local gap_y = math.max(b.y - (a.y + a.h), a.y - (b.y + b.h))
	-- Apart on both axes: one bend, out of the side and into the top or
	-- bottom, which keeps clear of the straight links beside it.
	if gap_x > 0 and gap_y > 0 then
		local sx = if bcx >= acx then a.x + a.w else a.x
		local ey = if bcy >= acy then b.y else b.y + b.h
		return { sx, acy, bcx, acy, bcx, ey }, (sx + bcx) / 2, acy
	end
	if gap_x >= gap_y then
		local sx = if bcx >= acx then a.x + a.w else a.x
		local ex = if bcx >= acx then b.x else b.x + b.w
		local mx = (sx + ex) / 2
		if math.abs(acy - bcy) < 1 then
			return { sx, acy, ex, acy }, mx, acy
		end
		return { sx, acy, mx, acy, mx, bcy, ex, bcy }, mx, (acy + bcy) / 2
	end
	local sy = if bcy >= acy then a.y + a.h else a.y
	local ey = if bcy >= acy then b.y else b.y + b.h
	local my = (sy + ey) / 2
	if math.abs(acx - bcx) < 1 then
		return { acx, sy, acx, ey }, acx, my
	end
	return { acx, sy, acx, my, bcx, my, bcx, ey }, (acx + bcx) / 2, my
end

local function clamp_zoom(z: number): number
	return math.clamp(z, MIN_ZOOM, MAX_ZOOM)
end

-- Zoom to `zoom` keeping the world point under screen point (px, py) still.
local function zoom_at(view: View, zoom: number, px: number, py: number)
	zoom = clamp_zoom(zoom)
	local wx, wy = view.x + px / view.zoom, view.y + py / view.zoom
	view.zoom = zoom
	view.x = wx - px / zoom
	view.y = wy - py / zoom
end

-- Frame every shape in the canvas, with a margin.
local function fit(state: State, shapes: { Shape })
	if #shapes == 0 or state.width <= 0 then
		state.view = { x = 0, y = 0, zoom = 1 }
		return
	end
	local x0: number, y0: number, x1: number, y1: number = math.huge, math.huge, -math.huge, -math.huge
	for _, s in shapes do
		x0 = math.min(x0, s.x)
		y0 = math.min(y0, s.y)
		x1 = math.max(x1, s.x + s.w)
		y1 = math.max(y1, s.y + s.h)
	end
	local margin = 48
	local zoom = clamp_zoom(math.min((state.width - margin * 2) / math.max(1, x1 - x0), (state.height - margin * 2) / math.max(1, y1 - y0), 1))
	state.view = {
		zoom = zoom,
		x = (x0 + x1) / 2 - state.width / 2 / zoom,
		y = (y0 + y1) / 2 - state.height / 2 / zoom,
	}
end

-- Tell the parent the zoom when it changed, for its readout.
local function told_zoom(state: State): { any }?
	local z = round(state.view.zoom * 100) / 100
	if state.zoom_told ~= z then
		state.zoom_told = z
		return to_parent { zoom = z, canvas = "luau" }
	end
	return nil
end

-- What a pointer message on the background does when it goes down.
local function canvas_down(state: State, msg: any, props: Props): { any }?
	local px: number, py: number = msg.x * msg.width, msg.y * msg.height
	local tool = props.tool or "select"
	if tool == "hand" or msg.button == "middle" then
		state.drag = { kind = "pan", sx = px, sy = py, dx = 0, dy = 0, vx = state.view.x, vy = state.view.y }
	elseif MAKES[tool] then
		state.drag = { kind = "create", id = MAKES[tool], sx = px, sy = py, dx = 0, dy = 0 }
	else
		state.drag = { kind = "marquee", sx = px, sy = py, dx = 0, dy = 0, add = msg.shift == true }
		if not msg.shift and #(props.selected or {}) > 0 then
			return to_parent { selected = {} }
		end
	end
	return nil
end

-- What letting go ends: one message to the parent for the whole gesture.
local function finish(state: State, props: Props): { any }?
	local drag = state.drag
	state.drag = nil
	if not drag then
		return nil
	end
	local v = state.view
	local travelled = math.abs(drag.dx) + math.abs(drag.dy) > SLOP
	if drag.kind == "move" and travelled then
		local moved = {}
		local ids = set_of(drag.ids)
		for _, s in props.shapes do
			if ids[s.id] then
				table.insert(moved, { id = s.id, x = round(s.x + drag.dx / v.zoom), y = round(s.y + drag.dy / v.zoom) })
			end
		end
		return to_parent { moved = moved }
	elseif drag.kind == "resize" and travelled then
		for _, s in props.shapes do
			if s.id == drag.id then
				local x, y, w, h = live_rect(s, drag, {}, v.zoom)
				return to_parent { resized = { id = s.id, x = round(x), y = round(y), w = round(w), h = round(h) } }
			end
		end
	elseif drag.kind == "marquee" then
		local x0, x1 = math.min(drag.sx, drag.sx + drag.dx), math.max(drag.sx, drag.sx + drag.dx)
		local y0, y1 = math.min(drag.sy, drag.sy + drag.dy), math.max(drag.sy, drag.sy + drag.dy)
		if not travelled then
			return nil
		end
		local wx0, wy0 = v.x + x0 / v.zoom, v.y + y0 / v.zoom
		local wx1, wy1 = v.x + x1 / v.zoom, v.y + y1 / v.zoom
		local picked = if drag.add then table.clone(props.selected or {}) else {}
		local already = set_of(picked)
		for _, s in props.shapes do
			if not already[s.id] and s.x < wx1 and s.x + s.w > wx0 and s.y < wy1 and s.y + s.h > wy0 then
				table.insert(picked, s.id)
			end
		end
		return to_parent { selected = picked }
	elseif drag.kind == "create" then
		local kind = drag.id or "box"
		local size = SIZES[kind] or SIZES.box
		local x: number, y: number, w: number, h: number = 0, 0, 0, 0
		if travelled then
			x = v.x + math.min(drag.sx, drag.sx + drag.dx) / v.zoom
			y = v.y + math.min(drag.sy, drag.sy + drag.dy) / v.zoom
			w = math.max(MIN_SIZE, math.abs(drag.dx) / v.zoom)
			h = math.max(MIN_SIZE, math.abs(drag.dy) / v.zoom)
		else
			w = size[1] :: number
			h = size[2] :: number
			x = v.x + drag.sx / v.zoom - w / 2
			y = v.y + drag.sy / v.zoom - h / 2
		end
		return to_parent { created = { kind = kind, x = round(x), y = round(y), w = round(w), h = round(h) } }
	elseif drag.kind == "link" then
		local wx, wy = v.x + (drag.sx + drag.dx) / v.zoom, v.y + (drag.sy + drag.dy) / v.zoom
		local target = hit(props.shapes, wx, wy)
		if target and target.id ~= drag.id then
			return to_parent { connected = { from = drag.id, to = target.id } }
		end
	end
	return nil
end

-- A press on a shape: select it (shift toggles), start moving the selection,
-- or with the connector tool start a link from it.
local function shape_down(state: State, msg: any, props: Props): { any }?
	local id = msg.shape
	local tool = props.tool or "select"
	if tool == "hand" or msg.button == "middle" then
		state.drag = { kind = "pan", sx = 0, sy = 0, dx = 0, dy = 0, vx = state.view.x, vy = state.view.y }
		return nil
	end
	if msg.clicks >= 2 then
		state.editing = id
		state.drag = nil
		return nil
	end
	if tool == "connector" then
		local v = state.view
		for _, s in props.shapes do
			if s.id == id then
				local sx, sy = (s.x - v.x) * v.zoom, (s.y - v.y) * v.zoom
				state.drag = { kind = "link", id = id, sx = sx + msg.x * msg.width, sy = sy + msg.y * msg.height, dx = 0, dy = 0 }
			end
		end
		return nil
	end
	local selected = props.selected or {}
	local chosen = set_of(selected)
	local sends = nil
	local moving: { string }
	if msg.shift then
		moving = {}
		local toggled = {}
		for _, s in selected do
			if s ~= id then
				table.insert(toggled, s)
			end
		end
		if not chosen[id] then
			table.insert(toggled, id)
		end
		sends = to_parent { selected = toggled }
		moving = toggled
	elseif chosen[id] then
		moving = selected
	else
		moving = { id }
		sends = to_parent { selected = moving }
	end
	state.drag = { kind = "move", ids = moving, sx = 0, sy = 0, dx = 0, dy = 0 }
	return sends
end

local function wheel(state: State, msg: any): { any }?
	local px, py = msg.x * msg.width, msg.y * msg.height
	local v = state.view
	if msg.zoom and msg.zoom ~= 0 then
		zoom_at(v, v.zoom * (1 + msg.zoom), px, py)
	elseif msg.secondary or msg.ctrl then
		zoom_at(v, v.zoom * math.exp(msg.dy * 0.004), px, py)
	else
		v.x -= msg.dx / v.zoom
		v.y -= msg.dy / v.zoom
	end
	return told_zoom(state)
end

-- The background: a box on screen per shape, links under them, then what
-- the selection and the gesture under way add on top.
local function draw(props: Props, state: State): { any }
	local W, H = state.width, state.height
	local out: { any } = {}
	if W <= 0 or H <= 0 then
		return out
	end
	local v, drag = state.view, state.drag
	local z = v.zoom
	local chosen = set_of(props.selected)
	local moving = if drag and drag.kind == "move" then set_of(drag.ids) else {}
	local tool = props.tool or "select"
	local boxes: { [string]: Box } = {}
	for _, s in props.shapes do
		local x, y, w, h = live_rect(s, drag, moving, z)
		boxes[s.id] = { x = (x - v.x) * z, y = (y - v.y) * z, w = w * z, h = h * z }
	end
	local function area(b: Box): any
		return { left = b.x / W, top = b.y / H, width = b.w / W, height = b.h / H }
	end
	local function shows(b: Box): boolean
		return b.x + b.w >= 0 and b.y + b.h >= 0 and b.x <= W and b.y <= H
	end

	-- Links, under the shapes.
	local paths: { any } = {}
	local marks: { any } = {}
	for _, l in props.links or {} do
		local a, b = boxes[l.from], boxes[l.to]
		if a and b and (shows(a) or shows(b)) then
			local points, mx, my = route(a, b)
			local on = chosen[l.id]
			table.insert(paths, {
				points = points,
				arrow = "end",
				radius = 10,
				stroke = if on then 2 else 1.5,
				dashed = l.dashed,
				color = if on then "accent" else "muted-foreground",
			})
			if l.label and l.label ~= "" and z >= 0.4 then
				table.insert(
					marks,
					ui.row {
						area = area { x = mx - 40, y = my - 10, w = 80, h = 20 },
						justify = "center",
						align = "center",
						ui.row {
							class = "diagram-link-label",
							selected = on,
							pointer = { link = l.id },
							ui.text { l.label, truncate = true },
						},
					}
				)
			else
				table.insert(marks, ui.row { area = area { x = mx - 6, y = my - 6, w = 12, h = 12 }, class = "diagram-link-pad", selected = on, pointer = { link = l.id } })
			end
		end
	end
	if #paths > 0 then
		table.insert(out, ui.column { float = "fill", ui.paths { paths = paths } })
	end

	-- Shapes.
	local labels = z >= 0.45
	for _, s in props.shapes do
		local b = boxes[s.id]
		if shows(b) then
			local kind = if SIZES[s.kind] then s.kind else "box"
			local sticky = kind == "sticky"
			table.insert(
				out,
				ui.column {
					area = area(b),
					class = { "diagram-shape", `diagram-{kind}` },
					style = if s.fill and kind ~= "text" then { background = s.fill } else nil,
					pointer = { shape = s.id },
					cursor = if tool == "select" then "move" elseif tool == "hand" then "grab" else "crosshair",
					align = if sticky then "start" else "center",
					justify = if sticky then "start" else "center",
					labels and s.label and s.label ~= "" and state.editing ~= s.id and ui.text {
						s.label,
						class = if kind == "text" then "diagram-text-label" else "diagram-label",
						wrap = true,
					},
				}
			)
		end
	end
	for _, mark in marks do
		table.insert(out, mark)
	end

	-- The selection: an outline round each chosen shape; handles and
	-- connector ports when one alone is chosen and nothing is dragged.
	local alone: string? = nil
	local count = 0
	for id in chosen do
		local b = boxes[id]
		if b then
			table.insert(out, ui.row { area = area { x = b.x - 3, y = b.y - 3, w = b.w + 6, h = b.h + 6 }, class = "diagram-selection" })
			alone, count = id, count + 1
		end
	end
	if alone and count == 1 and (not drag or drag.kind == "resize") and tool == "select" then
		local b = boxes[alone]
		for _, c in CORNERS do
			local hx, hy = b.x - 3 + c.fx * (b.w + 6) - 4, b.y - 3 + c.fy * (b.h + 6) - 4
			table.insert(out, ui.row { area = area { x = hx, y = hy, w = 8, h = 8 }, class = "diagram-handle", cursor = c.cursor :: any, pointer = { handle = alone, corner = c.name } })
		end
		if not drag then
			for _, p in { { 0.5, 0 }, { 1, 0.5 }, { 0.5, 1 }, { 0, 0.5 } } do
				local px, py = b.x + p[1] * b.w, b.y + p[2] * b.h
				-- Beyond the edge, so a press on the shape near it still moves it.
				px += (p[1] - 0.5) * 24
				py += (p[2] - 0.5) * 24
				table.insert(out, ui.row { area = area { x = px - 5, y = py - 5, w = 10, h = 10 }, class = "diagram-port", cursor = "crosshair", pointer = { port = alone, px = px, py = py } })
			end
		end
	end

	-- The gesture under way.
	if drag and drag.kind == "marquee" then
		local x, y = math.min(drag.sx, drag.sx + drag.dx), math.min(drag.sy, drag.sy + drag.dy)
		table.insert(out, ui.row { area = area { x = x, y = y, w = math.abs(drag.dx), h = math.abs(drag.dy) }, class = "diagram-marquee" })
	elseif drag and drag.kind == "create" then
		local x, y = math.min(drag.sx, drag.sx + drag.dx), math.min(drag.sy, drag.sy + drag.dy)
		table.insert(out, ui.row { area = area { x = x, y = y, w = math.abs(drag.dx), h = math.abs(drag.dy) }, class = "diagram-marquee" })
	elseif drag and drag.kind == "link" then
		local ex, ey = drag.sx + drag.dx, drag.sy + drag.dy
		table.insert(out, ui.column {
			float = "fill",
			ui.paths { paths = { { points = { drag.sx, drag.sy, ex, ey }, arrow = "end", dashed = true, stroke = 2, color = "accent" } } },
		})
	end

	-- A label being edited, in place.
	local editing = state.editing and boxes[state.editing]
	if editing then
		local label = ""
		for _, s in props.shapes do
			if s.id == state.editing then
				label = s.label or ""
			end
		end
		table.insert(out, ui.column {
			area = area { x = editing.x, y = editing.y + editing.h / 2 - 16, w = math.max(editing.w, 120), h = 32 },
			ui.input {
				id = `diagram-edit-{props.id}`,
				value = label,
				focus = true,
				submit = { edited = state.editing },
				blur = { edited = state.editing },
				keys = { escape = "cancel-edit" },
			},
		})
	end
	return out
end

return component {
	title = "Diagram",
	category = "Data",
	icon = "workflow",
	description = "A freeform canvas of boxes, text and sticky notes wired by connectors: select, drag, resize, connect, pan and zoom; the document stays the parent's.",
	when = "An editable diagram the user draws: flows, architecture sketches, mind maps. The parent keeps `shapes`, `links` and `selected` and applies the edits it is sent. For a fixed system diagram on a grid use `flow-diagram`; for dependency arrows between dated bars use `gantt`.",
	props = {
		["id"] = "the instance's id (its pan and zoom are kept under it)",
		["shapes"] = "list of { id, kind (box pill diamond ellipse text sticky), x, y, w, h (world pixels), label?, fill? (a color) }, bottom first",
		["links"] = "list of { id, from, to (shape ids), label?, dashed? }: drawn as elbows that follow the shapes",
		["selected"] = "ids of the chosen shapes and links",
		["tool"] = "select (default), hand, box, pill, ellipse, text, sticky or connector",
		["version"] = "a number the parent bumps on each change to the document",
	},
	styles = {
		[".diagram"] = { background = "background" },
		[".diagram-canvas"] = { background = "background", clip = true },
		[".diagram-shape"] = { background = "card", border = true, border_color = "foreground/25", radius = 8, padding = 6 },
		[".diagram-box"] = { radius = 8 },
		[".diagram-pill"] = { radius = 999 },
		[".diagram-diamond"] = { radius = 999 },
		[".diagram-ellipse"] = { radius = 999 },
		[".diagram-text"] = { background = "#00000000", border = "none" },
		[".diagram-sticky"] = { radius = 2, border = "none", padding = 10, shadow = "0 6px 14px -6px #00000040" },
		[".diagram-label"] = { color = "#1e293b", size = "small", weight = "medium" },
		[".diagram-text-label"] = { color = "foreground", weight = "medium" },
		[".diagram-selection"] = { border = true, border_color = "accent", radius = 4 },
		[".diagram-handle"] = { background = "background", border = true, border_color = "accent", radius = 2 },
		[".diagram-port"] = { background = "accent", radius = 999 },
		[".diagram-port:hover"] = { background = "accent/60" },
		[".diagram-marquee"] = { background = "accent/8", border = true, border_color = "accent/60" },
		[".diagram-link-label"] = { background = "background", radius = 4, padding = { 0, 4 }, size = "small", color = "muted-foreground" },
		[".diagram-link-label:selected"] = { color = "accent" },
		[".diagram-link-pad"] = { radius = 999 },
		[".diagram-link-pad:hover"] = { background = "accent/30" },
		[".diagram-link-pad:selected"] = { background = "accent" },
	},
	example = {
		id = "example",
		shapes = {
			{ id = "a", kind = "box", x = 20, y = 40, w = 130, h = 56, label = "Cart", fill = "#e2e8f0" },
			{ id = "b", kind = "pill", x = 210, y = 40, w = 140, h = 56, label = "Signed in?", fill = "#ddd6fe" },
			{ id = "c", kind = "ellipse", x = 210, y = 160, w = 140, h = 60, label = "Receipt", fill = "#bbf7d0" },
		},
		links = { { id = "l1", from = "a", to = "b" }, { id = "l2", from = "b", to = "c", label = "yes" } },
		selected = { "b" },
	},

	init = function(previous: State?, props: Props): State
		local state: State = if type(previous) == "table" then previous else {} :: any
		if type(state.view) ~= "table" then
			state.view = { x = -40, y = -40, zoom = 1 }
		end
		state.width = if type(state.width) == "number" then state.width else 0
		state.height = if type(state.height) == "number" then state.height else 0
		state.drag = nil
		state.editing = nil
		state.zoom_told = nil
		return state
	end,

	update = function(state: State, msg: any, props: Props)
		if type(msg) ~= "table" then
			if msg == "delete" then
				if #(props.selected or {}) > 0 then
					return state, to_parent { deleted = props.selected }
				end
			elseif msg == "escape" then
				state.drag = nil
				return state, to_parent { selected = {} }
			elseif msg == "select-all" then
				local all = {}
				for _, s in props.shapes do
					table.insert(all, s.id)
				end
				return state, to_parent { selected = all }
			elseif msg == "duplicate" then
				return state, to_parent { duplicate = props.selected or {} }
			elseif msg == "cancel-edit" then
				state.editing = nil
			end
			return state
		end
		if msg.measured then
			state.width, state.height = msg.width, msg.height
			return state, told_zoom(state)
		elseif msg.wheel then
			return state, wheel(state, msg)
		elseif msg.tool then
			return state, to_parent { tool = msg.tool }
		elseif msg.nudge then
			local moved = {}
			local chosen = set_of(props.selected)
			for _, s in props.shapes do
				if chosen[s.id] then
					table.insert(moved, { id = s.id, x = s.x + msg.nudge[1], y = s.y + msg.nudge[2] })
				end
			end
			return state, if #moved > 0 then to_parent { moved = moved } else nil
		elseif msg.zoom_by then
			zoom_at(state.view, state.view.zoom * msg.zoom_by, state.width / 2, state.height / 2)
			return state, told_zoom(state)
		elseif msg.zoom_to then
			zoom_at(state.view, msg.zoom_to, state.width / 2, state.height / 2)
			return state, told_zoom(state)
		elseif msg.fit then
			fit(state, props.shapes)
			return state, told_zoom(state)
		elseif msg.edited then
			local id = msg.edited
			state.editing = nil
			return state, to_parent { edited = { id = id, label = msg.value or "" } }
		end

		local phase = msg.phase
		if phase == "move" then
			local drag = state.drag
			if drag then
				drag.dx, drag.dy = msg.dx, msg.dy
				if drag.kind == "pan" then
					state.view.x = (drag.vx or 0) - msg.dx / state.view.zoom
					state.view.y = (drag.vy or 0) - msg.dy / state.view.zoom
				end
			end
			return state
		elseif phase == "up" then
			return state, finish(state, props)
		elseif phase ~= "down" then
			return state
		end

		if msg.canvas then
			state.editing = nil
			return state, canvas_down(state, msg, props)
		elseif msg.shape then
			return state, shape_down(state, msg, props)
		elseif msg.handle then
			local drag: Drag = { kind = "resize", id = msg.handle :: string, corner = msg.corner :: string, sx = 0, sy = 0, dx = 0, dy = 0 }
			state.drag = drag
		elseif msg.port then
			local drag: Drag = { kind = "link", id = msg.port :: string, sx = msg.px :: number, sy = msg.py :: number, dx = 0, dy = 0 }
			state.drag = drag
		elseif msg.link then
			local selected = props.selected or {}
			if msg.shift then
				local out = {}
				for _, id in selected do
					if id ~= msg.link then
						table.insert(out, id)
					end
				end
				if #out == #selected then
					table.insert(out, msg.link)
				end
				return state, to_parent { selected = out }
			end
			return state, to_parent { selected = { msg.link } }
		end
		return state
	end,

	build = function(props: Props, state: State)
		local tool = props.tool or "select"
		local panning = state.drag and state.drag.kind == "pan"
		return ui.column {
			class = "diagram",
			grow = true,
			gap = "none",
			scroll = true,
			measure = { measured = true },
			ui.column {
				class = "diagram-canvas",
				grow = true,
				gap = "none",
				pointer = { canvas = true },
				wheel = { wheel = true },
				keys = KEYS,
				cursor = if panning then "grabbing" elseif tool == "hand" then "grab" elseif tool == "select" then nil else "crosshair",
				table.unpack(draw(props, state)),
			},
		}
	end,
}
