-- Wraps any content: a right-click on it sends `context`, and while `open`
-- a `menu` floats beside it. A click outside or Escape sends `dismiss`;
-- arrow keys and Enter send the mod's `keys` messages.

local menu = use "menu"

type Props = {
	open: boolean?,
	context: any,
	dismiss: any,
	items: { any },
	highlighted: any,
	event: string?,
	keys: { [string]: any }?,
	width: number?,
	side: string?,
	align: string?,
	to: string?,
	[number]: any,
}

-- The preview's items, typed loosely: they mix kinds of entries.
local EXAMPLE: { any } = {
	{ id = "open", label = "Open", icon = "external-link", shortcut = "enter" },
	{ id = "copy-path", label = "Copy path", icon = "copy", shortcut = "cmd-alt-c" },
	{ id = "reveal", label = "Reveal in Finder", icon = "folder-open" },
	"separator",
	{ id = "delete", label = "Move to Trash", icon = "trash-2", shortcut = "cmd-backspace", danger = true },
}

return component {
	title = "Context Menu",
	category = "Overlays",
	icon = "ellipsis-vertical",
	description = "Any content that opens a menu beside it on a right-click; picking sends { pick = id }.",
	when = "Right-click actions on a row, tab, tile or file. It anchors the menu to the wrapped content, not the pointer. For a menu opened by a button use `dropdown`; for the menu rows alone use `menu`. On `tabs`, use its `context` (each tab already sends { tab_menu = id }) and put this around the strip, or a `ui.popover` with `menu`.",
	props = {
		["[children]"] = "the content that takes the right-click (the anchor)",
		["open"] = "true while the menu shows",
		["context"] = "message a right-click on the content sends, to open the menu",
		["dismiss"] = "message sent on a click outside or Escape",
		["items"] = "`menu` items: { id, label, icon?, shortcut?, checked?, danger?, disabled?, detail? }, \"separator\", { heading = ... }",
		["highlighted"] = "keyboard-highlighted item (id or index)",
		["event"] = "key of the message a pick sends, default \"pick\": { pick = id }",
		["keys"] = "keys while open, e.g. { up = \"menu-up\", down = \"menu-down\", enter = \"menu-choose\" }; escape sends `dismiss`",
		["width"] = "menu width in pixels (default 220)",
		["side"] = "\"bottom\" (default) or \"top\"",
		["align"] = "\"start\" (default) or \"end\"",
		["to"] = "mod to send right-click and pick messages to, if not the drawing one",
	},
	styles = {
		[".context-menu"] = { gap = 0 },
		[".context-menu-popover"] = { padding = 0, radius = 10 },
	},
	example = {
		open = true,
		context = "open-menu",
		dismiss = "close-menu",
		highlighted = 2,
		items = EXAMPLE,
	},
	usage = [[local context_menu = use "context-menu"

context_menu {
	open = state.menu_open, context = "open-menu", dismiss = "close-menu",
	keys = { up = "menu-up", down = "menu-down", enter = "menu-choose" },
	highlighted = state.menu_index,
	items = {
		{ id = "open", label = "Open", icon = "external-link" },
		{ id = "copy-path", label = "Copy path", icon = "copy" },
		"separator",
		{ id = "delete", label = "Move to Trash", icon = "trash-2", danger = true },
	},
	ui.item { "README.md", detail = "~/code/app" },
}
-- update: "open-menu" sets state.menu_open; msg.pick runs the action and
-- closes it; "close-menu" closes it.]],

	build = function(props: Props)
		local keys = { escape = props.dismiss }
		for key, msg in pairs(props.keys or {}) do
			keys[key] = msg
		end
		local children = {}
		for _, child in ipairs(props) do
			children[#children + 1] = child
		end
		if #children == 0 then
			children[1] = ui.item { "README.md", detail = "Right-click for actions" }
		end

		return ui.popover {
			class = "context-menu-popover",
			open = props.open == true,
			dismiss = props.dismiss,
			side = (props.side or "bottom") :: any,
			align = (props.align or "start") :: any,
			gap = 4,
			width = props.width or 220,
			keys = keys,
			content = menu {
				items = props.items,
				highlighted = props.highlighted,
				event = props.event,
				to = props.to,
			},
			ui.column {
				class = "context-menu",
				context = props.context,
				to = props.to,
				table.unpack(children),
			},
		}
	end,
}
