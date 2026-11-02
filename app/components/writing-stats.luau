-- How long a text is, one figure at a time: words, characters, sentences
-- or reading time, counted from `text`. A click sends `msg` with `show`
-- set to the next figure, for the mod to keep, so the user steps through
-- them as they write.

type Props = {
	text: string?,
	show: ("words" | "characters" | "sentences" | "reading")?,
	wpm: number?,
	msg: any?,
	to: string?,
	tooltip: string?,
}

local ORDER = { "words", "characters", "sentences", "reading" }
local NEXT = {}
for index, name in ORDER do
	NEXT[name] = ORDER[index % #ORDER + 1]
end

local function plural(n: number, one: string, many: string): string
	return `{n} {if n == 1 then one else many}`
end

-- The four figures of `text`.
local function count(text: string, wpm: number)
	local words = 0
	for _ in string.gmatch(text, "[%w'’]+") do
		words += 1
	end
	local sentences = 0
	for _ in string.gmatch(text, "[%w'’)\"*_]+[.!?]+") do
		sentences += 1
	end
	-- A last sentence still being written counts too: words after the
	-- last ending mark.
	local tail = string.match(text, "[.!?][^.!?]*$") or text
	if string.find(tail, "[%w'’]") then
		sentences += 1
	end
	local minutes = math.ceil(words / wpm)
	return {
		words = plural(words, "word", "words"),
		characters = plural(utf8.len(text) or #text, "character", "characters"),
		sentences = plural(sentences, "sentence", "sentences"),
		reading = if words == 0 then "0 min read" else `{minutes} min read`,
	}
end

return component {
	title = "Writing Stats",
	category = "Status bar",
	icon = "file-text",
	description = "A text's length as one figure, words, characters, sentences or reading time, that steps to the next on a click.",
	when = "Under or beside a writing surface (`prose-editor`), in its status bar, where the writer glances at how far they got. For any other reading in a status bar use `stat`; for progress toward a target use `progress` or `meter`.",
	props = {
		["text"] = "the text counted",
		["show"] = "the figure shown: words (default), characters, sentences or reading",
		["wpm"] = "reading speed in words per minute, for the reading time (default 230)",
		["msg"] = "message a click sends, with `show` set to the next figure; not clickable without it",
		["to"] = "mod to send `msg` to, if not the drawing one",
		["tooltip"] = "text under the pointer (default: all four figures)",
	},
	styles = {
		[".writing-stats"] = { padding = { 3, 10 }, radius = 999, color = "muted-foreground", transition = "fade fast" },
		[".writing-stats:hover"] = { background = "list-hover", color = "foreground" },
		[".writing-stats-text"] = { size = "small" },
	},
	example = {
		text = "A short sentence is a door. It opens, you walk through, and you are somewhere new.",
		show = "words",
	},

	build = function(props: Props)
		local text = props.text or ""
		local wpm = props.wpm or 230
		local figures = ui.memo("writing-stats", { text, wpm }, function()
			return count(text, wpm)
		end)
		local show = props.show or "words"
		local msg = props.msg
		if msg ~= nil then
			msg = if type(msg) == "table" then table.clone(msg) else { msg = msg }
			msg.show = NEXT[show]
		end
		return ui.row {
			class = "writing-stats",
			hug = true,
			msg = msg,
			to = props.to,
			tooltip = props.tooltip
				or `{figures.words} · {figures.characters} · {figures.sentences} · {figures.reading}`,
			ui.text { figures[show] or figures.words, class = "writing-stats-text" },
		}
	end,
}
