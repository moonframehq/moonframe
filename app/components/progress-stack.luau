-- Progress toward a target in stacked segments: what is done solid, what is
-- likely in lighter tints or hatched, what is missing hatched in a warning
-- colour, a marker where the current pace lands, and a ticked axis. Views
-- cannot measure, so it takes its width in pixels.

type Segment = { value: number, color: string?, tone: string?, hatch: boolean? }

type Props = {
	segments: { Segment },
	total: number,
	width: number?,
	marker: { value: number, label: string }?,
	labels: { string }?,
	ticks: number?,
}

local TONES = { accent = "accent", success = "success", warning = "warning", danger = "danger", muted = "muted-foreground" }

return component {
	title = "Progress Stack",
	category = "Data",
	icon = "chart-column",
	description = "A horizontal bar of stacked segments toward a target, solid or hatched, with a labelled marker and a ticked axis.",
	when = "How far a goal is (a raise, a budget, a quota) split by stage: closed, out, soft, still needed, with where the pace leads. For one fraction use `meter`; for a quota with a reset use `usage-bar`; for task progress use `progress`.",
	props = {
		["segments"] = "list of { value, color? or tone? (default accent), hatch? }, left to right",
		["total"] = "the value at the right end (the target)",
		["width"] = "pixels (default 520)",
		["marker"] = "{ value, label }: a line and a pill over the bar (\"Pace $69M\")",
		["labels"] = "axis labels, evenly spaced from the left end to the right",
		["ticks"] = "small ticks along the axis (default 40)",
	},
	styles = {
		[".progress-stack"] = { gap = 4 },
		[".progress-stack-bar"] = { gap = 2, radius = 8 },
		[".progress-stack-segment"] = { radius = 6 },
		[".progress-stack-marker"] = { background = "danger" },
		[".progress-stack-pill"] = { background = "background", color = "danger", border = true, border_color = "danger/40", radius = 6, padding = { 1, 6 } },
		[".progress-stack-pill-text"] = { size = "small", weight = "medium" },
		[".progress-stack-tick"] = { background = "muted-foreground/40" },
		[".progress-stack-label"] = { size = "small", color = "muted-foreground", font = "mono" },
	},
	example = {
		total = 75,
		width = 420,
		marker = { value = 69, label = "Pace $69M" },
		labels = { "$0M", "$25M", "$50M", "Target $75M" },
		segments = {
			{ value = 41.3, color = "accent", hatch = false },
			{ value = 8.5, color = "accent/45", hatch = false },
			{ value = 12, color = "accent/60", hatch = true },
			{ value = 13.2, color = "danger", hatch = true },
		},
	},

	build = function(props: Props)
		local width = props.width or 520
		local height = 34
		local scale = width / props.total
		local parts = {}
		local used = 0
		for _, segment in props.segments do
			local color = segment.color or TONES[segment.tone or "accent"] or "accent"
			local size = math.max(2, segment.value * scale - 2)
			used += segment.value
			if segment.hatch then
				table.insert(parts, ui.hatch {
					class = "progress-stack-segment",
					width = size,
					height = height,
					gap = 5,
					style = { color = segment.color or color, background = "muted-foreground/8" },
				})
			else
				table.insert(parts, ui.row {
					class = "progress-stack-segment",
					width = size,
					height = height,
					style = { background = color },
				})
			end
		end
		local marker = props.marker
		local at: number = if marker then math.min(width - 1, marker.value * scale) else 0
		local ticks = {}
		for _ = 0, props.ticks or 40 do
			table.insert(ticks, ui.row { class = "progress-stack-tick", width = 1, height = 4 })
		end
		local labels = {}
		local given: { string } = props.labels or {}
		for _, label in given do
			table.insert(labels, ui.text { label, class = "progress-stack-label" })
		end
		return ui.column {
			class = "progress-stack",
			width = width,
			shrink = false,
			marker and ui.row {
				height = 24,
				ui.row {
					class = "progress-stack-pill",
					float = { top = 0, left = math.max(0, at - 50) },
					ui.text { marker.label, class = "progress-stack-pill-text" },
				},
			},
			ui.row {
				height = height,
				gap = "none",
				ui.row { class = "progress-stack-bar", table.unpack(parts) },
				marker and ui.row {
					class = "progress-stack-marker",
					width = 2,
					height = height + 12,
					float = { top = -6, left = at },
				},
			},
			ui.row { justify = "between", gap = "none", table.unpack(ticks) },
			#labels > 0 and ui.row { justify = "between", table.unpack(labels) },
		}
	end,
}
