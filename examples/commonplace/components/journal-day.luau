-- A daily note as an outline: its markdown list drawn as bullets (the
-- file stays a plain list), with the day as a heading. Lines that are not
-- list items show as top-level blocks.

local outline_blocks = use "outline-blocks"

local function body(text: string): string
	if string.sub(text, 1, 4) ~= "---\n" then
		return text
	end
	local close = string.find(text, "\n---\n", 4, true)
	return if close then string.sub(text, close + 5) else text
end

-- The list items of `text` as outline blocks: depth from the indentation
-- (two spaces or a tab a level), tasks with their check.
local function blocks(text: string): { any }
	local out: { any } = {}
	for line in string.gmatch(body(text) .. "\n", "([^\n]*)\n") do
		-- A block anchor ( ^id) is for links, not for reading.
		line = string.gsub(line, "%s%^[%w%-]+%s*$", "")
		local indent, rest = string.match(line, "^([ \t]*)[-*+] (.*)$")
		if indent and rest then
			local spaces = string.len((string.gsub(indent, "\t", "  ")))
			local depth = spaces // 2
			local mark, task = string.match(rest, "^%[([ xX])%] (.*)$")
			if mark and task then
				table.insert(out, { depth = depth, text = task, task = mark ~= " " })
			else
				table.insert(out, { depth = depth, text = rest, task = nil })
			end
		elseif string.match(line, "%S") then
			local plain = string.gsub(line, "^#+%s*", "")
			table.insert(out, { depth = 0, text = plain, task = nil })
		end
	end
	return out
end

return component {
	title = "Journal Day",
	category = "Commonplace",
	icon = "sun",
	description = "A daily note's list as an outline under the day's date.",
	when = "A day in Commonplace's journal. For any list of blocks use `outline-blocks`.",
	props = {
		day = "the notes provider's journal row: { date, today, exists, text }",
		label = "the heading (default the date)",
		link = "key of the message a wikilink click sends: { [link] = title } (default open)",
	},
	styles = {
		[".journal-day"] = { gap = 10, padding = { 20, 24 } },
		[".journal-empty"] = { color = "muted-foreground" },
	},
	example = { day = { date = "2026-11-10", today = true, exists = true, text = "- read: [[Make It Stick]]\n  - retrieval beats re-reading\n- [ ] import Bear notes\n" } },
	build = function(props)
		local day: any = props.day or {}
		local list = blocks(day.text or "")
		return ui.column {
			class = "journal-day",
			ui.text { props.label or day.date or "", size = if day.today then "headline" else "title", weight = "bold" },
			#list > 0 and outline_blocks { blocks = list, link = props.link } or ui.text {
				if day.today then "Nothing yet today. Add a line below." else "Nothing written.",
				class = "journal-empty",
			},
		}
	end,
}
