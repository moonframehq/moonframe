-- A card on a board: a title with a small badge (a percentage, a status),
-- a muted line under it, then a figure, a date and the owner's initials at
-- the bottom. A tone outlines it (at risk). Clicks and right-clicks send
-- messages, for opening it or a menu of moves.

local avatar = use "avatar"

type Props = {
	title: string,
	subtitle: string?,
	badge: string?,
	badge_tone: string?,
	amount: string?,
	date: string?,
	owner: string?,
	owner_color: string?,
	tone: string?,
	msg: any,
	context: any,
	to: string?,
	selected: boolean?,
}

local TONES = { danger = "danger", warning = "warning", success = "success", accent = "accent" }

return component {
	title = "Board Card",
	category = "Lists",
	icon = "square",
	description = "A kanban card: title with a badge, a muted line, then an amount, a date and the owner's initials; a tone outlines it.",
	when = "Items in a `kanban-column`: deals, tickets, tasks, candidates. For a row in a list use `task-row` or `list`; for a pull request use `pr-row`.",
	props = {
		["title"] = "what it is (a company, a ticket's title)",
		["subtitle"] = "a muted line under it",
		["badge"] = "a small label on the right (\"30%\", \"Won\", \"At risk\")",
		["badge_tone"] = "the badge's tone: success, danger, warning or muted (default)",
		["amount"] = "the figure at the bottom left",
		["date"] = "a date beside it",
		["owner"] = "the owner's initials, in a round avatar",
		["owner_color"] = "the avatar's colour",
		["tone"] = "outline it: danger (at risk), warning, success",
		["msg"] = "message a click sends",
		["context"] = "message a right-click sends (a menu of moves)",
		["to"] = "mod to send them to, if not the drawing one",
		["selected"] = "highlight it",
	},
	styles = {
		[".board-card"] = { background = "background", radius = 10, padding = { 10, 12 }, gap = 4, border = true, border_color = "border", shadow = "0 1px 2px #0000000a" },
		[".board-card:hover"] = { border_color = "muted-foreground/40" },
		[".board-card:selected"] = { border_color = "accent" },
		[".board-card-title"] = { weight = "medium" },
		[".board-card-sub"] = { size = "small", color = "muted-foreground" },
		[".board-card-badge"] = { size = "small", radius = 6, padding = { 0, 6 }, background = "secondary", color = "muted-foreground" },
		[".board-card-amount"] = { weight = "bold" },
		[".board-card-date"] = { size = "small", color = "muted-foreground" },
		[".board-card-foot"] = { gap = 8, padding = { 6, 0, 0, 0 } },
	},
	example = {
		title = "Lumen Retail",
		subtitle = "Platform renewal",
		badge = "At risk",
		badge_tone = "danger",
		amount = "$98,000",
		date = "Nov 13",
		owner = "TO",
		tone = "danger",
	},

	build = function(props: Props)
		local badge_tone = props.badge_tone and TONES[props.badge_tone]
		local outline = props.tone and TONES[props.tone]
		return ui.column {
			class = "board-card",
			selected = props.selected,
			msg = props.msg,
			context = props.context,
			to = props.to,
			style = if outline then { border_color = `{outline}/50`, background = `{outline}/4` } else nil,
			ui.row {
				align = "center",
				gap = "small",
				ui.text { props.title, class = "board-card-title", truncate = true },
				ui.spacer(),
				props.badge and ui.text {
					props.badge,
					class = "board-card-badge",
					style = if badge_tone then { background = `{badge_tone}/12`, color = badge_tone } else nil,
				},
			},
			props.subtitle and ui.text { props.subtitle, class = "board-card-sub", truncate = true },
			ui.row {
				class = "board-card-foot",
				align = "center",
				props.amount and ui.text { props.amount, class = "board-card-amount" },
				ui.spacer(),
				props.date and ui.text { props.date, class = "board-card-date" },
				props.owner and avatar { name = props.owner, initial = props.owner, color = props.owner_color, shape = "circle" },
			},
		}
	end,
}
