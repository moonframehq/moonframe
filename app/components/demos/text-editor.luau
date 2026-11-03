-- A task's description: select some words and press Bold, or put the caret
-- on a line and press a list; press the same button again to take it off.
-- Preview shows it rendered. The description is saved as the field loses
-- focus.
local text_editor = use "text-editor"

return mod {
	init = function(previous)
		return previous
			or {
				text = "Rebuild the marketing site with a clearer information architecture.\n\nGoals\nShip the new navigation\nMove the blog to the CMS",
				preview = false,
				saved = 0,
			}
	end,

	update = function(state, msg)
		if typeof(msg) ~= "table" then
			return
		end
		if msg.msg == "type" then
			state.text = msg.value
		elseif msg.msg == "save" then
			state.saved += 1
		elseif msg.view then
			state.preview = msg.view == "preview"
		end
	end,

	view = function(state)
		return ui.column {
			gap = "normal",
			width = 560,
			text_editor {
				id = "description",
				value = state.text,
				change = "type",
				blur = "save",
				preview = state.preview,
				switch = "view",
				placeholder = "Add a description…",
			},
			ui.text {
				if state.saved > 0 then `Saved {state.saved} times` else "Saved when the field loses focus",
				size = "small",
				tone = "muted",
			},
		}
	end,
}
