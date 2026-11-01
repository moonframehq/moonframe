-- A scratch note: type in the source and the preview follows; the switch at
-- the top right shows the source, the preview, or both. Without a `path`
-- nothing is read or saved, and a clicked link only shows here.
local markdown_editor = use "markdown-editor"

local NOTE = [[
# Scratch note

Type on the left; the **preview** follows on the right.

- [x] Source and preview
- [ ] Save (a file needs a `path`)

See the [Luau guide](https://luau.org).
]]

return mod {
	init = function(previous)
		return previous or { clicked = nil }
	end,

	update = function(state, msg)
		if type(msg) == "table" and msg.open_link then
			state.clicked = msg.open_link
		end
	end,

	view = function(state)
		return ui.column {
			gap = "normal",
			height = 420,
			ui.text {
				if state.clicked then `Clicked: {state.clicked}` else "Click a link in the preview",
				tone = "muted",
				size = "small",
			},
			ui.column { grow = true, gap = "none", markdown_editor { id = "demo", text = NOTE } },
		}
	end,
}
