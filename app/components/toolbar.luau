-- A row of small, quiet buttons: actions over a panel or list. Each sends
-- its `msg` to the mod drawing the toolbar, or runs its `command`.

type Button = {
	label: string,
	msg: any,
	command: any,
	variant: any,
}

type Props = {
	buttons: { Button },
}

return component {
	title = "Toolbar",
	category = "Controls",
	icon = "ellipsis",
	description = "Small ghost buttons in a row; each sends its msg to the mod drawing it, or runs a command.",
	when = "A row of small quiet buttons above a panel or list. For icon-only buttons use `icon-button`; for one-of-many choices use `segmented`.",
	props = {
		["buttons"] = "list of { label, msg or command, variant }",
	},
	example = { buttons = { { label = "New", msg = "new" }, { label = "Refresh", msg = "refresh" }, { label = "Remove", msg = "remove", variant = "danger" } } },

	build = function(props: Props)
		local buttons = {}
		for index, button in props.buttons do
			buttons[index] = ui.button {
				button.label,
				small = true,
				variant = button.variant or "ghost",
				msg = button.msg,
				command = button.command,
			}
		end
		return ui.row { gap = "small", align = "center", table.unpack(buttons) }
	end,
}
