-- A guide or article in a grid of them: a pastel band with the topic's
-- icon, the category in its colour, the title, then a footer with the read
-- time and where the reader is: a bar and a percentage, Done, or a New pill.

type Props = {
	title: string,
	category: string?,
	color: string?,
	tint: string?,
	icon: string?,
	read: string?,
	progress: number?,
	done: boolean?,
	new: boolean?,
	selected: boolean?,
	msg: any,
	to: string?,
	bar: number?,
}

return component {
	title = "Guide Card",
	category = "Data",
	icon = "book-open",
	description = "A guide, article or course in a grid: a pastel band with an icon, the category in colour, the title, the read time, and progress, Done or New.",
	when = "Cards of reading material or courses the reader works through (guides, docs, tutorials), three or so to a row, each with how far the reader got. For one featured course with actions use `course-card`; for a plain titled box use `card`; for a linked record inside a message use `record-card`.",
	props = {
		["title"] = "the guide's title",
		["category"] = "its topic, shown in capitals in `color`",
		["color"] = "the topic's colour: the category, the icon and the band's tint (default accent)",
		["tint"] = "the band's colour instead of `color` at 10%",
		["icon"] = "icon in the band",
		["read"] = "read time, e.g. \"6 min read\"",
		["progress"] = "0 to 1: a bar and a percentage in the footer",
		["done"] = "finished: a full bar and Done in the success colour",
		["new"] = "not started and new: a New pill instead of the bar",
		["selected"] = "highlight the card",
		["msg"] = "message a click sends (open the guide)",
		["to"] = "mod to send `msg` to, if not the drawing one",
		["bar"] = "the bar's width in pixels (default 56)",
	},
	styles = {
		[".guide-card"] = { radius = 14, border = true, border_color = "border", background = "background", gap = 0, shadow = "0 1px 2px #0000000a" },
		[".guide-card:hover"] = { border_color = "muted-foreground/30" },
		[".guide-card:selected"] = { border_color = "accent" },
		[".guide-card-band"] = { radius = { 13, 13, 0, 0 } },
		[".guide-card-icon"] = { size = "title" },
		[".guide-card-body"] = { padding = { 14, 14, 16, 14 }, gap = 6 },
		[".guide-card-category"] = { size = "small", weight = "medium", case = "upper" },
		[".guide-card-title"] = { size = "large" },
		[".guide-card-foot"] = { border = "top", border_color = "border", padding = { 10, 14 }, gap = 8 },
		[".guide-card-read"] = { color = "muted-foreground" },
		[".guide-card-track"] = { background = "muted-foreground/14", radius = 999 },
		[".guide-card-fill"] = { background = "accent", radius = 999 },
		[".guide-card-done"] = { background = "success" },
		[".guide-card-percent"] = { color = "muted-foreground" },
		[".guide-card-done-text"] = { color = "success" },
		[".guide-card-new"] = { background = "accent/12", color = "accent", radius = 999, padding = { 2, 9 }, gap = 5 },
	},
	example = {
		title = "Grow with a referral program",
		category = "Growth",
		color = "#14a38b",
		icon = "rocket",
		read = "9 min read",
		progress = 0.4,
	},
	usage = [[
local guide_card = use "guide-card"

ui.row { gap = "large",
	guide_card { title = "Write subject lines people open", category = "Writing", color = "#7c5cff",
		icon = "a-large-small", read = "6 min read", done = true, msg = { open = "subject-lines" } },
	guide_card { title = "Grow with a referral program", category = "Growth", color = "#14a38b",
		icon = "rocket", read = "9 min read", progress = 0.4, msg = { open = "referral" } },
	guide_card { title = "Price your paid tier", category = "Monetization", color = "#f08a24",
		icon = "tag", read = "12 min read", new = true, msg = { open = "paid-tier" } },
}
]],

	build = function(props: Props)
		local color = props.color or "accent"
		local bar = props.bar or 56
		local status
		if props.done then
			status = ui.row {
				align = "center",
				gap = "normal",
				ui.row { class = { "guide-card-track", "guide-card-done" }, width = bar, height = 4 },
				ui.text { "Done", class = "guide-card-done-text" },
			}
		elseif props.new then
			status = ui.row {
				class = "guide-card-new",
				align = "center",
				ui.row { width = 5, height = 5, style = { background = "accent", radius = 3 } },
				ui.text { "New", size = "small" },
			}
		elseif props.progress then
			local value = math.clamp(props.progress, 0, 1)
			status = ui.row {
				align = "center",
				gap = "normal",
				ui.row {
					class = "guide-card-track",
					width = bar,
					height = 4,
					value > 0 and ui.row { class = "guide-card-fill", width = math.max(4, bar * value), height = 4 },
				},
				ui.row {
					width = 34,
					justify = "end",
					ui.text { `{math.floor(value * 100 + 0.5)}%`, class = "guide-card-percent" },
				},
			}
		end
		return ui.column {
			class = "guide-card",
			grow = true,
			selected = props.selected,
			msg = props.msg,
			to = props.to,
			ui.row {
				class = "guide-card-band",
				height = 78,
				align = "center",
				justify = "center",
				style = { background = props.tint or `{color}/10` },
				props.icon and ui.icon { name = props.icon :: any, size = "title", class = "guide-card-icon", style = { color = color } },
			},
			ui.column {
				class = "guide-card-body",
				grow = true,
				props.category and ui.text { props.category, class = "guide-card-category", style = { color = color } },
				ui.text { props.title, class = "guide-card-title", lines = 2 },
			},
			ui.row {
				class = "guide-card-foot",
				align = "center",
				height = 44,
				ui.icon { name = "clock", size = "small", class = "guide-card-read" },
				props.read and ui.text { props.read, class = "guide-card-read" },
				ui.spacer(),
				status,
			},
		}
	end,
}
