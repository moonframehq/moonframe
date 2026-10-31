-- A small round count, for the corner of an icon or beside a tab's label.
-- Counts past `max` (99) show as "99+"; zero hides it unless `zero`.

type Props = {
	count: number,
	max: number?,
	tone: string?,
	zero: boolean?,
	tooltip: string?,
	float: any,
}

return component {
	title = "Count Bubble",
	category = "Data",
	icon = "bell",
	description = "A small round count badge for icons and tabs, capped at 99+.",
	when = "An unread or pending count on a sidebar icon, a tab or a menu entry; pass `float = { top = -4, right = -4 }` to put it on the corner of its parent. For a status use `status-dot`; for a text label use `ui.badge`; for added/removed lines use `diff-counts`.",
	props = {
		["count"] = "the number",
		["max"] = "largest number shown before \"N+\" (default 99)",
		["tone"] = "accent (default) | danger | warning | success | muted",
		["zero"] = "show 0 instead of nothing",
		["tooltip"] = "text under the pointer",
		["float"] = "corner offsets to sit over its parent, e.g. { top = -4, right = -4 } on an icon button",
	},
	styles = {
		[".count-bubble"] = {
			radius = 999,
			padding = { 0, 5 },
			background = "accent",
			color = "accent-foreground",
		},
		[".count-bubble-text"] = { size = "small", weight = "bold" },
		[".count-bubble-danger"] = { background = "danger", color = "#ffffff" },
		[".count-bubble-warning"] = { background = "warning", color = "#1e1e2e" },
		[".count-bubble-success"] = { background = "success", color = "#1e1e2e" },
		[".count-bubble-muted"] = { background = "muted-foreground/30", color = "foreground" },
	},
	example = { count = 128, tone = "danger" },

	build = function(props: Props)
		local count = math.max(0, math.floor(props.count or 0))
		if count == 0 and not props.zero then
			return nil
		end
		local max = props.max or 99
		local text = if count > max then `{max}+` else tostring(count)
		local classes = { "count-bubble" }
		if props.tone and props.tone ~= "accent" then
			table.insert(classes, `count-bubble-{props.tone}`)
		end
		return ui.row {
			class = classes,
			hug = true,
			shrink = false,
			float = props.float,
			align = "center",
			justify = "center",
			height = 18,
			-- At least round: one digit fills a circle.
			width = if #text == 1 then 18 else nil,
			tooltip = props.tooltip,
			ui.text { text, class = "count-bubble-text" },
		}
	end,
}
