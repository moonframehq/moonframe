-- A task's priority as signal bars, as issue trackers draw it: three faint
-- dashes for none, one, two or three of three bars lit for low, medium and
-- high, and a filled square with an "!" for urgent.

type Props = {
	priority: string?,
	size: number?,
	tooltip: string?,
	label: (string | boolean)?,
}

-- Each priority's name and how many bars it lights.
local PRIORITIES: { [string]: { name: string, lit: number } } = {
	none = { name = "No priority", lit = 0 },
	low = { name = "Low", lit = 1 },
	medium = { name = "Medium", lit = 2 },
	high = { name = "High", lit = 3 },
	urgent = { name = "Urgent", lit = 0 },
}

local function round(value: number): number
	return math.floor(value + 0.5)
end

-- Three bars rising left to right, the first `lit` of them lit; with
-- `dashes`, three short dashes across the middle instead.
local function bars(size: number, lit: number, dashes: boolean)
	local width = math.max(2, round(size * 0.2))
	local gap = math.max(1, round(size * 0.1))
	local heights = { round(size * 0.4), round(size * 0.62), round(size * 0.84) }
	local parts = {}
	for index = 1, 3 do
		table.insert(parts, ui.row {
			class = if index <= lit then { "priority-icon-bar", "priority-icon-lit" } else "priority-icon-bar",
			width = width,
			height = if dashes then 1.5 else heights[index],
		})
	end
	return ui.row {
		width = size,
		height = size,
		shrink = false,
		justify = "center",
		align = if dashes then "center" else "end",
		style = { gap = gap, padding = if dashes then 0 else { 0, 0, round((size - heights[3]) / 2), 0 } },
		table.unpack(parts),
	}
end

-- A filled rounded square with an "!" cut in it.
local function urgent(size: number)
	local stem = math.max(1.5, round(size * 0.13 * 2) / 2)
	return ui.column {
		class = "priority-icon-urgent",
		width = size,
		height = size,
		shrink = false,
		align = "center",
		justify = "center",
		style = { radius = math.max(2, round(size * 0.22)), gap = stem },
		ui.row { class = "priority-icon-mark", width = stem, height = round(size * 0.36), style = { radius = stem / 2 } },
		ui.row { class = "priority-icon-mark", width = stem, height = stem, style = { radius = stem / 2 } },
	}
end

return component {
	title = "Priority Icon",
	category = "Data",
	icon = "chart-column",
	description = "A task's priority as signal bars: faint dashes for none, one to three bars lit for low, medium and high, a red square with \"!\" for urgent.",
	when = "Beside a task's title in a list, a board card or a detail panel, and in the menu that changes it. For a task's status use `status-icon`; for a coloured label use `pill`.",
	props = {
		["priority"] = "none (default), low, medium, high or urgent",
		["size"] = "the glyph's side in pixels (default 14)",
		["tooltip"] = "text under the pointer (default the priority's name)",
		["label"] = "`true` shows the priority's name after the glyph, a string shows that text",
	},
	styles = {
		[".priority-icon"] = { gap = 6 },
		[".priority-icon-bar"] = { radius = 1, background = "foreground/15" },
		[".priority-icon-lit"] = { background = "foreground/85" },
		[".priority-icon-urgent"] = { background = "danger" },
		[".priority-icon-mark"] = { background = "#ffffff" },
		[".priority-icon-label"] = { size = "small" },
	},
	example = { priority = "high", label = true },
	usage = [[local priority_icon = use "priority-icon"

ui.row { align = "center", gap = "normal",
	priority_icon { priority = "none" },
	priority_icon { priority = "low" },
	priority_icon { priority = "medium" },
	priority_icon { priority = "high" },
	priority_icon { priority = "urgent", label = true },
}]],

	build = function(props: Props)
		local priority = props.priority or "none"
		local known = PRIORITIES[priority]
		if known == nil then
			priority, known = "none", PRIORITIES.none
		end
		local size = props.size or 14
		local mark = if priority == "urgent" then urgent(size) else bars(size, known.lit, priority == "none")
		local label = if props.label == true then known.name elseif type(props.label) == "string" then props.label else nil
		return ui.row {
			class = "priority-icon",
			align = "center",
			hug = true,
			shrink = false,
			tooltip = props.tooltip or known.name,
			mark,
			label and ui.text { label, class = "priority-icon-label" },
		}
	end,
}
