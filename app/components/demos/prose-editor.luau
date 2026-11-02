-- A writing surface: type, then switch focus mode and typewriter mode.
local prose_editor = use "prose-editor"
local segmented = use "segmented"
local switch = use "switch"
local writing_stats = use "writing-stats"

local TEXT = [[# A quiet page

The sentence being written stays in full ink. The rest of the page steps back, so the next word is all there is.

Markdown takes its shape as it is typed: *emphasis*, **strong**, `code` and [links](https://example.com), the markup faded.

- Lists keep their bullets quiet
> Quotes lean back a little.]]

return mod {
	init = function(previous)
		return previous or { dim = "sentence", typewriter = false, text = TEXT, show = "words" }
	end,

	update = function(state, msg)
		if type(msg) ~= "table" then
			if msg == "typewriter" then
				state.typewriter = not state.typewriter
			end
		elseif msg.select then
			state.dim = if msg.select == "off" then nil else msg.select
		elseif msg.msg == "typed" then
			state.text = msg.value
		elseif msg.show then
			state.show = msg.show
		end
	end,

	view = function(state)
		return ui.column {
			grow = true,
			gap = "normal",
			ui.row {
				align = "center",
				gap = "large",
				segmented {
					small = true,
					selected = state.dim or "off",
					options = {
						{ id = "off", label = "All" },
						{ id = "sentence", label = "Sentence" },
						{ id = "paragraph", label = "Paragraph" },
					},
				},
				switch { label = "Typewriter", on = state.typewriter, msg = "typewriter", small = true },
				ui.spacer(),
				writing_stats { text = state.text, show = state.show, msg = { show = true } },
			},
			ui.column {
				height = 320,
				prose_editor {
					id = "demo",
					value = TEXT,
					change = "typed",
					dim = state.dim,
					typewriter = state.typewriter,
					measure = 560,
				},
			},
		}
	end,
}
