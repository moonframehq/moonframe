-- Radio buttons for one of many options, stacked or in a row. Each option
-- may have a description under its label.

type Option = {
	id: string?,
	label: string,
	description: string?,
	disabled: boolean?,
	msg: any,
}

type Props = {
	options: { Option },
	selected: string?,
	horizontal: boolean?,
	event: string?,
	to: string?,
}

return component {
	title = "Radio Group",
	category = "Forms",
	icon = "circle-dot",
	description = "Radio buttons, vertical or horizontal, each with a label and optional description; sends { select = id }.",
	when = "Pick one of a few options that need a description or room to read (a mode, a policy). For 2 to 4 short values use `segmented`; for many values use `dropdown`; for options with a picture use `choice-cards`; for independent on/off options use `checkbox`.",
	props = {
		["options"] = "list of { id, label, description?, disabled?, msg? (sent instead of the default) }",
		["selected"] = "id of the chosen option",
		["horizontal"] = "lay the options out in a row",
		["event"] = "key of the message a click sends, default \"select\": { select = id }",
		["to"] = "mod to send messages to, if not the drawing one",
	},
	styles = {
		[".radio-group"] = { gap = 8 },
		[".radio-group-horizontal"] = { gap = 16 },
		[".radio-group-option"] = { gap = 8 },
		[".radio-group-disabled"] = { opacity = 0.5 },
		[".radio-group-circle"] = {
			radius = 999,
			border = "all",
			border_color = "muted-foreground/50",
			background = "background",
		},
		[".radio-group-option:hover"] = { color = "foreground" },
		[".radio-group-circle:selected"] = { border_color = "accent" },
		[".radio-group-dot"] = { radius = 999, background = "accent" },
		[".radio-group-description"] = { color = "muted-foreground", size = "small" },
	},
	example = {
		selected = "worktree",
		options = {
			{ id = "worktree", label = "New worktree", description = "Each agent gets its own checkout and branch." },
			{ id = "current", label = "Current folder", description = "Agents share the working tree you are in." },
			{ id = "remote", label = "Cloud machine", description = "Run on a remote box; needs a connected device.", disabled = true },
		},
	},

	build = function(props: Props)
		local event = props.event or "select"
		local cells = {}
		for _, option in props.options do
			local id = option.id or option.label
			local chosen = id == props.selected
			local class = { "radio-group-option" }
			if option.disabled then
				table.insert(class, "radio-group-disabled")
			end
			cells[#cells + 1] = ui.row {
				class = class,
				align = if option.description then "start" else "center",
				msg = if option.disabled then nil else (option.msg or { [event] = id }),
				to = props.to,
				ui.column {
					style = { padding = { top = if option.description then 2 else 0 } },
					ui.row {
						class = "radio-group-circle",
						width = 16,
						height = 16,
						align = "center",
						justify = "center",
						selected = chosen,
						chosen and ui.row { class = "radio-group-dot", width = 8, height = 8 },
					},
				},
				ui.column {
					gap = "none",
					ui.text { option.label },
					option.description and ui.text { option.description, class = "radio-group-description" },
				},
			}
		end
		local class = { "radio-group" }
		if props.horizontal then
			table.insert(class, "radio-group-horizontal")
		end
		if props.horizontal then
			return ui.row { class = class, table.unpack(cells) }
		end
		return ui.column { class = class, table.unpack(cells) }
	end,
}
