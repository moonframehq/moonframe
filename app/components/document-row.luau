-- A document in a writing app's library: its title in bold, a couple of
-- lines of what it says, and when it changed with its length; a star when
-- it is a favourite. The chosen one is tinted. A click sends `msg`, a
-- right-click `context`.

type Props = {
	title: string,
	excerpt: string?,
	date: string?,
	words: number?,
	favorite: boolean?,
	selected: boolean?,
	lines: number?,
	msg: any?,
	context: any?,
	to: string?,
}

-- "1,234 words": a number with thousands separators.
local function count(words: number): string
	local digits = tostring(math.floor(words))
	local grouped = digits:reverse():gsub("(%d%d%d)", "%1,"):reverse()
	grouped = grouped:gsub("^,", "")
	return `{grouped} {if words == 1 then "word" else "words"}`
end

return component {
	title = "Document Row",
	category = "Lists",
	icon = "file-text",
	description = "A document in a library list: bold title, a two-line excerpt, the date it changed and its word count, a star for favourites; selectable.",
	when = "The list of documents in a writing or notes app (drafts, articles, notes), stacked in a scrolling column beside the editor. For mail use `mail-row`; for tasks use `task-row`; for files in folders use `tree` or `file-tree`; for a plain row use `list`.",
	props = {
		["title"] = "the document's title (its first line)",
		["excerpt"] = "what it says after the title, cut to `lines` lines",
		["date"] = "when it last changed, as shown (\"Today, 14:02\")",
		["words"] = "its length in words, shown after the date",
		["favorite"] = "a star after the title",
		["selected"] = "the open document: tinted",
		["lines"] = "lines of excerpt shown (default 2)",
		["msg"] = "message a click sends (open it)",
		["context"] = "message a right-click sends (a menu of actions)",
		["to"] = "mod to send them to, if not the drawing one",
	},
	styles = {
		[".document-row"] = { padding = { 12, 16 }, gap = 4, radius = 10, transition = "fade fast" },
		[".document-row:hover"] = { background = "list-hover" },
		[".document-row:selected"] = { background = "list-active" },
		[".document-row-title"] = { weight = "semibold" },
		[".document-row-excerpt"] = { size = "small", color = "muted-foreground" },
		[".document-row-meta"] = { size = "small", color = "muted-foreground", opacity = 0.8 },
		[".document-row-star"] = { size = "small", color = "warning" },
	},
	example = {
		title = "On short sentences",
		excerpt = "A short sentence is a door. It opens, you walk through, and you are somewhere new. Long sentences have their place, of course.",
		date = "Today, 09:41",
		words = 1284,
		favorite = true,
		selected = true,
	},

	build = function(props: Props)
		local meta = props.date or ""
		if props.words then
			meta = if meta == "" then count(props.words) else `{meta} · {count(props.words)}`
		end
		return ui.column {
			class = "document-row",
			shrink = false,
			selected = props.selected,
			msg = props.msg,
			context = props.context,
			to = props.to,
			ui.row {
				align = "center",
				gap = "small",
				ui.text { props.title, class = "document-row-title", truncate = true },
				ui.spacer(),
				props.favorite and ui.icon { name = "star-fill", class = "document-row-star" },
			},
			props.excerpt and props.excerpt ~= "" and ui.text {
				props.excerpt,
				class = "document-row-excerpt",
				lines = props.lines or 2,
			},
			meta ~= "" and ui.text { meta, class = "document-row-meta", truncate = true },
		}
	end,
}
