-- A featured course: an eyebrow, a big title and its facts, a dark
-- primary button to carry on, a quiet second one, a progress line ("3 of 8
-- lessons · 38%"), and anything on the right (a `heat-grid` of activity).

type Action = { label: string, icon: string?, msg: any }

type Props = {
	title: string,
	eyebrow: string?,
	meta: string?,
	primary: Action?,
	secondary: Action?,
	progress: number?,
	label: string?,
	detail: string?,
	bar: number?,
	aside: any,
	to: string?,
}

return component {
	title = "Course Card",
	category = "Data",
	icon = "circle-play",
	description = "A featured course or program: eyebrow, title, facts, a Continue button and a second action, a progress line, and a slot on the right.",
	when = "The one course, plan or program a page puts first, with where the reader is and a button to carry on. For a grid of guides use `guide-card`; for a figure with a trend use `stat-tile`; for a titled box of anything use `card`.",
	props = {
		["title"] = "the course's name",
		["eyebrow"] = "small capitals over the title (\"Featured course\")",
		["meta"] = "a muted line under it (\"8 lessons · 46 min · Beginner\")",
		["primary"] = "{ label, icon?, msg }: the dark button (Continue lesson 4)",
		["secondary"] = "{ label, icon?, msg }: the outlined button beside it",
		["progress"] = "0 to 1: the bar",
		["label"] = "text over the bar's left (\"3 of 8 lessons\")",
		["detail"] = "text over the bar's right (default the percentage)",
		["bar"] = "the bar's width in pixels (default 240)",
		["aside"] = "an element on the right, such as a `heat-grid`",
		["to"] = "mod to send the buttons' msgs to, if not the drawing one",
	},
	styles = {
		[".course-card"] = { radius = 16, border = true, border_color = "border", background = "muted-foreground/4", padding = { 24, 26 }, gap = 24 },
		[".course-card-eyebrow"] = { size = "small", color = "muted-foreground", case = "upper" },
		[".course-card-title"] = { size = "title", weight = "medium" },
		[".course-card-meta"] = { color = "muted-foreground" },
		[".course-card-primary"] = { background = "foreground", color = "background", radius = 999, padding = { 8, 16, 8, 8 }, gap = 8, shadow = "0 4px 10px -4px #00000066" },
		[".course-card-primary:hover"] = { background = "foreground/85" },
		[".course-card-primary-icon"] = { background = "background/20", radius = 999 },
		[".course-card-secondary"] = { background = "background", border = true, border_color = "border", radius = 999, padding = { 8, 14 }, gap = 8 },
		[".course-card-secondary:hover"] = { background = "list-hover" },
		[".course-card-label"] = { size = "small", weight = "medium" },
		[".course-card-detail"] = { size = "small", color = "muted-foreground" },
		[".course-card-track"] = { background = "muted-foreground/14", radius = 999 },
		[".course-card-fill"] = { background = "accent", radius = 999 },
	},
	example = {
		eyebrow = "Featured course",
		title = "The Reader Growth Playbook",
		meta = "8 lessons · 46 min · Beginner",
		primary = { label = "Continue lesson 4", icon = "play", msg = "continue" },
		secondary = { label = "View syllabus", icon = "book-open", msg = "syllabus" },
		progress = 0.38,
		label = "3 of 8 lessons",
	},
	usage = [[
local course_card = use "course-card"
local heat_grid = use "heat-grid"

course_card {
	eyebrow = "Featured course", title = "The Reader Growth Playbook",
	meta = "8 lessons · 46 min · Beginner",
	primary = { label = "Continue lesson 4", icon = "play", msg = "continue" },
	secondary = { label = "View syllabus", icon = "book-open", msg = "syllabus" },
	progress = 3 / 8, label = "3 of 8 lessons",
	aside = heat_grid { rows = 6, columns = { 1, 2, 1, 2, 3, 2, 3, 4, 3, 4, 5, 5, 6, 6 } },
}
]],

	build = function(props: Props)
		local bar = props.bar or 240
		local buttons = {}
		local primary = props.primary
		if primary then
			table.insert(buttons, ui.row {
				class = "course-card-primary",
				align = "center",
				msg = primary.msg,
				to = props.to,
				ui.row {
					class = "course-card-primary-icon",
					width = 20,
					height = 20,
					align = "center",
					justify = "center",
					ui.icon { name = (primary.icon or "play") :: any, size = "small" },
				},
				ui.text { primary.label, weight = "medium" },
			})
		end
		local secondary = props.secondary
		if secondary then
			table.insert(buttons, ui.row {
				class = "course-card-secondary",
				align = "center",
				msg = secondary.msg,
				to = props.to,
				secondary.icon and ui.icon { name = secondary.icon :: any, size = "small" },
				ui.text { secondary.label },
			})
		end
		local progress
		if props.progress then
			local value = math.clamp(props.progress, 0, 1)
			progress = ui.column {
				width = bar,
				gap = "small",
				ui.row {
					ui.text { props.label or "", class = "course-card-label" },
					ui.spacer(),
					ui.text { props.detail or `{math.floor(value * 100 + 0.5)}%`, class = "course-card-detail" },
				},
				ui.row {
					class = "course-card-track",
					height = 6,
					value > 0 and ui.row { class = "course-card-fill", width = math.max(6, bar * value), height = 6 },
				},
			}
		end
		return ui.row {
			class = "course-card",
			align = "center",
			ui.column {
				grow = true,
				gap = "small",
				props.eyebrow and ui.text { props.eyebrow, class = "course-card-eyebrow" },
				ui.text { props.title, class = "course-card-title" },
				props.meta and ui.text { props.meta, class = "course-card-meta" },
				ui.row {
					align = "center",
					gap = "large",
					style = { margin = { top = 14 } },
					ui.row { gap = "normal", align = "center", table.unpack(buttons) },
					ui.row { width = 16 },
					progress,
				},
			},
			props.aside,
		}
	end,
}
