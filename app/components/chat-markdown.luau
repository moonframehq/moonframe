-- Markdown as a chat answer: a `ui.markdown`, so it reads the same as any
-- document, with source chips for links to `cite:` and a fade on what a
-- streamed answer adds. It reads half-written text too: with `cursor` on, a
-- `**`, `_` or backtick not closed yet in the last paragraph already styles
-- what follows it, so an answer reads right as it grows, and a dot follows the
-- last word.

type Props = {
	text: string?,
	cursor: boolean?,
	link: any,
	to: string?,
}

-- The marks a paragraph leaves open, closed in order, innermost first.
local function closers(paragraph: string): string
	local stack: { string } = {}
	local function flip(mark: string)
		if stack[#stack] == mark then
			table.remove(stack)
		else
			table.insert(stack, mark)
		end
	end
	local i, length = 1, #paragraph
	while i <= length do
		local c = string.sub(paragraph, i, i)
		local pair = string.sub(paragraph, i, i + 1)
		if c == "\\" then
			i += 2
		elseif c == "`" then
			local close = string.find(paragraph, "`", i + 1, true)
			if not close then
				table.insert(stack, "`")
				break
			end
			i = close + 1
		elseif pair == "**" or pair == "__" then
			flip(pair)
			i += 2
		elseif c == "*" or c == "_" then
			-- A closing underscore follows a word; an intraword one has a
			-- word on both sides. Only the latter belongs to snake_case.
			local before = string.sub(paragraph, i - 1, i - 1)
			local after = string.sub(paragraph, i + 1, i + 1)
			local closes = stack[#stack] == "_" and before ~= "" and not string.match(before, "%s")
				and (after == "" or string.match(after, "[%s%p]"))
			local opens = after ~= "" and not string.match(after, "%s")
				and (i == 1 or string.match(before, "[%s%p]"))
			if c == "*" or closes or opens then
				flip(c)
			end
			i += 1
		else
			i += 1
		end
	end
	local closing = ""
	for index = #stack, 1, -1 do
		closing ..= stack[index]
	end
	return closing
end

-- Whether a fence is open, or the last nonblank line closed one. A fence
-- only closes with the same mark and at least as many of it.
local function fence_tail(text: string): (boolean, boolean)
	local open: string? = nil
	local closed = false
	for line in string.gmatch(text, "[^\n]+") do
		local mark, rest = string.match(line, "^%s*(```+)(.*)$")
		if not mark then
			mark, rest = string.match(line, "^%s*(~~~+)(.*)$")
		end
		if string.find(line, "%S") then
			closed = false
		end
		if mark then
			if not open then
				open = mark
			elseif string.sub(mark, 1, 1) == string.sub(open, 1, 1) and #mark >= #open
				and string.match(rest or "", "^%s*$") then
				open = nil
				closed = true
			end
		end
	end
	return open ~= nil, closed
end

-- `text` as it should read while still being written.
local function growing(text: string): string
	local open, closed = fence_tail(text)
	if open then
		return text
	end
	local kept = string.gsub(text, "%s+$", "")
	if closed then
		return `{kept}\n\n[](cursor:)`
	end
	local last = string.match(kept, ".*\n%s*\n(.*)$") or kept
	local closing = closers(last)
	-- A `*` alone at the start of a line is a list about to begin.
	if string.match(last, "^%s*[%*%-%+]%s*$") then
		closing = ""
	end
	return `{kept}{closing} [](cursor:)`
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
		["cursor"] = "the text is still being written: new words fade in, open marks style what follows them, and a dot follows the last word",
		["link"] = "message a link click sends, with `url` added, as `markdown` sends it",
		["to"] = "mod to send `link` to, if not the drawing one",
	},
	styles = {
		[".chat-markdown"] = { padding = 0 },
	},
	example = { text = SAMPLE },

	build = function(props: Props)
		local text = props.text or ""
		return ui.markdown {
			class = "chat-markdown",
			text = if props.cursor then growing(text) else text,
			stream = props.cursor,
			link = props.link,
			to = if props.link then props.to else nil,
		}
	end,
}
