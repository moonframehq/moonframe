-- A spreadsheet grid: numbered rows under typed column headers, with grid
-- lines, a highlighted row, one outlined cell (with a caret while someone
-- is typing in it), grey skeleton bars for cells still to come, and empty
-- numbered rows below. A cell value is text, an element, `false` for a
-- skeleton, or nil for empty. Clicking a cell sends `{ cell = { row, key } }`.

type Column = {
	key: string,
	title: string?,
	icon: string?,
	glyph: string?,
	width: number?,
	align: string?,
	skeleton: number?,
}

type Props = {
	columns: { Column },
	rows: { { [string]: any } },
	current: number?,
	selected: { row: number, key: string }?,
	caret: boolean?,
	blank: number?,
	numbers: boolean?,
	row_height: number?,
	event: string?,
	to: string?,
	grow: boolean?,
	height: number?,
	id: string?,
}

-- Lists in the preview mix optional fields; typed loosely for the checker.
local function loose(list: { any }): { any }
	return list
end

local JUSTIFY = { left = "start", start = "start", center = "center", right = "end", ["end"] = "end" }
-- Past this many rows a grid that scrolls is a `ui.list`, which builds only
-- the rows that show: a view holds at most 10,000 elements.
local LIST_AFTER = 200

return component {
	title = "Spreadsheet",
	category = "Data",
	icon = "columns-2",
	description = "A spreadsheet grid: row numbers, typed column headers, grid lines, a highlighted row, an outlined cell with a caret, skeleton cells and empty rows.",
	when = "Data laid out as a spreadsheet the user (or an agent) fills cell by cell: research sheets, imports, editable records, with cells still loading shown as skeleton bars. For a read-only list of records with sorting use `table`; for label/value pairs use `key-values`.",
	props = {
		["columns"] = "list of { key, title, icon? or glyph? (a short text such as \"%\" or \"$\") before the title, width (px; else it grows), align (left|center|right), skeleton? (bar width, px) }",
		["rows"] = "list of maps, a value per column key: text, a number, an element, `false` for a skeleton bar, or nil for empty",
		["current"] = "index of the highlighted row (the one being worked on)",
		["selected"] = "{ row, key }: the outlined cell",
		["caret"] = "draw a caret after the selected cell's content (someone is typing)",
		["blank"] = "empty numbered rows after the data (default 0)",
		["numbers"] = "false hides the row number column",
		["row_height"] = "pixels per row (default 34)",
		["event"] = "key of the message a cell click sends, default \"cell\": { cell = { row, key } }",
		["to"] = "mod to send it to, if not the drawing one",
		["grow"] = "fill the parent and scroll the rows",
		["height"] = "pixels: the rows scroll within it",
		["id"] = "names the list a long scrolling grid becomes (default \"spreadsheet\"); two long grids in one mod need two",
	},
	styles = {
		[".spreadsheet-grid"] = { gap = 0 },
		[".spreadsheet-head"] = { border = "bottom", border_color = "border" },
		[".spreadsheet-head-cell"] = { padding = { 0, 10 }, gap = 6, border = "right", border_color = "border" },
		[".spreadsheet-head-icon"] = { color = "muted-foreground" },
		[".spreadsheet-head-text"] = { size = "small", weight = "medium" },
		[".spreadsheet-row"] = { border = "bottom", border_color = "border" },
		[".spreadsheet-row:hover"] = { background = "list-hover" },
		[".spreadsheet-row:selected"] = { background = "accent/12" },
		[".spreadsheet-cell"] = { padding = { 0, 10 }, gap = 6, border = "right", border_color = "border" },
		[".spreadsheet-cell-selected"] = { border = "all", border_color = "accent", shadow = "inset 0 0 0 1 accent", background = "background" },
		[".spreadsheet-number"] = { padding = { 0, 10 }, border = "right", border_color = "border" },
		[".spreadsheet-number-text"] = { size = "small", color = "muted-foreground" },
		[".spreadsheet-text"] = {},
		[".spreadsheet-skeleton"] = { background = "muted-foreground/15", radius = 4 },
		[".spreadsheet-caret"] = { background = "accent" },
	},
	usage = [[local spreadsheet = use "spreadsheet"

spreadsheet {
	columns = {
		{ key = "company", title = "Company", icon = "case-sensitive", width = 150 },
		{ key = "apy", title = "APY", glyph = "%", width = 80 },
		{ key = "fee", title = "Monthly fee", glyph = "$", width = 100 },
		{ key = "source", title = "Source", icon = "link" },
	},
	rows = state.rows, -- { { company = "Bayline", apy = "4.35%", fee = false, source = "bayline.com" }, ... }
	current = 2, selected = { row = 2, key = "fee" }, caret = true,
	blank = 4, grow = true,
} -- sends { cell = { row = 2, key = "fee" } }]],
	example = {
		columns = loose {
			{ key = "company", title = "Company", icon = "case-sensitive", width = 130 },
			{ key = "apy", title = "APY", glyph = "%", width = 76 },
			{ key = "fee", title = "Monthly fee", glyph = "$", width = 104 },
			{ key = "source", title = "Source", icon = "link" },
		},
		rows = loose {
			{ company = "Bayline", apy = "4.35%", fee = "$0", source = "bayline.com" },
			{ company = "Kestrel", apy = "4.60%", fee = "$0", source = "kestrel.app" },
			{ company = "Northgate", apy = "4.10%", fee = false, source = "northgate.com" },
			{ company = "Tendril", apy = false, fee = false, source = false },
		},
		current = 3,
		selected = { row = 3, key = "apy" },
		caret = true,
		blank = 2,
	},

	build = function(props: Props)
		local height = props.row_height or 34
		local numbers = props.numbers ~= false
		local selected = props.selected
		local event = props.event or "cell"

		local function cell(column: Column, value: any, index: number?)
			local chosen = index ~= nil and selected ~= nil and selected.row == index and selected.key == column.key
			local content: any = nil
			if value == false then
				content = ui.row { class = "spreadsheet-skeleton", width = column.skeleton or 44, height = 8 }
			elseif type(value) == "table" then
				content = value
			elseif value ~= nil then
				content = ui.text { tostring(value), class = "spreadsheet-text", truncate = true }
			end
			return ui.row {
				class = if chosen then { "spreadsheet-cell", "spreadsheet-cell-selected" } else "spreadsheet-cell",
				width = column.width,
				grow = column.width == nil,
				height = height,
				align = "center",
				justify = (JUSTIFY[column.align or "left"] or "start") :: any,
				msg = if index then { [event] = { row = index, key = column.key } } else nil,
				to = props.to,
				content,
				chosen and props.caret and ui.row { class = "spreadsheet-caret", width = 1.5, height = 16 },
			}
		end

		local function number(index: number)
			return ui.row {
				class = "spreadsheet-number",
				width = 44,
				height = height,
				align = "center",
				ui.text { tostring(index), class = "spreadsheet-number-text" },
			}
		end

		local heads = {}
		if numbers then
			table.insert(heads, ui.row {
				class = "spreadsheet-head-cell",
				width = 44,
				height = height,
				align = "center",
				ui.text { "#", class = { "spreadsheet-head-text", "spreadsheet-head-icon" } },
			})
		end
		for _, column in props.columns do
			table.insert(heads, ui.row {
				class = "spreadsheet-head-cell",
				width = column.width,
				grow = column.width == nil,
				height = height,
				align = "center",
				column.icon and ui.icon { name = column.icon :: any, size = "small", class = "spreadsheet-head-icon" },
				column.glyph and ui.text { column.glyph, class = { "spreadsheet-head-text", "spreadsheet-head-icon" } },
				ui.text { column.title or column.key, class = "spreadsheet-head-text", truncate = true },
			})
		end

		-- Row `index`: one of the data, or an empty one after them.
		local filled = #props.rows
		local function draw(index: number)
			local row = props.rows[index]
			local cells = {}
			if numbers then
				table.insert(cells, number(index))
			end
			for _, column in props.columns do
				table.insert(cells, cell(column, row and row[column.key], if row then index else nil))
			end
			return ui.row {
				class = "spreadsheet-row",
				shrink = false,
				gap = "none",
				selected = row ~= nil and index == props.current,
				table.unpack(cells),
			}
		end

		local count = filled + (props.blank or 0)
		local scrolls = props.height ~= nil or props.grow == true
		local rows
		if scrolls and count > LIST_AFTER then
			-- Each row is its cells' height and its bottom border.
			rows = ui.list { id = props.id or "spreadsheet", count = count, row_height = height + 1, row = draw, grow = true }
		else
			local body = table.create(count)
			for index = 1, count do
				body[index] = draw(index)
			end
			rows = ui.column {
				gap = "none",
				grow = scrolls,
				scroll = scrolls,
				ui.column { gap = "none", shrink = false, table.unpack(body) },
			}
		end
		return ui.column {
			class = "spreadsheet-grid",
			grow = props.grow,
			height = props.height,
			gap = "none",
			ui.row { class = "spreadsheet-head", shrink = false, gap = "none", table.unpack(heads) },
			rows,
		}
	end,
}
