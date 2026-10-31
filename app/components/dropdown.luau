-- A button showing the current value and a chevron; clicking it sends
-- `toggle`, and while `open` a `menu` of options floats under it. The mod
-- keeps `open` and `selected`.

local menu = use "menu"

type Props = {
	open: boolean?,
	label: string?,
	value: string?,
	icon: string?,
	options: { any },
	selected: string?,
	toggle: any,
	dismiss: any,
	event: string?,
	highlighted: any,
	keys: { [string]: any }?,
	width: number?,
	align: string?,
	side: string?,
	placeholder: string?,
	disabled: boolean?,
	to: string?,
}

-- The preview's options, typed loosely: they mix kinds of entries.
local EXAMPLE: { any } = {
	{ heading = "Installed" },
	{ id = "zsh", label = "zsh", detail = "/bin/zsh" },
	{ id = "fish", label = "fish", detail = "/opt/homebrew/bin/fish" },
	{ id = "bash", label = "bash", detail = "/bin/bash" },
	"separator",
	{ id = "custom", label = "Custom…", icon = "settings", shortcut = "cmd-," },
}

return component {
	title = "Dropdown",
	category = "Controls",
	icon = "chevrons-up-down",
	description = "A button with the current value and a chevron that opens a menu of options under it; picking sends { pick = id }.",
	when = "Pick one of many options that do not fit a `segmented` control (more than 3 or 4, or long labels), or a button that opens a menu of actions. For 2 to 4 short choices use `segmented`; for a menu on right-click use `context-menu`; for the menu rows alone use `menu`.",
	props = {
		["open"] = "true while the menu shows",
		["label"] = "muted text before the value (\"Shell\")",
		["value"] = "text on the button; defaults to the selected option's label",
		["icon"] = "icon before the text",
		["options"] = "`menu` items: { id, label, icon?, shortcut?, detail?, danger?, disabled? }, \"separator\", { heading = ... }",
		["selected"] = "id of the current option; it gets a check",
		["toggle"] = "message the button sends (open or close)",
		["dismiss"] = "message sent on a click outside or Escape",
		["event"] = "key of the message a pick sends, default \"pick\": { pick = id }",
		["highlighted"] = "keyboard-highlighted option (id or index), passed to `menu`",
		["keys"] = "more keys while open, e.g. { up = \"prev\", down = \"next\", enter = \"choose\" }; escape sends `dismiss`",
		["width"] = "menu width in pixels (default 220)",
		["align"] = "\"start\" (default) or \"end\": which edge of the button the menu lines up with",
		["side"] = "\"bottom\" (default) or \"top\"",
		["placeholder"] = "button text when nothing is selected",
		["disabled"] = "greys the button out and ignores clicks",
		["to"] = "mod to send messages to, if not the drawing one",
	},
	styles = {
		[".dropdown-trigger"] = {
			padding = { 4, 8, 4, 10 },
			gap = 6,
			radius = 7,
			border = "all",
			border_color = "border",
			background = "background",
		},
		[".dropdown-trigger:hover"] = { background = "list-hover" },
		[".dropdown-trigger:selected"] = { border_color = "accent/60", background = "list-hover" },
		[".dropdown-disabled"] = { opacity = 0.5 },
		[".dropdown-label"] = { color = "muted-foreground" },
		[".dropdown-icon"] = { color = "muted-foreground" },
		[".dropdown-chevron"] = { color = "muted-foreground", size = "small" },
		[".dropdown-popover"] = { padding = 0, radius = 10 },
	},
	example = {
		open = true,
		label = "Shell",
		icon = "terminal",
		selected = "zsh",
		toggle = "toggle-shell",
		dismiss = "close-shell",
		options = EXAMPLE,
	},
	usage = [[local dropdown = use "dropdown"

dropdown {
	label = "Shell", icon = "terminal",
	open = state.shell_open, toggle = "toggle-shell", dismiss = "close-shell",
	selected = state.shell,
	options = {
		{ id = "zsh", label = "zsh", detail = "/bin/zsh" },
		{ id = "fish", label = "fish" },
		"separator",
		{ id = "custom", label = "Custom…", icon = "settings" },
	},
}
-- update: msg == "toggle-shell" flips state.shell_open; msg.pick sets
-- state.shell and closes it; "close-shell" closes it.]],

	build = function(props: Props)
		local value = props.value
		local items = {}
		for index, option in props.options do
			if type(option) == "table" and not option.heading then
				local id = option.id or option.label
				local copy = table.clone(option)
				copy.checked = id == props.selected
				if value == nil and id == props.selected then
					value = option.label
				end
				items[index] = copy
			else
				items[index] = option
			end
		end

		local keys = { escape = props.dismiss }
		for key, msg in pairs(props.keys or {}) do
			keys[key] = msg
		end

		local class = { "dropdown-trigger" }
		if props.disabled then
			table.insert(class, "dropdown-disabled")
		end
		local trigger = ui.row {
			class = class,
			align = "center",
			selected = props.open == true,
			msg = if props.disabled then nil else props.toggle,
			to = props.to,
			props.icon and ui.icon { name = props.icon :: any, class = "dropdown-icon" },
			props.label and ui.text { props.label, class = "dropdown-label" },
			ui.text { value or props.placeholder or "Choose…", truncate = true },
			ui.icon { name = "chevron-down", class = "dropdown-chevron" },
		}

		return ui.popover {
			class = "dropdown-popover",
			open = props.open == true and not props.disabled,
			dismiss = props.dismiss,
			side = (props.side or "bottom") :: any,
			align = (props.align or "start") :: any,
			gap = 4,
			width = props.width or 220,
			keys = keys,
			content = menu {
				items = items,
				highlighted = props.highlighted,
				event = props.event,
				to = props.to,
			},
			trigger,
		}
	end,
}
