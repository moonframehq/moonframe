-- Rows of data under a header of columns. A column has a key into each
-- row, a title, and optionally a width, an alignment and the mono font.
-- Clicking a header sends `{ sort = key }`; clicking a row sends
-- `{ select = id }`. The mod keeps the sort and the selection.

type Column = {
	key: string,
	title: string?,
	width: number?,
	align: string?,
	mono: boolean?,
	muted: boolean?,
	sortable: boolean?,
}

type Props = {
	columns: { Column },
	rows: { { [string]: any } },
	selected: any,
	sort: string?,
	descending: boolean?,
	sortable: boolean?,
	zebra: boolean?,
	empty: string?,
	event: string?,
	to: string?,
	height: number?,
	grow: boolean?,
}

local JUSTIFY = { left = "start", start = "start", center = "center", right = "end", ["end"] = "end" }

return component {
	title = "Table",
	category = "Data",
	icon = "list",
	description = "Columns over selectable rows, with a sortable header showing the sort arrow, zebra stripes, hover, and an empty text.",
	when = "Tabular data with several aligned fields per row: devices, processes, files, runs. For one field per row use `list`; for pull requests use `pr-row`; for label/value pairs use `key-values`.",
	props = {
		["columns"] = "list of { key, title, width (px; else it grows), align (left|center|right), mono, muted, sortable }",
		["rows"] = "list of maps: `id` plus a value per column key; a value may be a string, a number or an element",
		["selected"] = "id of the selected row",
		["sort"] = "key of the sorted column (shows an arrow)",
		["descending"] = "the sort arrow points down",
		["sortable"] = "false makes no header clickable (a column can also set `sortable = false`)",
		["zebra"] = "stripe every other row",
		["empty"] = "text shown when there are no rows (default \"Nothing to show\")",
		["event"] = "row message key instead of \"select\": sends { [event] = id }",
		["to"] = "the mod to send { sort = key } and { select = id } to",
		["height"] = "pixels: the body scrolls within it",
		["grow"] = "fill the parent and scroll the body",
	},
	styles = {
		[".table"] = { gap = 0 },
		[".table-header"] = { padding = { 6, 10 }, gap = 12, border = "bottom", border_color = "border" },
		[".table-heading"] = { gap = 4, color = "muted-foreground" },
		[".table-heading-text"] = { size = "small", weight = "medium", case = "upper" },
		[".table-heading:hover"] = { color = "foreground" },
		[".table-heading-sorted"] = { color = "foreground" },
		[".table-row"] = { padding = { 7, 10 }, gap = 12, border = "bottom", border_color = "border/60" },
		[".table-row-odd"] = { background = "muted-foreground/5" },
		[".table-row:hover"] = { background = "list-hover" },
		[".table-row:selected"] = { background = "list-active" },
		[".table-cell"] = { size = "small" },
		[".table-cell-mono"] = { font = "mono" },
		[".table-cell-muted"] = { color = "muted-foreground" },
		[".table-empty"] = { padding = 24 },
		[".table-empty-text"] = { color = "muted-foreground", size = "small" },
	},
	usage = [[local table_view = use "table"

table_view {
	columns = {
		{ key = "name", title = "Device" },
		{ key = "cpu", title = "CPU", width = 56, align = "right", mono = true },
		{ key = "seen", title = "Seen", width = 56, align = "right", muted = true },
	},
	rows = state.devices, -- { { id = "studio", name = "mac-studio", cpu = "42%", seen = "now" }, ... }
	selected = state.selected, sort = state.sort, descending = state.descending,
	zebra = true, grow = true,
} -- sends { sort = key } and { select = id }]],
	example = {
		sort = "cpu",
		descending = true,
		selected = "studio",
		zebra = true,
		columns = {
			{ key = "name", title = "Device" },
			{ key = "os", title = "OS", width = 110, muted = true },
			{ key = "cpu", title = "CPU", width = 56, align = "right", mono = true },
			{ key = "memory", title = "Memory", width = 72, align = "right", mono = true },
			{ key = "seen", title = "Seen", width = 56, align = "right", muted = true },
		},
		rows = {
			{ id = "studio", name = "mac-studio", os = "macOS 26.1", cpu = "42%", memory = "72%", seen = "now" },
			{ id = "build", name = "build-01", os = "Ubuntu 24.04", cpu = "31%", memory = "48%", seen = "1m" },
			{ id = "air", name = "macbook-air", os = "macOS 26.0", cpu = "8%", memory = "61%", seen = "5m" },
			{ id = "pi", name = "pi-kitchen", os = "Debian 12", cpu = "3%", memory = "22%", seen = "2h" },
		},
	},

	build = function(props: Props)
		local sortable = props.sortable ~= false

		local function cell(column: Column, content: any)
			local classes = { "table-cell" }
			if column.mono then
				table.insert(classes, "table-cell-mono")
			end
			if column.muted then
				table.insert(classes, "table-cell-muted")
			end
			local child = content
			if type(content) ~= "table" then
				child = ui.text { if content == nil then "" else tostring(content), class = classes, truncate = true }
			end
			return ui.row {
				width = column.width,
				grow = column.width == nil,
				align = "center",
				justify = (JUSTIFY[column.align or "left"] or "start") :: any,
				child,
			}
		end

		local headings = {}
		for index, column in props.columns do
			local sorted = props.sort == column.key
			local clickable = sortable and column.sortable ~= false
			local classes = { "table-heading" }
			if sorted then
				table.insert(classes, "table-heading-sorted")
			end
			headings[index] = ui.row {
				class = classes,
				width = column.width,
				grow = column.width == nil,
				align = "center",
				justify = (JUSTIFY[column.align or "left"] or "start") :: any,
				msg = if clickable then { sort = column.key } else nil,
				to = props.to,
				ui.text { column.title or column.key, class = "table-heading-text", truncate = true },
				sorted and ui.icon { name = if props.descending then "arrow-down" else "arrow-up", size = "small" },
			}
		end

		local body = {}
		for index, row in props.rows do
			local id = row.id or index
			local cells = {}
			for c, column in props.columns do
				cells[c] = cell(column, row[column.key])
			end
			local classes = { "table-row" }
			if props.zebra and index % 2 == 0 then
				table.insert(classes, "table-row-odd")
			end
			body[index] = ui.row {
				class = classes,
				shrink = false,
				align = "center",
				selected = id == props.selected,
				msg = row.msg or { [props.event or "select"] = id },
				to = props.to,
				table.unpack(cells),
			}
		end
		if #body == 0 then
			body[1] = ui.row {
				class = "table-empty",
				justify = "center",
				ui.text { props.empty or "Nothing to show", class = "table-empty-text" },
			}
		end

		local scrolls = props.height ~= nil or props.grow == true
		return ui.column {
			class = "table",
			grow = props.grow,
			height = props.height,
			gap = "none",
			ui.row { class = "table-header", shrink = false, align = "center", table.unpack(headings) },
			ui.column { gap = "none", grow = scrolls, scroll = scrolls, table.unpack(body) },
		}
	end,
}
