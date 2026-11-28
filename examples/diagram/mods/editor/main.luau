-- The editor: owns the document (shapes and links), the selection, the tool
-- and undo, and draws the top bar, the layers, the canvas and the inspector.
-- The canvas (`use "diagram"`) only sends edits; each lands here as one
-- change to the document, with the document before it kept for undo. All of
-- it is mod state, so the document comes back at the next launch.

local diagram = use "diagram"
local button_group = use "button-group"
local icon_button = use "icon-button"
local inspector = use "diagram-inspector"
local live_diagrams = use "live-diagrams"

type Shape = { id: string, kind: string, x: number, y: number, w: number, h: number, label: string?, fill: string? }
type Link = { id: string, from: string, to: string, label: string?, dashed: boolean? }
type Doc = { shapes: { Shape }, links: { Link } }
type State = {
	title: string,
	doc: Doc,
	selected: { string },
	tool: string,
	next: number,
	version: number,
	zoom: number,
	-- The Luau canvas said it is there (a build without `ui.diagram`):
	-- zooming is a message to it, else a `camera` prop.
	luau: boolean,
	camera: { [string]: any }?,
	-- The agent's (or project's) diagram on show instead of the document,
	-- by its key in `app.diagrams`.
	live: string?,
	-- The last export asked of the diagram on show, each new `seq` once,
	-- until its answer comes back or another diagram is shown.
	export: { [string]: any }?,
	exports: number,
	-- The `seq` of every export not answered yet: each answer is taken
	-- once, and only while its diagram is on show.
	pending: { number },
	-- Why the last export failed, shown in the top bar until dismissed.
	export_error: string?,
	-- Zooming the diagram on show, as `camera` asks it.
	live_camera: { [string]: any }?,
	undo: { Doc },
	redo: { Doc },
}

-- How many earlier documents undo keeps.
local HISTORY = 50
local FILLS = { "#e2e8f0", "#bfdbfe", "#bbf7d0", "#fde68a", "#fecaca", "#ddd6fe" }
local DEFAULT_FILL: { [string]: string } = { sticky = "#fde68a", diamond = "#ddd6fe", pill = "#ddd6fe", ellipse = "#bbf7d0" }
local LABELS: { [string]: string } =
	{ box = "Box", diamond = "Decision?", pill = "Step", ellipse = "Ellipse", text = "Text", sticky = "Note" }

local TOOLS = {
	{ id = "select", icon = "mouse-pointer-2", tooltip = "Select (V)" },
	{ id = "hand", icon = "hand", tooltip = "Pan (H)" },
	{ id = "box", icon = "square", tooltip = "Box (R)" },
	{ id = "diamond", icon = "diamond", tooltip = "Decision (D)" },
	{ id = "ellipse", icon = "circle", tooltip = "Ellipse (O)" },
	{ id = "text", icon = "type", tooltip = "Text (T)" },
	{ id = "sticky", icon = "sticky-note", tooltip = "Sticky note (S)" },
	{ id = "connector", icon = "spline", tooltip = "Connector (C)" },
}

local function sample(): Doc
	return {
		shapes = {
			{ id = "cart", kind = "box", x = 40, y = 70, w = 140, h = 64, label = "Cart", fill = "#e2e8f0" },
			{ id = "auth", kind = "diamond", x = 250, y = 70, w = 150, h = 64, label = "Signed in?", fill = "#ddd6fe" },
			{ id = "pay", kind = "box", x = 470, y = 70, w = 140, h = 64, label = "Payment", fill = "#bfdbfe" },
			{ id = "login", kind = "box", x = 250, y = 220, w = 150, h = 64, label = "Log in", fill = "#e2e8f0" },
			{ id = "done", kind = "ellipse", x = 690, y = 70, w = 120, h = 64, label = "Receipt", fill = "#bbf7d0" },
			{ id = "note", kind = "sticky", x = 40, y = 210, w = 150, h = 100, label = "Guest checkout later?", fill = "#fde68a" },
		},
		links = {
			{ id = "l1", from = "cart", to = "auth" },
			{ id = "l2", from = "auth", to = "pay", label = "yes" },
			{ id = "l3", from = "auth", to = "login", label = "no" },
			{ id = "l4", from = "login", to = "pay", dashed = true },
			{ id = "l5", from = "pay", to = "done" },
		},
	}
end

-- A copy of the document, for undo: shapes and links are replaced, never
-- shared, so a later edit can't reach back into history.
local function copy(doc: Doc): Doc
	local shapes, links = {}, {}
	for _, s in doc.shapes do
		table.insert(shapes, table.clone(s))
	end
	for _, l in doc.links do
		table.insert(links, table.clone(l))
	end
	return { shapes = shapes, links = links }
end

local function shape_of(doc: Doc, id: string): Shape?
	for _, s in doc.shapes do
		if s.id == id then
			return s
		end
	end
	return nil
end

local function link_of(doc: Doc, id: string): Link?
	for _, l in doc.links do
		if l.id == id then
			return l
		end
	end
	return nil
end

local function new_id(state: State, prefix: string): string
	state.next += 1
	return `{prefix}{state.next}`
end

-- Keep the document as it is now for undo, before changing it.
local function remember(state: State)
	table.insert(state.undo, copy(state.doc))
	if #state.undo > HISTORY then
		table.remove(state.undo, 1)
	end
	state.redo = {}
	state.version += 1
end

local function remove(state: State, ids: { string })
	local gone = {}
	for _, id in ids do
		gone[id] = true
	end
	local shapes, links = {}, {}
	for _, s in state.doc.shapes do
		if not gone[s.id] then
			table.insert(shapes, s)
		end
	end
	for _, l in state.doc.links do
		if not (gone[l.id] or gone[l.from] or gone[l.to]) then
			table.insert(links, l)
		end
	end
	state.doc = { shapes = shapes, links = links }
	state.selected = {}
end

local function duplicate(state: State, ids: { string })
	local chosen, renamed = {}, {}
	for _, id in ids do
		chosen[id] = true
	end
	local picked = {}
	for _, s in table.clone(state.doc.shapes) do
		if chosen[s.id] then
			local twin = table.clone(s)
			twin.id = new_id(state, "s")
			twin.x += 24
			twin.y += 24
			renamed[s.id] = twin.id
			table.insert(state.doc.shapes, twin)
			table.insert(picked, twin.id)
		end
	end
	-- Links between two duplicated shapes come along.
	for _, l in table.clone(state.doc.links) do
		if renamed[l.from] and renamed[l.to] then
			local twin = table.clone(l)
			twin.id = new_id(state, "l")
			twin.from, twin.to = renamed[l.from], renamed[l.to]
			table.insert(state.doc.links, twin)
		end
	end
	state.selected = picked
end

-- Move the chosen shapes to the top of the stack, or the bottom.
local function restack(state: State, front: boolean)
	local chosen = {}
	for _, id in state.selected do
		chosen[id] = true
	end
	local rest, moved = {}, {}
	for _, s in state.doc.shapes do
		table.insert(if chosen[s.id] then moved else rest, s)
	end
	local shapes = if front then rest else moved
	for _, s in if front then moved else rest do
		table.insert(shapes, s)
	end
	state.doc.shapes = shapes
end

-- `count` boxes on a grid, each linked to the one before: for measuring.
local function stress(state: State, count: number)
	local x0 = 0
	for _, s in state.doc.shapes do
		x0 = math.max(x0, s.x + s.w)
	end
	x0 += 80
	local columns = math.max(1, math.floor(math.sqrt(count) * 1.5))
	local last: string? = nil
	for i = 0, count - 1 do
		local id = new_id(state, "s")
		table.insert(state.doc.shapes, {
			id = id,
			kind = "box",
			x = x0 + (i % columns) * 180,
			y = math.floor(i / columns) * 110,
			w = 140,
			h = 60,
			label = `Box {i + 1}`,
			fill = FILLS[(i % #FILLS) + 1],
		})
		if last and i % columns ~= 0 then
			table.insert(state.doc.links, { id = new_id(state, "l"), from = last, to = id })
		end
		last = id
	end
end

-- An edit the canvas sent, applied to the document. Returns whether it
-- changed the document, so the caller keeps undo only for real changes.
local function apply(state: State, msg: any): boolean
	local doc = state.doc
	if msg.moved then
		for _, m in msg.moved do
			local s = shape_of(doc, m.id)
			if s then
				s.x, s.y = m.x, m.y
			end
		end
	elseif msg.resized then
		local r = msg.resized
		local s = shape_of(doc, r.id)
		if s then
			s.x, s.y, s.w, s.h = r.x, r.y, r.w, r.h
		end
	elseif msg.created then
		local c = msg.created
		local id = new_id(state, "s")
		table.insert(doc.shapes, {
			id = id,
			kind = c.kind,
			x = c.x,
			y = c.y,
			w = c.w,
			h = c.h,
			label = LABELS[c.kind] or "",
			fill = DEFAULT_FILL[c.kind] or FILLS[1],
		})
		state.selected = { id }
		state.tool = "select"
	elseif msg.connected then
		local c = msg.connected
		for _, l in doc.links do
			if l.from == c.from and l.to == c.to then
				return false
			end
		end
		local id = new_id(state, "l")
		table.insert(doc.links, { id = id, from = c.from, to = c.to })
		state.selected = { id }
	elseif msg.edited then
		local s = shape_of(doc, msg.edited.id)
		if not s or s.label == msg.edited.label then
			return false
		end
		s.label = msg.edited.label
	elseif msg.deleted then
		remove(state, msg.deleted)
	elseif msg.duplicate then
		if #msg.duplicate == 0 then
			return false
		end
		duplicate(state, msg.duplicate)
	elseif msg.set then
		-- From the inspector: one field of the chosen shapes or links.
		local field, value = msg.set, msg.value
		local changed = false
		for _, id in state.selected do
			local item: any = shape_of(doc, id) or link_of(doc, id)
			if item then
				if field == "x" or field == "y" or field == "w" or field == "h" then
					value = tonumber(value)
					if value and (field == "w" or field == "h") then
						value = math.max(12, value)
					end
				end
				if value ~= nil and item[field] ~= value then
					item[field] = value
					changed = true
				end
			end
		end
		return changed
	else
		return false
	end
	return true
end

local function topbar(state: State, problem: string?): any
	return ui.row {
		class = "editor-topbar",
		drag = true,
		align = "center",
		ui.row { width = 64, shrink = false },
		button_group { small = true, selected = state.tool, event = "tool", buttons = TOOLS },
		ui.spacer(),
		ui.text { state.title, weight = "medium" },
		problem and ui.text { `Export failed: {problem}`, tone = "danger", size = "small", truncate = true },
		problem and icon_button { icon = "x", tooltip = "Dismiss", msg = { export_error = false }, size = "small" },
		ui.spacer(),
		icon_button { icon = "undo-2", tooltip = "Undo (⌘Z)", msg = "undo", size = "small", disabled = #state.undo == 0 },
		icon_button { icon = "redo-2", tooltip = "Redo (⇧⌘Z)", msg = "redo", size = "small", disabled = #state.redo == 0 },
		ui.row { width = 12 },
		icon_button { icon = "zoom-out", tooltip = "Zoom out (⌘-)", msg = { zoom_by = 0.8 }, size = "small" },
		ui.row {
			width = 48,
			justify = "center",
			msg = { zoom_to = 1 },
			tooltip = "Actual size (⌘0)",
			ui.text { `{math.floor(state.zoom * 100 + 0.5)}%`, size = "small" },
		},
		icon_button { icon = "zoom-in", tooltip = "Zoom in (⌘=)", msg = { zoom_by = 1.25 }, size = "small" },
		icon_button { icon = "maximize", tooltip = "Zoom to fit (⇧1)", msg = { fit = true }, size = "small" },
	}
end

local KIND_ICON: { [string]: string } = {
	box = "square",
	pill = "square",
	diamond = "diamond",
	ellipse = "circle",
	text = "type",
	sticky = "sticky-note",
}

-- Shapes topmost first, as a stack of layers reads.
local function layers(state: State, app: any): any
	local rows: { Shape } = {}
	for i = #state.doc.shapes, 1, -1 do
		table.insert(rows, state.doc.shapes[i])
	end
	local chosen = {}
	for _, id in state.selected do
		chosen[id] = true
	end
	return ui.column {
		class = "editor-side editor-layers",
		width = 220,
		shrink = false,
		ui.row {
			class = "editor-heading-row",
			ui.text { "LAYERS", class = "editor-heading" },
			ui.spacer(),
			ui.text { `{#state.doc.shapes}`, class = "editor-count" },
		},
		ui.list {
			id = "layers",
			count = #rows,
			row_height = 30,
			grow = true,
			row = function(i)
				local s = rows[i]
				return ui.row {
					class = "editor-layer",
					selected = chosen[s.id] == true,
					align = "center",
					msg = { pick = s.id },
					ui.icon { name = (KIND_ICON[s.kind] or "square") :: any, size = "small", tone = "muted" },
					ui.text { if s.label and s.label ~= "" then s.label else s.kind, truncate = true },
				}
			end,
		},
		ui.row {
			class = "editor-heading-row",
			ui.text { "CONNECTORS", class = "editor-heading" },
			ui.spacer(),
			ui.text { `{#state.doc.links}`, class = "editor-count" },
		},
		app.diagrams and #app.diagrams.diagrams > 0 and ui.row {
			class = "editor-heading-row",
			ui.text { "FROM AGENTS", class = "editor-heading" },
			ui.spacer(),
			ui.text { `{#app.diagrams.diagrams}`, class = "editor-count" },
		},
		app.diagrams and live_diagrams { diagrams = app.diagrams.diagrams, shown = state.live },
	}
end

-- The picked diagram of `app.diagrams`, if it is still there.
local function live(state: State, app: any): any
	if not state.live or not app.diagrams then
		return nil
	end
	for _, d in app.diagrams.diagrams do
		if d.key == state.live then
			return d
		end
	end
	return nil
end

-- An agent's diagram: what it says over it, then the diagram, read-only.
local function live_view(state: State, d: any, app: any): any
	local body = if d.error
		then ui.text { `Not drawn: {d.error}`, tone = "danger" }
		elseif not d.document then ui.text { "Too big to share: open its file.", tone = "muted" }
		elseif not app.capabilities.diagram then ui.text { "Drawing it needs the `diagram` feature.", tone = "muted" }
		else ui.diagram {
			id = "live",
			document = d.document,
			highlight = d.highlight,
			camera = state.live_camera or { fit = true, seq = d.key },
			export = state.export,
		}
	return ui.column {
		grow = true,
		gap = "none",
		ui.row {
			class = "editor-live-bar",
			align = "center",
			ui.icon { name = (if d.from == "agent" then "sparkles" else "file-text") :: any, size = "small", tone = "accent" },
			ui.text { d.title, weight = "medium" },
			d.agent and ui.text { `from {d.agent}`, tone = "muted", size = "small" },
			d.note and ui.text { d.note, tone = "accent", size = "small", truncate = true },
			ui.spacer(),
			d.from == "agent" and ui.button { "Close", small = true, variant = "ghost", msg = { close_live = d.id } },
			ui.button { "Back to my diagram", small = true, variant = "ghost", msg = { live = false } },
		},
		ui.column { grow = true, gap = "none", body },
	}
end

return mod {
	styles = {
		[".editor-topbar"] = { padding = { 6, 10 }, gap = 6, background = "sidebar", border = "bottom", border_color = "border" },
		[".editor-side"] = { background = "sidebar", padding = { 10, 8 }, gap = 4, border_color = "border" },
		[".editor-layers"] = { border = "right" },
		[".editor-heading-row"] = { padding = { 6, 8, 4, 8 } },
		[".editor-heading"] = { size = "small", weight = "semibold", color = "muted-foreground" },
		[".editor-count"] = { size = "small", color = "muted-foreground" },
		[".editor-layer"] = { padding = { 4, 8 }, gap = 8, radius = 6, margin = { 0, 2 } },
		[".editor-layer:hover"] = { background = "list-hover" },
		[".editor-layer:selected"] = { background = "accent/15", color = "accent" },
		[".editor-live-bar"] = { padding = { 6, 12 }, gap = 8, background = "accent/8", border = "bottom", border_color = "border" },
	},

	init = function(previous: State?): State
		local state: State = if type(previous) == "table" then previous else {} :: any
		if type(state.doc) ~= "table" or type(state.doc.shapes) ~= "table" then
			state.doc = sample()
		end
		state.title = if type(state.title) == "string" then state.title else "Checkout flow"
		state.selected = if type(state.selected) == "table" then state.selected else {}
		state.tool = "select"
		state.next = if type(state.next) == "number" then state.next else 100
		state.version = if type(state.version) == "number" then state.version else 1
		state.zoom = if type(state.zoom) == "number" then state.zoom else 1
		state.luau = false
		state.camera = nil
		state.live = nil
		state.export = nil
		state.exports = 0
		state.pending = {}
		state.export_error = nil
		state.live_camera = nil
		-- History lives for the session only.
		state.undo, state.redo = {}, {}
		return state
	end,

	update = function(state: State, msg: any)
		-- While an agent's diagram is on show, zooming moves it, and what it
		-- tells of itself and an export of it stay its own; anything else is
		-- for the user's diagram, which comes back first so no edit lands
		-- unseen.
		local live_msg = type(msg) == "table"
			and (msg.live ~= nil or msg.close_live or msg.export or msg.exported or msg.export_error ~= nil)
		if state.live and not live_msg then
			if type(msg) == "table" and (msg.zoom_by or msg.zoom_to or msg.fit) then
				local seq = if state.live_camera then state.live_camera.seq + 1 else 1
				state.live_camera = { zoom_by = msg.zoom_by, zoom = msg.zoom_to, fit = msg.fit, seq = seq }
				return
			elseif type(msg) == "table" and (msg.zoom or msg.selected) then
				state.zoom = msg.zoom or state.zoom
				return
			end
			state.live = nil
		end
		if msg == "undo" or msg == "redo" then
			local from, to = if msg == "undo" then state.undo else state.redo, if msg == "undo" then state.redo else state.undo
			local doc = table.remove(from)
			if doc then
				table.insert(to, copy(state.doc))
				state.doc = doc
				state.selected = {}
				state.version += 1
			end
		elseif msg == "delete" then
			if #state.selected > 0 then
				remember(state)
				remove(state, state.selected)
			end
		elseif msg == "duplicate" then
			if #state.selected > 0 then
				remember(state)
				duplicate(state, state.selected)
			end
		elseif msg == "select-all" then
			local all = {}
			for _, s in state.doc.shapes do
				table.insert(all, s.id)
			end
			state.selected = all
		elseif msg == "front" or msg == "back" then
			if #state.selected > 0 then
				remember(state)
				restack(state, msg == "front")
			end
		elseif msg == "sample" or msg == "clear" then
			remember(state)
			state.doc = if msg == "sample" then sample() else { shapes = {}, links = {} }
			state.selected = {}
		elseif type(msg) ~= "table" then
			return
		elseif msg.tool then
			state.tool = msg.tool
		elseif msg.zoom then
			state.zoom = msg.zoom
			state.luau = state.luau or msg.canvas == "luau"
		elseif msg.zoom_by or msg.zoom_to or msg.fit then
			if state.luau then
				return state, { { to = "diagram#main", msg = msg } }
			end
			local seq = if state.camera then (state.camera.seq or 0) + 1 else 1
			state.camera = { zoom_by = msg.zoom_by, zoom = msg.zoom_to, fit = msg.fit, seq = seq }
		elseif msg.selected then
			state.selected = msg.selected
		elseif msg.export then
			-- Asked of the diagram on show: the editor's or a live one.
			state.exports += 1
			state.export = { format = msg.export, seq = state.exports }
			table.insert(state.pending, state.exports)
			state.export_error = nil
		elseif msg.exported then
			-- Asked of a diagram no longer on show: let it go.
			local at = table.find(state.pending, msg.exported.seq)
			if not at then
				return
			end
			table.remove(state.pending, at)
			-- The last one answered: no view asks again when it is drawn anew.
			if state.export and state.export.seq == msg.exported.seq then
				state.export = nil
			end
			if msg.exported.error then
				state.export_error = msg.exported.error
			elseif msg.exported.path then
				-- The file is in the app's share folder: offer it to the system.
				return state, { { to = "app:share", msg = { files = { msg.exported.path } } } }
			end
		elseif msg.export_error == false then
			state.export_error = nil
			state.export = nil
			state.pending = {}
		elseif msg.live ~= nil then
			state.live = msg.live or nil
			state.export = nil
			state.pending = {}
			state.live_camera = nil
		elseif msg.close_live then
			state.live = nil
			state.export = nil
			state.pending = {}
			return state, { { to = "provider:diagrams", msg = { close = msg.close_live } } }
		elseif msg.pick then
			state.selected = { msg.pick }
		elseif msg.stress then
			remember(state)
			stress(state, msg.stress)
			if state.luau then
				return state, { { to = "diagram#main", msg = { fit = true } } }
			end
			state.camera = { fit = true, seq = (if state.camera then state.camera.seq or 0 else 0) + 1 }
		else
			local before = copy(state.doc)
			if apply(state, msg) then
				table.insert(state.undo, before)
				if #state.undo > HISTORY then
					table.remove(state.undo, 1)
				end
				state.redo = {}
				state.version += 1
			end
		end
	end,

	view = function(state: State, app)
		-- Only the app's own view exports; a live diagram it cannot draw
		-- (or a build without it) never answers, so say so here.
		local shown = live(state, app)
		local drawn = app.capabilities.diagram and (not shown or (shown.document and not shown.error))
		local problem = state.export_error or (state.export and not drawn and "this diagram is not drawn by the app") or nil
		return ui.column {
			grow = true,
			gap = "none",
			topbar(state, problem),
			ui.row {
				grow = true,
				gap = "none",
				align = "stretch",
				layers(state, app),
				ui.column {
					grow = true,
					gap = "none",
					-- An agent's diagram, read-only, while one is picked.
					if shown
						then live_view(state, shown, app)
					-- The Rust view where the build has it, else the Luau canvas:
					-- the same props, the same messages back.
					elseif app.capabilities.diagram
						then ui.diagram {
							id = "main",
							shapes = state.doc.shapes,
							links = state.doc.links,
							selected = state.selected,
							tool = state.tool,
							camera = state.camera,
							export = state.export,
						}
						else diagram {
							id = "main",
							shapes = state.doc.shapes,
							links = state.doc.links,
							selected = state.selected,
							tool = state.tool,
							version = state.version,
						},
				},
				-- The inspector edits the user's diagram: not beside another.
				not shown and inspector { doc = state.doc, selected = state.selected, fills = FILLS },
			},
		}
	end,
}
