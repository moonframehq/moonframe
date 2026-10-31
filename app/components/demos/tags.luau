-- A repository's topics: click "+ name" to add a tag or "− name" to
-- remove it, and pick a tone for the badges.
local tags = use "tags"
local segmented = use "segmented"

local SUGGESTIONS = { "rust", "gpui", "luau", "wasm", "terminal", "agents", "macos" }

return mod {
	init = function(previous)
		return previous or { tags = { "rust", "gpui", "luau" }, tone = "normal" }
	end,

	update = function(state, msg)
		if msg.select then
			state.tone = msg.select
		elseif msg.add then
			local index = table.find(state.tags, msg.add)
			if index then
				table.remove(state.tags, index)
			else
				table.insert(state.tags, msg.add)
			end
		end
	end,

	view = function(state)
		local buttons = {}
		for _, name in SUGGESTIONS do
			local has = table.find(state.tags, name) ~= nil
			table.insert(buttons, ui.button { if has then `− {name}` else `+ {name}`, small = true, variant = "ghost", msg = { add = name } })
		end
		return ui.column {
			gap = "large",
			segmented {
				small = true,
				selected = state.tone,
				options = {
					{ id = "normal", label = "Normal" },
					{ id = "accent", label = "Accent" },
					{ id = "success", label = "Success" },
					{ id = "warning", label = "Warning" },
				},
			},
			tags { tags = state.tags, tone = state.tone },
			ui.row { gap = "small", wrap = true, table.unpack(buttons) },
		}
	end,
}
