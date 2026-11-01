-- One headline figure with how it splits: a coloured dot and title with a
-- ↗ link, the figure large beside a muted caption, a bar of rounded
-- segments (the parts of the whole, or one segment per item) and a small
-- mono legend under it. Compliance and operations dashboards set five of
-- these side by side in a bordered strip.

type Segment = { share: number, color: string?, dot: boolean?, label: string? }
type Legend = { text: string, color: string? }

type Props = {
	title: string,
	value: string,
	caption: string?,
	caption_color: string?,
	dot: string?,
	mono: boolean?,
	segments: { Segment }?,
	legend: { Legend }?,
	width: number?,
	msg: any,
	to: string?,
}

return component {
	title = "KPI Meter",
	category = "Data",
	icon = "chart-column",
	description = "A headline figure with a dot and title, the value beside a caption, a bar of rounded segments and a mono legend.",
	when = "A strip of headline figures where each one is a part of a whole (on time / late, fixed / open, one segment per notice). For a figure with a change pill use `kpi-cell`; for one fraction use `meter`; for a whole split into labelled parts in a card use `share-bar`.",
	props = {
		["title"] = "what it counts",
		["value"] = "the figure, as text",
		["caption"] = "muted text beside the figure",
		["caption_color"] = "a colour for the caption instead of muted (\"danger\")",
		["dot"] = "colour of the dot before the title (default accent)",
		["mono"] = "set the figure in the monospace font",
		["segments"] = "list of { share (any unit, summed), color?, dot? (a small round end marker), label? (tooltip) }, left to right",
		["legend"] = "list of { text, color? }: dotted mono labels under the bar, spread across it",
		["width"] = "the bar's width in pixels (default 190)",
		["msg"] = "message the ↗ sends; no arrow without it",
		["to"] = "mod to send `msg` to, if not the drawing one",
	},
	styles = {
		[".kpi-meter"] = { padding = { 16, 18, 14, 18 }, gap = 10 },
		[".kpi-meter-title"] = { color = "foreground" },
		[".kpi-meter-dot"] = { radius = 3 },
		[".kpi-meter-arrow"] = { color = "muted-foreground", radius = 4, padding = { 0, 3 } },
		[".kpi-meter-arrow:hover"] = { color = "foreground", background = "muted-foreground/12" },
		[".kpi-meter-value"] = { size = "title" },
		[".kpi-meter-mono"] = { font = "mono" },
		[".kpi-meter-caption"] = { size = "small", color = "muted-foreground" },
		[".kpi-meter-bar"] = { gap = 3 },
		[".kpi-meter-segment"] = { radius = 3 },
		[".kpi-meter-legend"] = { size = "small", font = "mono", color = "muted-foreground" },
	},
	example = {
		title = "Exceptions open",
		dot = "#c9961a",
		value = "9",
		caption = "of 55 invoices left",
		segments = { { share = 46, color = "#6f9bd1" }, { share = 9, color = "#c9961a" } },
		legend = { { text = "46 fixed", color = "#6f9bd1" }, { text = "9 open", color = "#c9961a" } },
		msg = "open",
	},

	build = function(props: Props)
		local width = props.width or 190
		local segments: { Segment } = props.segments or {}
		local total, gaps, dots = 0, 0, 0
		for index, segment in segments do
			if segment.dot then
				dots += 6
			else
				total += segment.share
			end
			if index > 1 then
				gaps += 3
			end
		end
		local room = width - gaps - dots
		local bar = {}
		for _, segment in segments do
			if segment.dot then
				table.insert(bar, ui.row {
					class = "kpi-meter-segment",
					width = 6,
					height = 6,
					shrink = false,
					tooltip = segment.label,
					style = { background = segment.color or "danger" },
				})
			else
				table.insert(bar, ui.row {
					class = "kpi-meter-segment",
					width = math.max(4, math.floor(room * segment.share / math.max(total, 1))),
					height = 6,
					shrink = false,
					tooltip = segment.label,
					style = { background = segment.color or "accent" },
				})
			end
		end
		local legend = {}
		local items: { Legend } = props.legend or {}
		for _, item in items do
			table.insert(legend, ui.row {
				gap = "small",
				align = "center",
				ui.row { class = "kpi-meter-dot", width = 5, height = 5, style = { background = item.color or "muted-foreground" } },
				ui.text { item.text, class = "kpi-meter-legend", wrap = false },
			})
		end
		return ui.column {
			class = "kpi-meter",
			grow = true,
			ui.row {
				align = "center",
				gap = "normal",
				ui.row { class = "kpi-meter-dot", width = 6, height = 6, shrink = false, style = { background = props.dot or "accent" } },
				ui.text { props.title, class = "kpi-meter-title", truncate = true },
				ui.spacer(),
				props.msg ~= nil and ui.row { class = "kpi-meter-arrow", msg = props.msg, to = props.to, tooltip = "Open", ui.text { "↗", size = "small" } },
			},
			ui.row {
				align = "center",
				gap = "normal",
				ui.text { props.value, class = if props.mono then { "kpi-meter-value", "kpi-meter-mono" } else "kpi-meter-value", wrap = false },
				props.caption and ui.text {
					props.caption,
					class = "kpi-meter-caption",
					truncate = true,
					style = if props.caption_color then { color = props.caption_color } else nil,
				},
			},
			#bar > 0 and ui.row { class = "kpi-meter-bar", align = "center", width = width, shrink = false, table.unpack(bar) },
			#legend > 0 and ui.row { justify = "between", width = width, table.unpack(legend) },
		}
	end,
}
