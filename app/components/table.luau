-- Rows of data under a header of columns. A column has a key into each
-- row, a title, and optionally a width, an alignment and the mono font.
-- Clicking a header sends `{ sort = key }`; clicking a row sends
-- `{ select = id }`. The mod keeps the sort and the selection.
-- With `checkable`, a column of checkboxes leads each row and the header:
-- a row's box sends `{ check = id }`, the header's `check_all` (checked,
-- dashed when only some rows are). `bordered` frames it as a card with a
-- tinted header row in sentence case, as admin tables are.

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
	checkable: boolean?,
	checks: any,
	check: string?,
	check_all: any,
	bordered: boolean?,
}

local JUSTIFY = { left = "start", start = "start", center = "center", right = "end", ["end"] = "end" }

return component {
	title = "Table",
	category = "Data",
	icon = "list",
	description = "Columns over selectable rows, with a sortable header showing the sort arrow, a column of checkboxes, rich cells, zebra stripes or a bordered card look, hover, and an empty text.",
	when = "Tabular data with several aligned fields per row: devices, processes, files, runs; an admin list (audit log, orders) as `bordered` and `checkable`, with `filter-dropdown`s over it and `pagination` under it. For one field per row use `list`; for pull requests use `pr-row`; for label/value pairs use `key-values`.",
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
		["checkable"] = "a leading column of checkboxes, in the header too",
		["checks"] = "the checked rows' ids: a list, or a set { [id] = true }",
		["check"] = "key of the message a row's box sends, default \"check\": { check = id }",
		["check_all"] = "message the header's box sends (default { check_all = true }); it shows checked when every row is, a dash when some are",
		["bordered"] = "frame it as a rounded card with a tinted header in sentence case and lines between rows",
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
		[".table-check"] = {
			radius = 4,
			border = "all",
			border_color = "muted-foreground/40",
			background = "background",
			color = "accent-foreground",
			size = "small",
		},
		[".table-check:hover"] = { border_color = "muted-foreground" },
		[".table-check:selected"] = { background = "accent", border_color = "accent" },
		[".table-bordered"] = { border = "all", border_color = "border", radius = 8, background = "background" },
		[".table-header-bordered"] = { padding = { 9, 16 }, background = "muted-foreground/5", radius = { 8, 8, 0, 0 } },
		[".table-heading-text-bordered"] = { case = "none", size = "normal", weight = "normal" },
		[".table-row-bordered"] = { padding = { 7, 16 }, border_color = "border" },
		[".table-row-last"] = { border = "none" },
		[".table-cell-bordered"] = { size = "normal" },
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
} -- sends { sort = key } and { select = id }

-- An admin table: a card frame, a box per row, rich cells.
table_view {
	bordered = true, checkable = true, checks = state.checks, -- { [id] = true }
	check_all = { check_all = page_ids }, -- the ids on show, to tick them all
	columns = { { key = "activity", title = "Activity" }, { key = "time", title = "Timestamp", width = 220 } },
	rows = { { id = "a1", activity = rich_text { parts = parts }, time = "10/30/2026 09:12 AM" } },
} -- a box sends { check = id }]],
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
		local bordered = props.bordered == true

		-- `checks` as a set, from a list or a set.
		local checked: { [any]: boolean } = {}
		if type(props.checks) == "table" then
			if props.checks[1] ~= nil then
				for _, id in props.checks do
					checked[id] = true
				end
			else
				for id, on in props.checks do
					checked[id] = on == true
				end
			end
		end

		local function checkbox(on: boolean, dash: boolean, msg: any)
			return ui.row {
				width = 24,
				shrink = false,
				align = "center",
				ui.row {
					class = "table-check",
					width = 16,
					height = 16,
					align = "center",
					justify = "center",
					selected = on or dash,
					msg = msg,
					to = props.to,
					(on or dash) and ui.icon { name = if on then "check" else "minus" },
				},
			}
		end

		local function cell(column: Column, content: any)
			local classes = { "table-cell" }
			if bordered then
				table.insert(classes, "table-cell-bordered")
			end
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
				ui.text {
					column.title or column.key,
					class = if bordered then { "table-heading-text", "table-heading-text-bordered" } else "table-heading-text",
					truncate = true,
				},
				sorted and ui.icon { name = if props.descending then "arrow-down" else "arrow-up", size = "small" },
			}
		end

		local all, some = #props.rows > 0, false
		local body = {}
		for index, row in props.rows do
			local id = row.id or index
			local on = checked[id] == true
			all = all and on
			some = some or on
			local cells = {}
			if props.checkable then
				cells[1] = checkbox(on, false, { [props.check or "check"] = id })
			end
			for _, column in props.columns do
				table.insert(cells, cell(column, row[column.key]))
			end
			local classes = { "table-row" }
			if props.zebra and index % 2 == 0 then
				table.insert(classes, "table-row-odd")
			end
			if bordered then
				table.insert(classes, "table-row-bordered")
				if index == #props.rows then
					table.insert(classes, "table-row-last")
				end
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

		if props.checkable then
			table.insert(headings, 1, checkbox(all, some and not all, props.check_all or { check_all = true }))
		end

		local scrolls = props.height ~= nil or props.grow == true
		return ui.column {
			class = if bordered then { "table", "table-bordered" } else "table",
			grow = props.grow,
			height = props.height,
			gap = "none",
			ui.row {
				class = if bordered then { "table-header", "table-header-bordered" } else "table-header",
				shrink = false,
				align = "center",
				table.unpack(headings),
			},
			ui.column { gap = "none", grow = scrolls, scroll = scrolls, table.unpack(body) },
		}
	end,
}
