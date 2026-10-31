-- A checkbox: a small rounded box, filled with a check when checked (or a
-- dash when indeterminate), with a label. The row is clickable.

type Props = {
	checked: boolean?,
	indeterminate: boolean?,
	label: string?,
	description: string?,
	msg: any,
	disabled: boolean?,
	to: string?,
}

return component {
	title = "Checkbox",
	category = "Forms",
	icon = "square-check",
	description = "A box with a check mark (or a dash when indeterminate) and a label; a click sends `msg`.",
	when = "An option that is on or off among others in a form or list, or a \"select all\" (indeterminate when some are). For a setting that applies at once use `switch`; for one of many use `radio-group`; for filter toggles use `chips`.",
	props = {
		["checked"] = "true when checked",
		["indeterminate"] = "shows a dash: some but not all of a group are checked",
		["label"] = "text beside the box",
		["description"] = "muted line under the label",
		["msg"] = "message a click sends (the mod flips its state)",
		["disabled"] = "greys it out and ignores clicks",
		["to"] = "mod to send `msg` to, if not the drawing one",
	},
	styles = {
		[".checkbox"] = { gap = 8, padding = { 2, 0 } },
		[".checkbox-disabled"] = { opacity = 0.5 },
		[".checkbox-box"] = {
			radius = 4,
			border = "all",
			border_color = "muted-foreground/50",
			background = "background",
			color = "accent-foreground",
			size = "small",
		},
		[".checkbox:hover"] = { color = "foreground" },
		[".checkbox-box:selected"] = { background = "accent", border_color = "accent" },
		[".checkbox-box-top"] = { margin = { top = 3 } },
		[".checkbox-description"] = { color = "muted-foreground", size = "small" },
	},
	example = {
		checked = true,
		label = "Restore tabs on launch",
		description = "Reopen the terminals and pages from last time.",
		msg = "toggle-restore",
	},
	usage = [[local checkbox = use "checkbox"

checkbox { label = "Restore tabs on launch", checked = state.restore, msg = "toggle-restore" }
checkbox { label = "All files", indeterminate = true, msg = "toggle-all" }]],

	build = function(props: Props)
		local marked = props.checked == true or props.indeterminate == true
		local box = ui.row {
			class = if props.description then { "checkbox-box", "checkbox-box-top" } else "checkbox-box",
			width = 16,
			height = 16,
			align = "center",
			justify = "center",
			selected = marked,
			marked and ui.icon { name = if props.indeterminate then "minus" else "check" },
		}
		local class = { "checkbox" }
		if props.disabled then
			table.insert(class, "checkbox-disabled")
		end
		return ui.row {
			class = class,
			align = if props.description then "start" else "center",
			msg = if props.disabled then nil else props.msg,
			to = props.to,
			box,
			(props.label or props.description) and ui.column {
				gap = "none",
				props.label and ui.text { props.label },
				props.description and ui.text { props.description, class = "checkbox-description" },
			},
		}
	end,
}
