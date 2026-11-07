-- A quiet icon, or a chip with a word, that opens a menu. `dropdown` with no
-- text and no chevron, for headers and the composer's footer.

local menu = use "menu"

type Props = {
	icon: string?,
	label: string?,
	class: string?,
	open: boolean?,
	toggle: any,
	dismiss: any,
	items: { any },
	side: string?,
	align: string?,
	width: number?,
	tooltip: string?,
}

return component {
	title = "Menu Button",
	category = "Controls",
	icon = "ellipsis",
	description = "An icon or short label that opens a menu; picking sends { pick = id }.",
	when = "A small menu in a header or footer. With a value and chevron use `dropdown`.",
	props = {
		icon = "the icon",
		label = "or a word",
		class = "class of the button (default quiet-button)",
		open = "true while the menu shows",
		toggle = "message the button sends",
		dismiss = "message sent on a click outside or Escape",
		items = "`menu` items",
		side = "top or bottom (default)",
		align = "start (default) or end",
		width = "menu width in pixels (default 220)",
		tooltip = "text under the pointer",
	},
	example = { icon = "ellipsis", items = { { id = "one", label = "One" } } },
	build = function(props: Props)
		return ui.popover {
			open = props.open == true,
			dismiss = props.dismiss,
			side = (props.side or "bottom") :: any,
			align = (props.align or "start") :: any,
			width = props.width or 220,
			content = menu { items = props.items },
			ui.row {
				class = props.class or "quiet-button",
				align = "center",
				gap = "small",
				msg = props.toggle,
				tooltip = props.tooltip,
				props.icon and ui.icon { props.icon :: any },
				props.label and ui.text { props.label, wrap = false },
			},
		}
	end,
}
