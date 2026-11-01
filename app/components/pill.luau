-- A small tinted label: a tag on a task ("Work"), a count in a sentence
-- ("2 events"), with an optional icon, in a tone or any colour.

type Props = {
	label: string,
	icon: string?,
	tone: string?,
	color: string?,
	size: string?,
	msg: any,
	to: string?,
	tooltip: string?,
}

-- Tones to the colours their tint and text use.
local TONES = {
	neutral = "muted-foreground",
	accent = "accent",
	success = "success",
	warning = "warning",
	danger = "danger",
}

return component {
	title = "Pill",
	category = "Data",
	icon = "tag",
	description = "A small tinted label with an optional icon, in a tone or any colour; clickable when given `msg`.",
	when = "A tag on a row (\"Work\", \"Health\") or a count inside a sentence (\"2 events\"), where colour carries meaning. For several read-only labels use `tags`; for filters the user toggles use `chips`; for a plain status word use `ui.badge`.",
	props = {
		["label"] = "the text",
		["icon"] = "icon before the text",
		["tone"] = "neutral (default), accent, success, warning or danger",
		["color"] = "any colour instead of a tone, such as \"#3b82f6\"; the tint is its 14%",
		["size"] = "\"small\" (default, for tags) or \"normal\" (in a sentence)",
		["msg"] = "message a click sends",
		["to"] = "mod to send `msg` to, if not the drawing one",
		["tooltip"] = "text under the pointer",
	},
	styles = {
		[".pill"] = { radius = 7, padding = { 2, 7 }, gap = 4 },
		[".pill-normal"] = { radius = 9, padding = { 3, 9 }, gap = 5 },
		[".pill-label"] = { weight = "medium" },
	},
	example = { label = "2 events", icon = "calendar", tone = "danger", size = "normal" },
	usage = [[local pill = use "pill"

pill { label = "Work", color = "#3b82f6" }
pill { label = "2 events", icon = "calendar", tone = "danger", size = "normal", msg = "show-events" }]],

	build = function(props: Props)
		local color = props.color or TONES[props.tone or "neutral"] or "muted-foreground"
		local normal = props.size == "normal"
		local size = (if normal then "normal" else "small") :: any
		return ui.row {
			class = if normal then { "pill", "pill-normal" } else "pill",
			align = "center",
			hug = true,
			shrink = false,
			msg = props.msg,
			to = props.to,
			tooltip = props.tooltip,
			style = { background = `{color}/14`, color = color },
			props.icon and ui.icon { name = props.icon :: any, size = size },
			ui.text { props.label, class = "pill-label", size = size },
		}
	end,
}
