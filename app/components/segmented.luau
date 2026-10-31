-- One-of-many choice in a rounded track: the selected segment is raised.
-- Like the Light / Dark / System appearance picker.

type Option = {
	id: string?,
	label: string?,
	icon: string?,
	tooltip: string?,
	msg: any,
}

type Props = {
	options: { Option },
	selected: string?,
	event: string?,
	small: boolean?,
	grow: boolean?,
	to: string?,
}

return component {
	title = "Segmented",
	category = "Controls",
	icon = "sun",
	description = "One-of-many choice as segments in a rounded track, the selected one raised; sends { select = id }.",
	when = "A setting with 2 to 4 short, mutually exclusive values (Light / Dark / System, All / Mine). For more or longer options use `dropdown`; for a vertical list with descriptions use `radio-group`; for switching documents or views use `tabs`; for actions rather than a choice use `button-group`.",
	props = {
		["options"] = "list of { id, label?, icon?, tooltip?, msg? (sent instead of the default) }; an icon without a label makes an icon-only segment",
		["selected"] = "id of the chosen option",
		["event"] = "key of the message a click sends, default \"select\": { select = id }",
		["small"] = "smaller, tighter segments",
		["grow"] = "fill the width, segments sharing it equally",
		["to"] = "mod to send messages to, if not the drawing one",
	},
	styles = {
		[".segmented"] = { background = "muted", border = "all", border_color = "border", radius = 9, padding = 2, gap = 2 },
		[".segmented-option"] = { padding = { 4, 12 }, gap = 6, radius = 7, color = "muted-foreground" },
		[".segmented-option:hover"] = { color = "foreground" },
		[".segmented-option:selected"] = {
			background = "background",
			color = "foreground",
			shadow = "0 1 2 #00000024",
		},
		[".segmented-small"] = { padding = { 2, 8 }, size = "small", radius = 6 },
	},
	example = {
		selected = "dark",
		options = {
			{ id = "light", label = "Light", icon = "sun" },
			{ id = "dark", label = "Dark", icon = "moon" },
			{ id = "system", label = "System", icon = "settings-2" },
		},
	},

	build = function(props: Props)
		local event = props.event or "select"
		local cells = {}
		for _, option in props.options do
			local id = option.id or option.label
			local class = { "segmented-option" }
			if props.small then
				table.insert(class, "segmented-small")
			end
			cells[#cells + 1] = ui.row {
				class = class,
				align = "center",
				justify = "center",
				grow = props.grow,
				selected = id == props.selected,
				tooltip = option.tooltip,
				msg = option.msg or { [event] = id },
				to = props.to,
				option.icon and ui.icon { name = option.icon :: any },
				option.label and ui.text { option.label },
			}
		end
		local track = ui.row { class = "segmented", align = "center", grow = props.grow, table.unpack(cells) }
		if props.grow then
			return track
		end
		-- Wrapped so a stretching parent does not widen the track.
		return ui.row { gap = "none", track }
	end,
}
