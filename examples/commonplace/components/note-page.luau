-- A note as a page: its title, its properties (the frontmatter) and its
-- markdown, with [[wikilinks]] drawn as links. Or, in `source` mode, the
-- file's text to edit, markdown styled as typed. Takes the `note` view the
-- notes provider sends.

local prose_editor = use "prose-editor"

local function wikify(text: string): string
	-- [[Title|alias]] and [[Title]] become markdown links to `note:Title`;
	-- the target goes in <…> so titles with spaces stay one link.
	local out = string.gsub(text, "%[%[([^%]|]+)|([^%]]+)%]%]", "[%2](<note:%1>)")
	out = string.gsub(out, "%[%[([^%]]+)%]%]", "[%1](<note:%1>)")
	return out
end

-- The text after the frontmatter.
local function body(text: string): string
	if string.sub(text, 1, 4) ~= "---\n" then
		return text
	end
	local close = string.find(text, "\n---\n", 4, true)
	if not close then
		return text
	end
	return string.sub(text, close + 5)
end

local function shown(value: any): string
	if type(value) == "table" then
		local parts = {}
		for _, item in value do
			table.insert(parts, tostring(item))
		end
		return table.concat(parts, ", ")
	end
	local text = tostring(value)
	-- An ISO time (`2025-12-27T10:00:00.000Z`) as a date and a time.
	local date, time = string.match(text, "^(%d%d%d%d%-%d%d%-%d%d)T(%d%d:%d%d)")
	if date and time then
		return `{date} {time}`
	end
	return text
end

return component {
	title = "Note Page",
	category = "Commonplace",
	icon = "file-text",
	description = "A note's title, properties and body, wikilinks as links; or its source to edit.",
	when = "The open note in Commonplace. For plain markdown use `markdown`; for a writing surface alone use `prose-editor`.",
	props = {
		note = "the notes provider's `note` view: { path, title, text, info = { props } }",
		mode = "\"page\" (default) shows it, \"source\" edits it",
		link = "message a wikilink click sends, with `url` = note:Title (default \"link\")",
		change = "message each edit sends in source mode, with `value` (default { edit = true })",
		width = "max text width in pixels (default 680)",
	},
	styles = {
		[".note-page"] = { padding = { 28, 40, 40, 40 }, gap = 14 },
		[".note-title"] = { size = "display", weight = "bold" },
		[".note-path"] = { size = "small", color = "muted-foreground" },
		[".note-props"] = { gap = 4, padding = { 10, 0, 12, 0 }, border = "bottom", border_color = "border" },
		[".note-prop"] = { gap = 12 },
		[".note-prop-key"] = { width = 76, size = "small", color = "muted-foreground" },
		[".note-prop-value"] = { size = "small" },
		[".note-source"] = { padding = { 20, 28 } },
	},
	example = {
		note = {
			path = "notes/Spaced repetition.md",
			title = "Spaced repetition",
			text = "---\ntype: concept\n---\nReview just before you forget, see [[Forgetting curve]].\n",
			info = { props = { type = "concept" } },
		},
	},
	build = function(props)
		local note: any = props.note or {}
		if props.mode == "source" then
			return ui.column {
				class = "note-source",
				grow = true,
				prose_editor {
					id = "note-source",
					value = note.text or "",
					change = props.change or { edit = true },
					measure = props.width or 680,
					size = "normal",
					placeholder = "Write…",
				},
			}
		end
		-- Properties, sorted by name, without the id.
		local keys = {}
		local info: any = note.info or {}
		for key in info.props or {} do
			-- Ids are for the app (`id`, an import's `apple-notes_id`), not for reading.
			if key ~= "id" and not string.match(tostring(key), "_id$") then
				table.insert(keys, tostring(key))
			end
		end
		table.sort(keys)
		local rows = {}
		for _, key in keys do
			table.insert(
				rows,
				ui.row {
					class = "note-prop",
					ui.text { key, class = "note-prop-key" },
					ui.text { shown(info.props[key]), class = "note-prop-value" },
				}
			)
		end
		return ui.column {
			class = "note-page",
			-- Inside a scrolling column: keep its full height, so it scrolls.
			shrink = false,
			max_width = props.width or 680,
			note.path and ui.text { note.path, class = "note-path" },
			ui.text { note.title or "Untitled", class = "note-title" },
			#rows > 0 and ui.column { class = "note-props", table.unpack(rows) },
			ui.markdown { wikify(body(note.text or "")), link = props.link or "link" },
		}
	end,
}
