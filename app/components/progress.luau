-- A thin rounded bar: filled to `value` (0 to 1), or, with `busy`, a
-- striped track with a spinning loader for work of unknown size. A label
-- and the percentage can sit above it.

type Props = {
	value: number?,
	busy: boolean?,
	label: string?,
	detail: string?,
	tone: string?,
	width: number?,
	height: number?,
	percent: boolean?,
}

local TONES = { accent = true, success = true, warning = true, danger = true, muted = true }

return component {
	title = "Progress",
	category = "Data",
	icon = "loader",
	description = "A thin rounded progress bar filled to a fraction, or a striped busy bar with a spinner, with an optional label and percentage.",
	when = "Progress of a task: a download, an index, a build step. Use `busy = true` when the size is unknown. For a reading such as memory use `meter`; for a quota with a reset use `usage-bar`; for a spinner alone use `ui.icon { name = \"loader-circle\", spin = true }`.",
	props = {
		["value"] = "0 to 1",
		["busy"] = "unknown size: a striped track and a spinning loader",
		["label"] = "text above the bar, left",
		["detail"] = "text above the bar, right (default the percentage)",
		["percent"] = "false hides the percentage",
		["tone"] = "accent (default) | success | warning | danger | muted",
		["width"] = "bar width in pixels (default 240)",
		["height"] = "bar thickness in pixels (default 4)",
	},
	styles = {
		[".progress"] = { gap = 6 },
		[".progress-label"] = { size = "small", color = "foreground", weight = "medium" },
		[".progress-detail"] = { size = "small", color = "muted-foreground" },
		[".progress-track"] = { background = "muted-foreground/20", radius = 999 },
		[".progress-fill"] = { background = "accent", radius = 999 },
		[".progress-stripe"] = { background = "accent/45", radius = 999 },
		[".progress-spin"] = { color = "accent", size = "small" },
		[".progress-success"] = { background = "success" },
		[".progress-warning"] = { background = "warning" },
		[".progress-danger"] = { background = "danger" },
		[".progress-muted"] = { background = "muted-foreground" },
	},
	example = { label = "Indexing workspace", value = 0.64, detail = "1,284 of 2,006 files", width = 280 },

	build = function(props: Props)
		local width = props.width or 240
		local height = props.height or 4
		local tone = props.tone or "accent"
		local fill_classes = { "progress-fill" }
		if TONES[tone] and tone ~= "accent" then
			table.insert(fill_classes, `progress-{tone}`)
		end

		local bar
		if props.busy then
			-- A row of short segments: a striped look that reads as "working".
			local segments = {}
			local step = 14
			for index = 1, math.floor(width / step) do
				segments[index] = ui.row {
					class = if index % 3 == 0 then fill_classes else "progress-stripe",
					width = step - 4,
					height = height,
				}
			end
			bar = ui.row {
				class = "progress-track",
				width = width,
				height = height,
				style = { gap = 4 },
				table.unpack(segments),
			}
		else
			local value = math.clamp(props.value or 0, 0, 1)
			bar = ui.row {
				class = "progress-track",
				width = width,
				height = height,
				ui.row { class = fill_classes, width = value * width, height = height },
			}
		end

		local detail = props.detail
		if detail == nil and not props.busy and props.percent ~= false then
			detail = `{math.floor(math.clamp(props.value or 0, 0, 1) * 100 + 0.5)}%`
		end
		local header = if props.label or detail or props.busy
			then ui.row {
				align = "center",
				gap = "small",
				width = width,
				props.busy and ui.icon { name = "loader-circle", spin = true, class = "progress-spin" },
				props.label and ui.text { props.label, class = "progress-label", truncate = true },
				ui.spacer(),
				detail and ui.text { detail, class = "progress-detail" },
			}
			else nil
		return ui.column { class = "progress", header, bar }
	end,
}
