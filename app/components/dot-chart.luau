-- Columns of rounded squares, one column per day (or bucket): each square
-- is a step of the value, filled for this period and pale where the
-- previous period went higher, so two series read at a glance. A value
-- axis on the left and a few labels under it.

type Column = { current: number, previous: number?, tooltip: string? }

type Props = {
	columns: { Column },
	max: number,
	rows: number?,
	ticks: { string }?,
	labels: { string }?,
	size: number?,
	color: string?,
	shade: string?,
}

return component {
	title = "Dot Chart",
	category = "Data",
	icon = "chart-column",
	description = "Columns of rounded squares, one per bucket: filled for this period, pale up to the previous period's value; with an axis and labels.",
	when = "A daily or weekly count compared with the period before (signups, orders), where the squares' rhythm reads better than smooth bars. For stacked counts by kind use `bar-chart`; for a balance against a floor use `forecast-chart`; for a trend line use `ui.line`.",
	props = {
		["columns"] = "list of { current, previous?, tooltip? }, left to right",
		["max"] = "the value at the top",
		["rows"] = "squares in a full column (default 8): each is max / rows",
		["ticks"] = "axis labels from the top (max) down to 0, evenly spaced",
		["labels"] = "labels under the chart, spread evenly from the first column to the last",
		["size"] = "a square's side in pixels (default 13)",
		["color"] = "this period's colour (default accent)",
		["shade"] = "the previous period's colour (default muted-foreground at 12%)",
	},
	styles = {
		[".dot-chart"] = { gap = 6 },
		[".dot-chart-tick"] = { size = "small", color = "muted-foreground" },
		[".dot-chart-label"] = { size = "small", color = "muted-foreground" },
		[".dot-chart-column"] = { gap = 3 },
		[".dot-chart-dot"] = { radius = 3 },
	},
	example = {
		max = 120,
		rows = 8,
		ticks = { "120", "60", "0" },
		labels = { "Oct 2", "Oct 5", "Oct 8" },
		columns = {
			{ current = 48, previous = 60 },
			{ current = 70, previous = 61 },
			{ current = 55, previous = 66 },
			{ current = 90, previous = 85 },
			{ current = 82, previous = 74 },
			{ current = 104, previous = 118 },
		},
	},

	build = function(props: Props)
		local rows = props.rows or 8
		local step = props.max / rows
		local size = props.size or 13
		local color = props.color or "accent"
		local shade = props.shade or "muted-foreground/12"
		local columns = {}
		for _, column in props.columns do
			local squares = {}
			-- Top row first, so a column grows from the bottom.
			for row = rows, 1, -1 do
				local floor = (row - 1) * step
				local fill
				if column.current > floor then
					fill = color
				elseif (column.previous or 0) > floor then
					fill = shade
				end
				table.insert(squares, ui.row {
					class = "dot-chart-dot",
					width = size,
					height = size,
					style = if fill then { background = fill } else nil,
				})
			end
			table.insert(columns, ui.column {
				class = "dot-chart-column",
				grow = true,
				align = "center",
				tooltip = column.tooltip,
				table.unpack(squares),
			})
		end
		local ticks = {}
		local tick_list: { string } = props.ticks or {}
		for _, tick in tick_list do
			table.insert(ticks, ui.text { tick, class = "dot-chart-tick" })
		end
		local labels = {}
		local label_list: { string } = props.labels or {}
		for _, label in label_list do
			table.insert(labels, ui.text { label, class = "dot-chart-label" })
		end
		local height = rows * size + (rows - 1) * 3
		return ui.column {
			class = "dot-chart",
			ui.row {
				gap = "normal",
				align = "end",
				-- As tall as a full column, so 0 sits on the bottom row.
				#ticks > 0 and ui.column {
					width = 30,
					height = height + 6,
					shrink = false,
					justify = "between",
					table.unpack(ticks),
				},
				ui.row { grow = true, gap = "none", align = "end", table.unpack(columns) },
			},
			#labels > 0 and ui.row {
				gap = "normal",
				#ticks > 0 and ui.row { width = 30, shrink = false },
				ui.row { grow = true, justify = "between", table.unpack(labels) },
			},
		}
	end,
}
