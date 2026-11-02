-- A task board: drag a card to another place in its column or to another
-- column. Each card carries its id; a card over another one marks the gap
-- above or below it, a card over a column's empty end marks its end.
local kanban_column = use "kanban-column"
local board_card = use "board-card"

local COLUMNS = {
	{ id = "todo", title = "To Do", color = "#94a3b8" },
	{ id = "progress", title = "In Progress", color = "#f59e0b" },
	{ id = "review", title = "Review", color = "#a78bfa" },
}

local CARDS: { [string]: any } = {
	web112 = { title = "Finalize navigation", key = "WEB-112", labels = { { label = "Design", color = "#8b5cf6" } }, date = "Today", owner = "TI" },
	web121 = { title = "Review responsive layouts", key = "WEB-121", labels = { { label = "QA", color = "#14b8a6" } }, date = "Oct 15", owner = "EW" },
	web124 = { title = "Prepare developer handoff", key = "WEB-124", labels = { { label = "Frontend", color = "#3b82f6" } }, date = "Oct 21", owner = "SC" },
	web130 = { title = "Build hero component", key = "WEB-130", labels = { { label = "Frontend", color = "#3b82f6" } }, date = "Oct 17", owner = "JC" },
	web133 = { title = "Design mobile onboarding", key = "WEB-133", labels = { { label = "Design", color = "#8b5cf6" } }, date = "Sat", owner = "PP" },
	web140 = { title = "Create homepage wireframes", key = "WEB-140", labels = { { label = "Design", color = "#8b5cf6" } }, date = "Tomorrow", owner = "TI" },
}

-- Where `id` sits: its column and index.
local function find(state, id: string): (string?, number?)
	for column, cards in state.board do
		local at = table.find(cards, id)
		if at then
			return column, at
		end
	end
	return nil, nil
end

-- Moves `id` to before index `at` of `column`.
local function move(state, id: string, column: string, at: number)
	local from, index = find(state, id)
	if not from or not index then
		return
	end
	table.remove(state.board[from], index)
	if from == column and index < at then
		at -= 1
	end
	local cards = state.board[column]
	table.insert(cards, math.clamp(at, 1, #cards + 1), id)
end

return mod {
	init = function(previous)
		return previous
			or {
				board = {
					todo = { "web121", "web124", "web130" },
					progress = { "web112", "web133" },
					review = { "web140" },
				},
				-- Where a dragged card would land: { column, index }.
				target = nil :: { column: string, index: number }?,
			}
	end,

	update = function(state, msg)
		if typeof(msg) ~= "table" then
			return
		end
		if msg.over_card then
			local column, index = find(state, msg.over_card)
			if column and index then
				if msg.phase == "drop" then
					local at = if msg.y > 0.5 then index + 1 else index
					move(state, msg.carry.card, column, at)
					state.target = nil
				elseif msg.phase == "over" then
					state.target = { column = column, index = if msg.y > 0.5 then index + 1 else index }
				end
			end
		elseif msg.over_column then
			local column = msg.over_column
			if msg.phase == "drop" then
				local target = state.target
				local at = if target and target.column == column then target.index else #state.board[column] + 1
				move(state, msg.carry.card, column, at)
				state.target = nil
			elseif msg.phase == "over" then
				-- A card over one of this column's cards already marked the spot.
				if not state.target or state.target.column ~= column then
					state.target = { column = column, index = #state.board[column] + 1 }
				end
			elseif msg.phase == "leave" and state.target and state.target.column == column then
				state.target = nil
			end
		end
	end,

	view = function(state)
		local columns = {}
		for _, column in COLUMNS do
			local cards = {}
			for _, id in state.board[column.id] do
				local card = CARDS[id]
				table.insert(cards, board_card {
					title = card.title,
					key = card.key,
					labels = card.labels,
					date = card.date,
					owner = card.owner,
					carry = { card = id },
					accept = { over_card = id },
				})
			end
			local target = state.target
			local here = target and target.column == column.id
			table.insert(columns, kanban_column {
				title = column.title,
				color = column.color,
				count = #cards,
				accept = { over_column = column.id },
				over = here,
				marker = if target and here then target.index else nil,
				add = { add = column.id },
				width = 236,
				table.unpack(cards),
			})
		end
		return ui.row { gap = "normal", align = "start", scroll = "x", table.unpack(columns) }
	end,
}
