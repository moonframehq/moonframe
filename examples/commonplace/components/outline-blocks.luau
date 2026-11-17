-- A markdown list drawn as an outline: one bullet per block, indented by
-- depth, folded blocks with a count, tasks with a check, and [[wikilinks]]
-- inline as links. Outliner mode over a plain list (decision 11 in
-- plans/commonplace.md): the file stays `- text` lines.

local function segments(text: string, link: any): { any }
	local parts = {}
	local rest = text
	while true do
		local s, e, title = string.find(rest, "%[%[([^%]]+)%]%]")
		if not s or not e or type(title) ~= "string" then
			break
		end
		if s > 1 then
			table.insert(parts, ui.text { string.sub(rest, 1, s - 1) })
		end
		table.insert(parts, ui.row { gap = "none", msg = { [link] = title }, ui.text { title, class = "outline-link" } })
		rest = string.sub(rest, e + 1)
	end
	if #rest > 0 then
		table.insert(parts, ui.text { rest })
	end
	return parts
end

return component {
	title = "Outline Blocks",
	category = "Commonplace",
	icon = "list-tree",
	description = "A list of blocks as an outline: bullets, depth, folds, tasks, wikilinks.",
	when = "A journal day or the inbox in Commonplace's outline mode. For a file tree use `tree`.",
	props = {
		blocks = "{ { depth, text, folded? (hidden children), task? (boolean: done) } }",
		link = "key of the message a wikilink click sends: { [link] = title } (default open)",
		selected = "index of the block with the caret, drawn raised",
	},
	styles = {
		[".outline"] = { gap = 2 },
		[".outline-block"] = { gap = 8, padding = { 3, 6 }, radius = 6 },
		[".outline-block:selected"] = { background = "accent/8" },
		[".outline-bullet"] = { width = 14, color = "muted-foreground" },
		[".outline-folded"] = { color = "accent" },
		[".outline-link"] = { color = "accent" },
		[".outline-count"] = { size = "small", color = "muted-foreground" },
		[".outline-guide"] = { width = 22 },
	},
	example = {
		blocks = {
			{ depth = 0, text = "read: [[Make It Stick]] ch. 3" },
			{ depth = 1, text = "retrieval beats re-reading" },
			{ depth = 0, text = "meeting notes", folded = 3 },
		},
	},
	build = function(props)
		local rows = {}
		local link = props.link or "open"
		for index, block: any in props.blocks or {} do
			local marker
			if block.task ~= nil then
				marker = ui.icon { if block.task then "square-check" else "square", class = "outline-bullet" }
			elseif block.folded then
				marker = ui.icon { "chevron-right", class = "outline-bullet outline-folded" }
			else
				marker = ui.text { "•", class = "outline-bullet" }
			end
			local line = segments(block.text, link)
			table.insert(
				rows,
				ui.row {
					class = "outline-block",
					selected = props.selected == index,
					align = "start",
					block.depth > 0 and ui.row { class = "outline-guide", style = { width = block.depth * 22 } },
					marker,
					ui.row { gap = "none", wrap = true, grow = true, table.unpack(line) },
					block.folded and ui.text { `{block.folded}`, class = "outline-count" },
				}
			)
		end
		return ui.column { class = "outline", table.unpack(rows) }
	end,
}
