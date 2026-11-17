-- A note as a page you edit where you read it: its title, its properties
-- (the frontmatter) and its markdown in blocks (paragraphs, lists, code),
-- [[wikilinks]] drawn as links. Clicking a block turns that block alone into
-- its markdown to edit, styled as typed; Escape or a click elsewhere puts
-- the page back. Each edit sends the note's whole text to the drawing mod.
-- Links with a cached favicon (the notes provider's `favicons`) get it.

-- Where the body starts: after the frontmatter, if any.
local function body_start(text: string): number
	if string.sub(text, 1, 4) ~= "---\n" then
		return 1
	end
	local close = string.find(text, "\n---\n", 4, true)
	return if close then close + 5 else 1
end

type Part = { from: number, to: number }

-- The body's blocks, as byte ranges of `text`: runs of lines between blank
-- lines, a heading on its own, a fenced code block whole.
local function split(text: string): { Part }
	local parts: { Part } = {}
	local pos = body_start(text)
	local from: number? = nil
	local last = 0
	local fenced = false
	local function close()
		if from then
			table.insert(parts, { from = from, to = last })
			from = nil
		end
	end
	while pos <= #text do
		local nl = string.find(text, "\n", pos, true)
		local stop = if nl then nl - 1 else #text
		local line = string.sub(text, pos, stop)
		local fence = string.match(line, "^%s*```") ~= nil or string.match(line, "^%s*~~~") ~= nil
		if fenced then
			last = stop
			fenced = not fence
		elseif fence then
			from = from or pos
			last = stop
			fenced = true
		elseif string.match(line, "^%s*$") then
			close()
		elseif string.match(line, "^#+%s") then
			close()
			table.insert(parts, { from = pos, to = stop })
		else
			from = from or pos
			last = stop
		end
		pos = stop + 2
	end
	close()
	return parts
end

-- `text` ready for a block after its last: ending in a blank line.
local function ending(text: string): string
	if string.sub(text, body_start(text)) == "" or string.match(text, "\n\n$") then
		return text
	elseif string.sub(text, -1) == "\n" then
		return text .. "\n"
	end
	return text .. "\n\n"
end

type State = {
	-- The note being edited and the block's place: the text before and after
	-- it, kept as they were when editing began, so the note's text coming
	-- back from a save (split differently) never moves the edit.
	path: string?,
	editing: number?,
	before: string,
	after: string,
	draft: string,
	-- A link was just clicked: the click on the block around it is not an
	-- edit.
	linked: boolean,
}

local function wikify(text: string): string
	-- [[Title|alias]] and [[Title]] become markdown links to `note:Title`;
	-- the target goes in <…> so titles with spaces stay one link.
	local out = string.gsub(text, "%[%[([^%]|]+)|([^%]]+)%]%]", "[%2](<note:%1>)")
	out = string.gsub(out, "%[%[([^%]]+)%]%]", "[%1](<note:%1>)")
	return out
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

-- The host a line's only link goes to, when the line is one link (a list
-- item or a paragraph): `- [Title](https://host/…)`, maybe text after.
local function link_host(line: string): string?
	local url = string.match(line, "^%s*[-*+]?%s*%[[^%]]*%]%(<?(https?://[^%s>)]+)")
		or string.match(line, "^%s*[-*+]?%s*<?(https?://[^%s>]+)")
	if not url then
		return nil
	end
	local host = string.match(url, "^https?://([^/:?#]+)")
	return host and string.lower(host)
end

-- A block whose every line is a link, each line a row with the link's
-- favicon; nil when a line isn't one or no favicon is cached.
local function linked_lines(text: string, favicons: { [string]: string }, link: any): any
	local rows = {}
	local any_icon = false
	for line in string.gmatch(text, "[^\n]+") do
		local host = link_host(line)
		if not host then
			return nil
		end
		local icon = favicons[host] or favicons[(string.gsub(host, "^www%.", ""))]
		any_icon = any_icon or icon ~= nil
		table.insert(
			rows,
			ui.row {
				class = "note-link-row",
				align = "center",
				if icon
					then ui.image { src = icon, width = 16, height = 16, class = "note-favicon" }
					else ui.icon { name = "globe", class = "note-favicon-none" },
				ui.markdown { wikify((string.gsub(line, "^%s*[-*+]%s+", ""))), link = link, grow = true },
			}
		)
	end
	if not any_icon then
		return nil
	end
	return ui.column { class = "note-links", table.unpack(rows) }
end

local function header(note: any, into: { any })
	-- Properties, sorted by name, without the ids.
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
	if note.path then
		table.insert(into, ui.text { note.path, class = "note-path" })
	end
	table.insert(into, ui.text { note.title or "Untitled", class = "note-title" })
	if #rows > 0 then
		table.insert(into, ui.column { class = "note-props", table.unpack(rows) })
	end
end

local function field(state: State)
	return ui.input {
		id = "note-block",
		class = "note-block-field",
		value = state.draft,
		multiline = true,
		prose = true,
		rows = 1,
		max_rows = 400,
		focus = true,
		placeholder = "Write…",
		change = { draft = true },
		blur = { done = true },
		keys = { escape = { done = true } },
	}
end

-- The block `index`, as the page shows it: clickable to edit.
local function block_view(index: number, text: string, favicons: { [string]: string })
	local body = linked_lines(text, favicons, "link")
		or ui.markdown { wikify(text), link = "link", images = true }
	return ui.column { class = "note-block", msg = { block = index }, body }
end

return component {
	title = "Note Page",
	category = "Commonplace",
	icon = "file-text",
	description = "A note's title, properties and body, edited in place a block at a time; wikilinks as links, favicons beside web links.",
	when = "The open note in Commonplace. For plain markdown use `markdown`; for a writing surface alone use `prose-editor`.",
	props = {
		id = "keeps the editing state; one per place the page shows",
		note = "the notes provider's `note` view: { path, title, text, info = { props }, favicons }",
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
		[".note-body"] = { gap = 2 },
		[".note-block"] = { radius = 6, padding = { 4, 8 } },
		[".note-block:hover"] = { background = "list-hover" },
		[".note-block-field"] = { background = "list-hover", border = "none", radius = 6, padding = { 4, 8 } },
		[".note-block-field:focus"] = { border = "none" },
		[".note-links"] = { gap = 4 },
		[".note-link-row"] = { gap = 8 },
		[".note-favicon"] = { radius = 3 },
		[".note-favicon-none"] = { color = "muted-foreground", size = "small" },
		[".note-more"] = { height = 120 },
	},
	example = {
		id = "example",
		note = {
			path = "notes/Spaced repetition.md",
			title = "Spaced repetition",
			text = "---\ntype: concept\n---\nReview just before you forget, see [[Forgetting curve]].\n\n- one\n- two\n",
			info = { props = { type = "concept" } },
		},
	},

	init = function(previous: any): State
		local state = previous or {}
		-- An edit doesn't outlive the app: the text is saved as it is typed.
		return { editing = nil, before = "", after = "", draft = "", linked = false, path = state.path }
	end,

	update = function(state: State, msg: any, props: any)
		if type(msg) ~= "table" then
			return
		end
		local note: any = props.note or {}
		local text: string = note.text or ""
		if msg.msg == "link" then
			-- The block around the link hears the same click next.
			state.linked = true
			return state, { { to = "parent", msg = msg } }
		elseif msg.block then
			if state.linked then
				state.linked = false
				return
			end
			local parts = split(text)
			local part = parts[msg.block]
			state.path = note.path
			if part then
				state.editing = msg.block
				state.before = string.sub(text, 1, part.from - 1)
				state.after = string.sub(text, part.to + 1)
				state.draft = string.sub(text, part.from, part.to)
			else
				-- After the last block: a new one.
				state.editing = #parts + 1
				state.before = ending(text)
				state.after = ""
				state.draft = ""
			end
		elseif msg.draft and state.editing then
			state.draft = msg.value or ""
			local whole = state.before .. state.draft .. state.after
			if state.draft == "" and state.after == "" then
				-- A new block left empty changes nothing.
				whole = text
			end
			if whole ~= text then
				return state, { { to = "parent", msg = { edit = true, value = whole } } }
			end
		elseif msg.done then
			state.editing = nil
			state.linked = false
		end
	end,

	build = function(props: any, state: any)
		local note: any = props.note or {}
		local text: string = note.text or ""
		local favicons: { [string]: string } = note.favicons or {}
		local editing = state.editing ~= nil and state.path == note.path
		local body = {}
		if editing then
			-- The blocks around the edit as they were when it began.
			for index, part in split(state.before) do
				table.insert(body, block_view(index, string.sub(state.before, part.from, part.to), favicons))
			end
			table.insert(body, field(state))
			-- `after` has no frontmatter of its own: split it as a body.
			local rest = "---\n---\n" .. state.after
			for _, part in split(rest) do
				table.insert(body, ui.markdown { wikify(string.sub(rest, part.from, part.to)), link = "link", images = true })
			end
		else
			for index, part in split(text) do
				table.insert(body, block_view(index, string.sub(text, part.from, part.to), favicons))
			end
		end
		local page: { any } = {}
		header(note, page)
		table.insert(page, ui.column { class = "note-body", table.unpack(body) })
		if not editing then
			-- Below the last block: a click starts a new one.
			table.insert(page, ui.column { class = "note-more", msg = { block = 0 } })
		end
		return ui.column {
			class = "note-page",
			-- Inside a scrolling column: keep its full height, so it scrolls.
			shrink = false,
			max_width = props.width or 680,
			table.unpack(page),
		}
	end,
}
