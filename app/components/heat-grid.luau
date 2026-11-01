-- A grid of small rounded squares, one column per bucket (a day, a week),
-- as a contribution graph draws them: a column is either a count, filled
-- from the bottom (a staircase of activity), or a list of levels from the
-- top, each a shade of the colour (0 is an empty square). Labels can sit
-- under the first and last columns ("Oct 5" … "This week").

type Props = {
	columns: { any },
	rows: number?,
	levels: number?,
	size: number?,
	gap: number?,
	color: string?,
	empty: string?,
	spread: boolean?,
	labels: { string }?,
	tooltips: { string }?,
}

-- The fill of a level from 1 to `levels`: the lightest is a pale tint, the
-- last the full colour.
local function shade(color: string, level: number, levels: number): string
	if level >= levels then
		return color
	end
	local alpha = math.floor(30 + 70 * (level - 1) / math.max(levels - 1, 1))
	return `{color}/{alpha}`
end

return component {
	title = "Heat Grid",
	category = "Data",
	icon = "layout-dashboard",
	description = "A contribution grid of rounded squares: columns filled from the bottom by a count, or shaded cell by cell by level, with labels under the ends.",
	when = "Activity over days or weeks at a glance: lessons per week, posts per day, a streak, a course's progress as squares, a GitHub-style contribution graph. For a count compared with the period before use `dot-chart`; for stacked counts with an axis use `bar-chart`; for one fraction use `meter` or `progress`.",
	props = {
		["columns"] = "list, left to right; each a count (squares filled from the bottom) or a list of levels from the top (0 empty, 1 to `levels` lighter to full)",
		["rows"] = "squares per column (default: the tallest column)",
		["levels"] = "shades for level lists (default 4); counts use the full colour",
		["size"] = "a square's side in pixels (default 13)",
		["gap"] = "pixels between squares (default 4)",
		["color"] = "the filled colour (default accent)",
		["empty"] = "an empty square's colour (default muted-foreground at 10%)",
		["spread"] = "columns spread across the parent's width, the first and last at its edges",
		["labels"] = "texts under the grid, spread from the first column to the last",
		["tooltips"] = "one text per column, shown under the pointer",
	},
	styles = {
		[".heat-grid"] = { gap = 8 },
		[".heat-grid-cell"] = { radius = 4 },
		[".heat-grid-label"] = { size = "small", color = "muted-foreground" },
	},
	example = {
		rows = 6,
		columns = { 1, 2, 1, 2, 3, 2, 3, 4, 3, 4, 5, 5, 6, 6 },
		labels = { "Aug", "This week" },
	},
	usage = [[
local heat_grid = use "heat-grid"

-- Counts, filled from the bottom:
heat_grid { rows = 3, columns = { 1, 0, 1, 0, 0, 2, 1, 0, 3, 1, 1, 1 },
	spread = true, size = 16, labels = { "Oct 5", "This week" } }
-- Levels, cell by cell (a single row of eight, two shades):
heat_grid { levels = 2, columns = { { 2 }, { 2 }, { 2 }, { 1 }, { 0 }, { 0 }, { 0 }, { 0 } } }
]],

	build = function(props: Props)
		local size = props.size or 13
		local gap = props.gap or 4
		local color = props.color or "accent"
		local empty = props.empty or "muted-foreground/10"
		local levels = props.levels or 4
		local tallest = 1
		for _, column in props.columns do
			tallest = math.max(tallest, if type(column) == "table" then #column else column)
		end
		local rows: number = props.rows or tallest
		local tooltips: { string } = props.tooltips or {}

		local columns = {}
		for index, column in props.columns do
			local cells = {}
			for row = 1, rows do
				local fill = empty
				if type(column) == "table" then
					local level = column[row] or 0
					if level > 0 then
						fill = shade(color, level, levels)
					end
				elseif column > rows - row then
					fill = color
				end
				table.insert(cells, ui.row {
					class = "heat-grid-cell",
					width = size,
					height = size,
					shrink = false,
					style = { background = fill },
				})
			end
			table.insert(columns, ui.column {
				style = { gap = gap },
				tooltip = tooltips[index],
				table.unpack(cells),
			})
		end

		local labels = {}
		local label_list: { string } = props.labels or {}
		for _, label in label_list do
			table.insert(labels, ui.text { label, class = "heat-grid-label" })
		end
		return ui.column {
			class = "heat-grid",
			hug = not props.spread,
			ui.row {
				justify = if props.spread then "between" else "start",
				style = { gap = gap },
				table.unpack(columns),
			},
			#labels > 0 and ui.row { justify = "between", table.unpack(labels) },
		}
	end,
}
