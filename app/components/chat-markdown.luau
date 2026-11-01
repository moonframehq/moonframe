-- Markdown as rich text: paragraphs with **bold**, _italic_, `code` and
-- [links](url) inside the line, headings, bulleted and numbered lists,
-- tables, fenced code blocks, quotes and rules. A link whose target starts
-- with `cite:` is drawn as a small source chip, as chat answers cite their
-- sources. Text in a paragraph wraps word by word, so bold and code sit in
-- the flow of the line. It reads half-written text too: with `cursor` on,
-- a `**` or `_` not closed yet in the last paragraph already styles what
-- follows it, so a streamed answer reads right as it grows; otherwise an
-- unclosed marker is shown as it is.

type Props = {
	text: string?,
	cursor: boolean?,
	to: string?,
}

-- One word or chip in a line, and whether a space follows it.
type Atom = {
	text: string,
	bold: boolean,
	italic: boolean,
	code: boolean,
	link: string?,
	space: boolean,
}

-- Splits a line's text into styled runs, then into words.
local function atoms(text: string, growing: boolean?): { Atom }
	local found: { Atom } = {}
	local bold, italic = false, false
	local buffer = ""

	local function push(piece: string, code: boolean, link: string?)
		if piece == "" then
			return
		end
		-- Space before the run belongs to the word before it.
		if string.match(piece, "^%s") and #found > 0 then
			found[#found].space = true
		end
		if code or (link and string.sub(link, 1, 5) == "cite:") then
			local atom: Atom = { text = piece, bold = bold, italic = italic, code = code, link = link, space = false }
			table.insert(found, atom)
			return
		end
		for word, gap in string.gmatch(piece, "(%S+)(%s*)") do
			table.insert(found, { text = word, bold = bold, italic = italic, code = false, link = link, space = gap ~= "" })
		end
	end

	local function flush()
		push(buffer, false, nil)
		buffer = ""
	end

	local i, length = 1, #text
	while i <= length do
		local c = string.sub(text, i, i)
		local pair = string.sub(text, i, i + 1)
		if c == "`" then
			local close = string.find(text, "`", i + 1, true)
			if close then
				flush()
				push(string.sub(text, i + 1, close - 1), true, nil)
				-- A space after the code span.
				if string.match(string.sub(text, close + 1, close + 1), "%s") and #found > 0 then
					found[#found].space = true
				end
				i = close + 1
				continue
			end
		elseif pair == "**" or pair == "__" then
			if bold or growing or string.find(text, pair, i + 2, true) then
				flush()
				bold = not bold
				i += 2
				continue
			end
		elseif c == "*" or (c == "_" and (i == 1 or string.match(string.sub(text, i - 1, i - 1), "[%s%p]") ~= nil)) then
			if italic or growing or string.find(text, c, i + 1, true) then
				flush()
				italic = not italic
				i += 1
				continue
			end
		elseif c == "[" then
			local label, target = string.match(text, "^%[([^%]]+)%]%(([^%)]*)%)", i)
			if label and target then
				flush()
				local whole = #label + #target + 4
				push(label, false, target)
				if string.match(string.sub(text, i + whole, i + whole), "%s") and #found > 0 then
					found[#found].space = true
				end
				i += whole
				continue
			end
		end
		buffer ..= c
		i += 1
	end
	flush()
	return found
end

-- One line of rich text that wraps word by word.
local function line(text: string, extra: string?, cursor: boolean?, to: string?)
	local words = {}
	for _, atom in atoms(text, cursor) do
		if atom.link and string.sub(atom.link, 1, 5) == "cite:" then
			table.insert(words, ui.row {
				class = "chat-markdown-cite",
				align = "center",
				tooltip = string.sub(atom.link, 6),
				ui.row {
					class = "chat-markdown-cite-mark",
					width = 14,
					height = 14,
					align = "center",
					justify = "center",
					ui.text { string.upper(string.sub(atom.text, 1, 1)), class = "chat-markdown-cite-letter" },
				},
				ui.text { atom.text, class = "chat-markdown-cite-text", wrap = false },
			})
			if atom.space then
				table.insert(words, ui.row { class = "chat-markdown-gap" })
			end
			continue
		end
		-- A chip keeps the space after it outside its background.
		local gap_after = atom.space and atom.code
		local classes = "chat-markdown-word"
		if extra then
			classes ..= " " .. extra
		end
		if atom.code then
			classes ..= " chat-markdown-code"
		end
		if atom.bold then
			classes ..= " chat-markdown-bold"
		end
		if atom.italic then
			classes ..= " chat-markdown-italic"
		end
		if atom.link then
			classes ..= " chat-markdown-link"
		end
		if atom.space and not gap_after then
			classes ..= " chat-markdown-space"
		end
		table.insert(words, ui.text { atom.text, class = classes, wrap = false, tooltip = atom.link })
		if gap_after then
			table.insert(words, ui.row { class = "chat-markdown-gap" })
		end
	end
	if cursor then
		table.insert(words, ui.text { "●", class = "chat-markdown-word chat-markdown-cursor", wrap = false })
	end
	return ui.row { class = "chat-markdown-line", wrap = true, gap = "none", align = "center", table.unpack(words) }
end

local function is_rule(text: string): boolean
	return string.match(text, "^%s*%-%-%-+%s*$") ~= nil
		or string.match(text, "^%s*%*%*%*+%s*$") ~= nil
		or string.match(text, "^%s*___+%s*$") ~= nil
end

local function cells(row: string): { string }
	local inner = string.gsub(string.gsub(row, "^%s*|", ""), "|%s*$", "")
	local found = {}
	for cell in string.gmatch(inner .. "|", "([^|]*)|") do
		table.insert(found, (string.gsub(string.gsub(cell, "^%s+", ""), "%s+$", "")))
	end
	return found
end

-- Where a block starts, so a paragraph stops before it.
local function starts_block(text: string): boolean
	return string.match(text, "^%s*$") ~= nil
		or string.match(text, "^```") ~= nil
		or string.match(text, "^#+%s") ~= nil
		or string.match(text, "^%s*[-*+]%s") ~= nil
		or string.match(text, "^%s*%d+[.)]%s") ~= nil
		or string.match(text, "^%s*|") ~= nil
		or string.match(text, "^>") ~= nil
		or is_rule(text)
end

local function build(props: Props)
	local text = props.text or ""
	local lines = {}
	for each in string.gmatch(text .. "\n", "([^\n]*)\n") do
		table.insert(lines, each)
	end
	local blocks = {}
	-- The last paragraph-like block's text, for the streaming cursor.
	local i = 1
	local count = #lines
	while i <= count do
		local current = lines[i]
		if string.match(current, "^%s*$") then
			i += 1
		elseif string.match(current, "^```") then
			local language = string.match(current, "^```%s*(%S*)") or ""
			local code = {}
			i += 1
			while i <= count and not string.match(lines[i], "^```") do
				table.insert(code, lines[i])
				i += 1
			end
			i += 1
			local rows = {}
			for _, code_line in code do
				table.insert(rows, ui.text { if code_line == "" then " " else code_line, class = "chat-markdown-pre-line", wrap = false })
			end
			table.insert(blocks, ui.column {
				class = "chat-markdown-pre",
				ui.row {
					class = "chat-markdown-pre-head",
					align = "center",
					ui.text { if language == "" then "code" else language, class = "chat-markdown-pre-language" },
					ui.spacer(),
					ui.row {
						class = "chat-markdown-pre-copy",
						align = "center",
						copy = table.concat(code, "\n"),
						tooltip = "Copy code",
						ui.icon { name = "copy", size = "small" },
						ui.text { "Copy", size = "small" },
					},
				},
				ui.column { class = "chat-markdown-pre-body", scroll = "x", ui.column { shrink = false, gap = "none", table.unpack(rows) } },
			})
		elseif string.match(current, "^#+%s") then
			local hashes, title = string.match(current, "^(#+)%s+(.*)$")
			local level = math.min(#(hashes or "#"), 3)
			table.insert(blocks, line(title or "", `chat-markdown-h{level}`, nil, props.to))
			i += 1
		elseif string.match(current, "^%s*[-*+]%s") or string.match(current, "^%s*%d+[.)]%s") then
			local items = {}
			while i <= count and (string.match(lines[i], "^%s*[-*+]%s") or string.match(lines[i], "^%s*%d+[.)]%s")) do
				local indent, marker, body = string.match(lines[i], "^(%s*)([-*+])%s+(.*)$")
				if not marker then
					indent, marker, body = string.match(lines[i], "^(%s*)(%d+)[.)]%s+(.*)$")
					marker = (marker or "1") .. "."
				else
					marker = "•"
				end
				table.insert(items, ui.row {
					class = "chat-markdown-item",
					style = { padding = { left = math.floor(#(indent or "") / 2) * 18 } },
					ui.row { width = 22, shrink = false, ui.text { marker or "•", class = "chat-markdown-marker", wrap = false } },
					ui.column { grow = true, gap = "none", line(body or "", nil, nil, props.to) },
				})
				i += 1
			end
			table.insert(blocks, ui.column { class = "chat-markdown-list", table.unpack(items) })
		elseif string.match(current, "^%s*|") then
			local rows = {}
			while i <= count and string.match(lines[i], "^%s*|") do
				if not string.match(lines[i], "^[%s|:%-]+$") then
					table.insert(rows, cells(lines[i]))
				end
				i += 1
			end
			-- Every column but the last as wide as its longest text; the last
			-- takes the rest.
			local columns = 0
			local longest = {}
			for _, row in rows do
				columns = math.max(columns, #row)
				for index, cell in row do
					longest[index] = math.max(longest[index] or 0, utf8.len(cell) or #cell)
				end
			end
			local drawn = {}
			for index, row in rows do
				local items = {}
				for column = 1, columns do
					local width = if column < columns then math.min(320, math.max(80, (longest[column] or 0) * 9 + 40)) else nil
					table.insert(items, ui.column {
						class = "chat-markdown-cell",
						width = width,
						grow = column == columns,
						shrink = column == columns,
						line(row[column] or "", if index == 1 then "chat-markdown-th" else nil, nil, props.to),
					})
				end
				table.insert(drawn, ui.row { class = if index == 1 then "chat-markdown-tr chat-markdown-thead" else "chat-markdown-tr", table.unpack(items) })
			end
			table.insert(blocks, ui.column { class = "chat-markdown-table", table.unpack(drawn) })
		elseif string.match(current, "^>") then
			local quoted = {}
			while i <= count and string.match(lines[i], "^>") do
				table.insert(quoted, (string.gsub(lines[i], "^>%s?", "")))
				i += 1
			end
			table.insert(blocks, ui.column { class = "chat-markdown-quote", line(table.concat(quoted, " "), nil, nil, props.to) })
		elseif is_rule(current) then
			table.insert(blocks, ui.row { class = "chat-markdown-rule" })
			i += 1
		else
			local paragraph = { current }
			i += 1
			while i <= count and not starts_block(lines[i]) do
				table.insert(paragraph, lines[i])
				i += 1
			end
			local last = i > count
			table.insert(blocks, line(table.concat(paragraph, " "), nil, props.cursor and last, props.to))
		end
	end
	if props.cursor and (#blocks == 0 or not string.match(text, "[^\n]$")) then
		table.insert(blocks, line("", nil, true, props.to))
	end
	return ui.column { class = "chat-markdown", table.unpack(blocks) }
end

local SAMPLE = [[A reply with **bold**, _italic_ and `code` in the line, a [link](https://example.com) and a source chip [Loomwell +1](cite:loomwell).

| Term | Emphasis |
|---|---|
| Clay Software | Software that keeps taking the shape of your needs |
| Live Software | Changes land at once, with no redeploy |

- **Clay Software** — the most memorable.
- **Self-Rewriting Software** — the most literal.

```lua
local markdown = use "chat-markdown"
```]]

return component {
	title = "Chat Markdown",
	category = "Content",
	icon = "file-text",
	description = "Markdown drawn as rich text: bold, italic, code and links in the line, headings, lists, tables, code blocks, quotes, and source chips.",
	when = "A chat answer in markdown, including one still streaming in (pass `cursor` while it grows) or citing its sources with `[label](cite:source)` chips. For a whole document, a note or a README use `markdown` (drawn by the host, with links the mod handles); for a file to edit use `markdown-editor`; for plain text use `ui.text`; for a file's source use `code-view`; for ordered steps alone use `numbered-list`.",
	props = {
		["text"] = "the markdown; a `[label](cite:source)` link draws a source chip",
		["cursor"] = "draw a dot after the last word, while the text is still being written",
		["to"] = "unused for now: links show their target as a tooltip",
	},
	styles = {
		[".chat-markdown"] = { gap = 14 },
		[".chat-markdown-line"] = {},
		[".chat-markdown-space"] = { padding = { right = 4 } },
		[".chat-markdown-gap"] = { width = 4 },
		[".chat-markdown-bold"] = { weight = "bold" },
		-- The host has no italic face yet: italic reads as a quieter colour.
		[".chat-markdown-italic"] = { color = "muted-foreground" },
		[".chat-markdown-code"] = { font = "mono", size = "small", background = "secondary", radius = 4, padding = { 1, 5 } },
		[".chat-markdown-link"] = { color = "accent" },
		[".chat-markdown-h1"] = { size = "title", weight = "bold" },
		[".chat-markdown-h2"] = { size = "large", weight = "bold" },
		[".chat-markdown-h3"] = { weight = "bold" },
		[".chat-markdown-cursor"] = { color = "foreground", size = "small", padding = { left = 2 } },
		[".chat-markdown-list"] = { gap = 0, padding = { left = 12 } },
		[".chat-markdown-item"] = { gap = 0 },
		[".chat-markdown-marker"] = { color = "foreground" },
		[".chat-markdown-table"] = { gap = 0 },
		[".chat-markdown-tr"] = { border = "bottom", border_color = "border", padding = { 5, 0 }, gap = 12 },
		[".chat-markdown-thead"] = { padding = { 0, 0, 7, 0 } },
		[".chat-markdown-th"] = { weight = "bold" },
		[".chat-markdown-cell"] = { gap = 0 },
		[".chat-markdown-pre"] = { background = "secondary", radius = 10, gap = 0, border = true, border_color = "border" },
		[".chat-markdown-pre-head"] = { padding = { 6, 12 }, border = "bottom", border_color = "border" },
		[".chat-markdown-pre-language"] = { size = "small", color = "muted-foreground" },
		[".chat-markdown-pre-copy"] = { gap = 4, color = "muted-foreground", radius = 6, padding = { 2, 6 } },
		[".chat-markdown-pre-copy:hover"] = { background = "list-hover", color = "foreground" },
		[".chat-markdown-pre-body"] = { padding = { 10, 14 } },
		[".chat-markdown-pre-line"] = { font = "mono", size = "small" },
		[".chat-markdown-quote"] = { border = "left", border_color = "border", padding = { 0, 0, 0, 12 }, color = "muted-foreground" },
		[".chat-markdown-rule"] = { height = 1, background = "border" },
		[".chat-markdown-cite"] = { background = "secondary", radius = 9, padding = { 2, 7, 2, 3 }, gap = 5 },
		[".chat-markdown-cite-mark"] = { background = "#e5488f", radius = 7 },
		[".chat-markdown-cite-letter"] = { size = "small", weight = "bold", color = "#ffffff" },
		[".chat-markdown-cite-text"] = { size = "small", color = "muted-foreground" },
	},
	example = { text = SAMPLE },
	build = build,
}
