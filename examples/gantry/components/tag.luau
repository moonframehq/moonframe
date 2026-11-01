-- Gantry's tags: capitals in a thin box, plain or tinted by a tone (ACTIVE,
-- DEGRADED, a percentage). Gantry only.

type Props = {
	label: string,
	tone: string?,
	solid: boolean?,
}

local TONES: { [string]: string } = {
	success = "#22c55e",
	warning = "#eab308",
	danger = "#f97316",
	error = "#ef4444",
	info = "#3b82f6",
	muted = "#8a8a8a",
}

return component {
	title = "Tag",
	category = "Data",
	icon = "tag",
	description = "A small uppercase label in a thin square box, plain or tinted.",
	when = "Tags, states and percentage badges in Gantry. Elsewhere use `pill` or `ui.badge`.",
	props = {
		["label"] = "the text, shown in capitals",
		["tone"] = "success, warning, danger, error, info or muted; plain when unset",
		["solid"] = "a tint without a border (the percentage badges)",
	},
	styles = {
		[".tag"] = { padding = { 0, 6 }, border = true, border_color = "border", color = "#b5b5b5", background = "#1a1a1a" },
		[".tag-text"] = { size = "small" },
	},
	example = { label = "Active", tone = "success" },

	build = function(props: Props)
		local color = if props.tone then TONES[props.tone] else nil
		local style = if color
			then { color = color, background = color .. "26", border_color = if props.solid then color .. "00" else color .. "40" }
			else nil
		return ui.row {
			class = "tag",
			hug = true,
			shrink = false,
			align = "center",
			style = style,
			ui.text { string.upper(props.label), class = "tag-text", wrap = false },
		}
	end,
}
