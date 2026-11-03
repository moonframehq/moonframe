-- One column of a kanban board: a header (a coloured dot, the title, a
-- count, a total on the right) over a rule in the column's colour, its
-- cards, and an "Add" button at the bottom. The board is a row of these.
-- Cards dragged with `carry` land on it through `accept`; `marker` draws
-- where one would go.

type Props = {
	title: string,
	color: string?,
	count: number?,
	total: string?,
	add: any,
	add_label: string?,
	to: string?,
	width: number?,
	accept: any,
	over: boolean?,
	marker: number?,
	[number]: any,
}

-- The children in order. `ipairs` and `#` stop at the first `nil`, which a
-- child such as `cond and ui.text "x"` leaves, and drop what follows it.
local function positional(props: any): { any }
	local keys = {}
	for key, child in props do
		if type(key) == "number" and child then
			table.insert(keys, key)
		end
	end
	table.sort(keys)
	local list = table.create(#keys)
	for _, key in keys do
		table.insert(list, props[key])
	end
	return list
end

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
		["accept"] = "message sent as a carried card moves over the column, leaves it and is dropped on it, with `phase`, `carry` and `y` added (see `carry` on `ui.row`)",
		["over"] = "a card is over it: tint the column",
		["marker"] = "draw a drop line before the card at this index (one past the last for the end)",
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
		[".kanban-column:over"] = { background = "accent/8" },
		[".kanban-column-marker"] = { background = "accent", radius = 1 },
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
		local cards = positional(props)
		local marker = if props.marker then math.clamp(props.marker, 1, #cards + 1) else nil
		-- Keep each card at the same path and size while the marker moves.
		-- The line is a floating second child, never a sibling of the cards.
		for index, card in cards do
			local before = marker == index
			local after = index == #cards and marker == #cards + 1
			cards[index] = ui.column {
				gap = "none",
				card,
				(before or after) and ui.row {
					class = "kanban-column-marker",
					height = 2,
					float = if before then "top" else "bottom",
				},
			}
		end
		if #cards == 0 and marker then
			table.insert(cards, ui.row { class = "kanban-column-marker", height = 2, float = "top" })
		end
		return ui.column {
			class = "kanban-column",
			state = if props.over then "over" else nil,
			accept = props.accept,
			to = if props.accept ~= nil then props.to else nil,
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
