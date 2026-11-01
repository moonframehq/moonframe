-- One column of a kanban board: a header (a coloured dot, the title, a
-- count, a total on the right) over a rule in the column's colour, its
-- cards, and an "Add" button at the bottom. The board is a row of these.

type Props = {
	title: string,
	color: string?,
	count: number?,
	total: string?,
	add: any,
	add_label: string?,
	to: string?,
	width: number?,
	[number]: any,
}

return component {
	title = "Kanban Column",
	category = "Lists",
	icon = "layout-dashboard",
	description = "A board column: dot, title, count and total over a coloured rule, its cards, then an Add button.",
	when = "Each stage of a kanban board (deals, tickets, tasks by status); put several in a row that scrolls sideways and fill them with `board-card`. For a list grouped by status without columns use `group-header` over rows.",
	props = {
		["title"] = "the stage's name",
		["color"] = "the dot's and rule's colour (default accent)",
		["count"] = "how many cards, in a small badge",
		["total"] = "a figure on the right (\"$412K\")",
		["add"] = "message the Add button sends; no button without it",
		["add_label"] = "the button's text (default \"Add\")",
		["to"] = "mod to send `add` to, if not the drawing one",
		["width"] = "pixels (default 232)",
		["[children]"] = "the cards",
	},
	styles = {
		[".kanban-column"] = { background = "secondary", radius = 12, padding = { 10, 8, 8, 8 }, gap = 8 },
		[".kanban-column-head"] = { padding = { 2, 4, 0, 4 }, gap = 8 },
		[".kanban-column-title"] = { weight = "medium" },
		[".kanban-column-count"] = { size = "small", color = "muted-foreground", background = "background", radius = 6, padding = { 0, 6 } },
		[".kanban-column-total"] = { size = "small", color = "muted-foreground" },
		[".kanban-column-cards"] = { gap = 8 },
		[".kanban-column-add"] = { padding = { 6, 8 }, radius = 8, gap = 6, color = "muted-foreground" },
		[".kanban-column-add:hover"] = { background = "list-hover" },
	},
	example = { title = "Proposal", color = "#a78bfa", count = 3, total = "$284K", add = "add" },
	usage = [[local kanban_column = use "kanban-column"
local board_card = use "board-card"

kanban_column {
	title = "Proposal", color = "#a78bfa", count = 3, total = "$284K", add = { add = "proposal" },
	board_card { title = "Kestrel Airways", subtitle = "Crew rostering suite", amount = "$132,000", date = "Nov 29", badge = "50%" },
}]],

	build = function(props: Props)
		local color = props.color or "accent"
		local cards = {}
		for _, child in ipairs(props) do
			table.insert(cards, child)
		end
		return ui.column {
			class = "kanban-column",
			width = props.width or 232,
			shrink = false,
			ui.row {
				class = "kanban-column-head",
				align = "center",
				ui.row { width = 8, height = 8, style = { background = color, radius = 4 } },
				ui.text { props.title, class = "kanban-column-title" },
				props.count and ui.text { tostring(props.count), class = "kanban-column-count" },
				ui.spacer(),
				props.total and ui.text { props.total, class = "kanban-column-total" },
			},
			ui.row { height = 2, style = { background = color, radius = 1 } },
			ui.column { class = "kanban-column-cards", table.unpack(cards) },
			props.add ~= nil and ui.row {
				class = "kanban-column-add",
				align = "center",
				msg = props.add,
				to = props.to,
				ui.icon { name = "plus", size = "small" },
				ui.text { props.add_label or "Add", size = "small" },
			},
		}
	end,
}
