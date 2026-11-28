-- A small editable diagram: the mod keeps the shapes, links and selection,
-- and applies each edit the canvas sends. Drag shapes, drag a corner to
-- resize, drag from a blue port to connect, double-click to rename, scroll
-- to pan and pinch (or ⌘-scroll) to zoom.
local diagram = use "diagram"

local function find(list: { any }, id: string): (any, number?)
	for i, item in list do
		if item.id == id then
			return item, i
		end
	end
	return nil, nil
end

return mod {
	init = function(previous: any): any
		return previous
			or {
				next = 10,
				selected = {},
				shapes = {
					{ id = "a", kind = "box", x = 0, y = 40, w = 130, h = 56, label = "Cart", fill = "#e2e8f0" },
					{ id = "b", kind = "pill", x = 200, y = 40, w = 140, h = 56, label = "Signed in?", fill = "#ddd6fe" },
					{ id = "c", kind = "ellipse", x = 200, y = 170, w = 140, h = 60, label = "Receipt", fill = "#bbf7d0" },
				},
				links = { { id = "l1", from = "a", to = "b" }, { id = "l2", from = "b", to = "c", label = "yes" } },
			}
	end,

	update = function(state: any, msg: any)
		if type(msg) ~= "table" then
			return
		end
		if msg.selected then
			state.selected = msg.selected
		elseif msg.moved then
			for _, m in msg.moved do
				local s = find(state.shapes, m.id)
				if s then
					s.x, s.y = m.x, m.y
				end
			end
		elseif msg.resized then
			local s = find(state.shapes, msg.resized.id)
			if s then
				s.x, s.y, s.w, s.h = msg.resized.x, msg.resized.y, msg.resized.w, msg.resized.h
			end
		elseif msg.created then
			state.next += 1
			local shape = msg.created
			shape.id = `s{state.next}`
			shape.label = "New"
			shape.fill = "#e2e8f0"
			table.insert(state.shapes, shape)
			state.selected = { shape.id }
		elseif msg.connected then
			state.next += 1
			table.insert(state.links, { id = `l{state.next}`, from = msg.connected.from, to = msg.connected.to })
		elseif msg.edited then
			local s = find(state.shapes, msg.edited.id)
			if s then
				s.label = msg.edited.label
			end
		elseif msg.deleted then
			local gone = {}
			for _, id in msg.deleted do
				gone[id] = true
			end
			local shapes, links = {}, {}
			for _, s in state.shapes do
				if not gone[s.id] then
					table.insert(shapes, s)
				end
			end
			for _, l in state.links do
				if not (gone[l.id] or gone[l.from] or gone[l.to]) then
					table.insert(links, l)
				end
			end
			state.shapes, state.links, state.selected = shapes, links, {}
		end
	end,

	view = function(state: any)
		return ui.column {
			height = 320,
			diagram { id = "demo", shapes = state.shapes, links = state.links, selected = state.selected },
		}
	end,
}
