-- Workspace avatars: click one to cycle its status dot; switch the shape
-- and size. The same name always gets the same colour.
local avatar = use "avatar"
local segmented = use "segmented"

local CYCLE = { none = "working", working = "blocked", blocked = "done", done = "idle", idle = "none" }

return mod {
	init = function(previous)
		return previous
			or {
				size = "xlarge",
				shape = "square",
				spaces = {
					{ name = "harbor-docs", status = "working" },
					{ name = "moonframe", status = "blocked" },
					{ name = "lumen", status = "none" },
					{ name = "ledger", status = "done" },
					{ name = "penso", status = "idle" },
				},
			}
	end,

	update = function(state, msg)
		if type(msg) ~= "table" then
			return
		end
		if msg.size then
			state.size = msg.size
		elseif msg.shape then
			state.shape = msg.shape
		elseif msg.cycle then
			local space = state.spaces[msg.cycle]
			space.status = CYCLE[space.status]
		end
	end,

	view = function(state)
		local tiles = {}
		for index, space in state.spaces do
			tiles[index] = ui.column {
				gap = "small",
				align = "center",
				msg = { cycle = index },
				avatar {
					name = space.name,
					size = state.size,
					shape = state.shape,
					status = if space.status == "none" then nil else space.status,
				},
				ui.text { space.name, size = "small", tone = "muted" },
			}
		end
		return ui.column {
			gap = "large",
			ui.row {
				gap = "normal",
				segmented {
					selected = state.shape,
					small = true,
					event = "shape",
					options = { { id = "square", label = "Square" }, { id = "circle", label = "Circle" } },
				},
				segmented {
					selected = state.size,
					small = true,
					event = "size",
					options = {
						{ id = "small", label = "S" },
						{ id = "normal", label = "M" },
						{ id = "large", label = "L" },
						{ id = "xlarge", label = "XL" },
					},
				},
			},
			ui.row { gap = "large", align = "end", table.unpack(tiles) },
		}
	end,
}
