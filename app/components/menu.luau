-- The inside of a menu: rows with an icon, a label, a check on the current
-- one, a detail and a shortcut, separated by lines and headings. Dropdowns
-- and context menus put it in a popover; it draws no popover itself, except
-- the submenu of the row named by `expanded`, beside that row.

local kbd = use "kbd"

type Item = {
	id: string?,
	label: string?,
	icon: string?,
	shortcut: any,
	checked: boolean?,
	danger: boolean?,
	disabled: boolean?,
	detail: string?,
	heading: string?,
	items: { any }?,
	msg: any,
}

type Props = {
	items: { any },
	highlighted: any,
	event: string?,
	expanded: any,
	expand: string?,
	submenu_highlighted: any,
	submenu_width: number?,
	to: string?,
}

-- The preview's items, typed loosely: they mix kinds of entries.
local EXAMPLE: { any } = {
	{ heading = "Tab" },
	{ id = "rename", label = "Rename…", icon = "pencil", shortcut = "cmd-r" },
	{ id = "duplicate", label = "Duplicate", icon = "copy", shortcut = "cmd-d" },
	{ id = "pin", label = "Pin tab", icon = "pin", checked = true },
	{ id = "split", label = "Split right", icon = "columns-2", detail = "2 panes" },
	{
		id = "move",
		label = "Move to",
		icon = "folder-open",
		items = {
			{ id = "move-left", label = "Left pane" },
			{ id = "move-right", label = "Right pane" },
			{ id = "move-window", label = "New window" },
		},
	},
	{ id = "reveal", label = "Reveal in Finder", icon = "folder-open", disabled = true },
	"separator",
	{ id = "close", label = "Close tab", icon = "close", shortcut = "cmd-w", danger = true },
}

-- The rows, and the open submenu's, which is drawn by this same function.
local function draw(props: Props)
	local event = props.event or "pick"
	local expand = props.expand or "expand"
	-- A check column only when some item can be checked, so labels line up.
	local checks = false
	for _, item in props.items do
		if type(item) == "table" and item.checked ~= nil then
			checks = true
		end
	end

	local rows = {}
	for index, entry in props.items do
		if entry == "separator" then
			rows[#rows + 1] = ui.column { class = "menu-separator", ui.divider() }
			continue
		end
		local item = entry :: Item
		if item.heading then
			rows[#rows + 1] = ui.row { class = "menu-heading", ui.text { item.heading } }
			continue
		end
		local id = item.id or item.label
		local class = { "menu-item" }
		if item.danger then
			table.insert(class, "menu-item-danger")
		end
		if item.disabled then
			table.insert(class, "menu-item-disabled")
		end
		local highlighted = props.highlighted ~= nil and (props.highlighted == id or props.highlighted == index)
		local submenu = item.items
		local open = submenu ~= nil and props.expanded ~= nil and props.expanded == id
		local default = if submenu then { [expand] = id } else { [event] = id }
		local row = ui.row {
			class = class,
			align = "center",
			grow = true,
			selected = (highlighted or open) and not item.disabled,
			msg = if item.disabled then nil else (item.msg or default),
			to = props.to,
			checks and ui.row {
				width = 14,
				justify = "center",
				item.checked and ui.icon { name = "check", class = "menu-check" },
			},
			item.icon and ui.icon {
				name = item.icon :: any,
				class = if item.danger then "menu-item-danger-icon" else "menu-icon",
			},
			ui.row { grow = true, ui.text { item.label or id or "", truncate = true } },
			item.detail and ui.text { item.detail, class = "menu-detail" },
			item.shortcut and kbd { keys = item.shortcut, small = true, plain = true },
			submenu and ui.icon { name = "chevron-right", size = "small", class = "menu-chevron" },
		}
		-- The open submenu floats beside its row, its picks sent as the
		-- menu's own.
		rows[#rows + 1] = if open and submenu
			then ui.popover {
				class = "menu-submenu",
				open = true,
				side = "right",
				align = "start",
				gap = 8,
				width = props.submenu_width or 200,
				content = draw {
					items = submenu,
					highlighted = props.submenu_highlighted,
					event = event,
					to = props.to,
				},
				row,
			}
			else row
	end
	return ui.column { class = "menu", table.unpack(rows) }
end

return component {
	title = "Menu",
	category = "Overlays",
	icon = "menu",
	description = "Menu rows (icon, label, check, detail, shortcut, danger) with separators and headings; a click sends { pick = id }.",
	when = "The content of any menu: put it in a `ui.popover` or `ui.overlay` yourself, or use `dropdown` (a button that opens one) or `context-menu` (opens on right-click), which already do. For a selectable sidebar list use `list`; for a command palette with a search field use `command-palette`.",
	props = {
		["items"] = "list of { id, label, icon?, shortcut? (kbd keys, \"cmd-s\"), checked? (true/false shows the check column), danger?, disabled?, detail? (muted text on the right), items? (a submenu: the same kinds of entries, one level deep), msg? (sent instead of the default) }, the string \"separator\", or { heading = \"Text\" }",
		["expanded"] = "id of the item whose submenu shows beside it; a click on an item with `items` sends { expand = id } (the mod sets this, or clears it on a second click)",
		["expand"] = "key of the message an item with a submenu sends, default \"expand\": { expand = id }",
		["submenu_highlighted"] = "the keyboard-highlighted row of the open submenu: an id or an index",
		["submenu_width"] = "the submenu's width in pixels (default 200)",
		["highlighted"] = "the keyboard-highlighted row: an item id, or its index in `items`",
		["event"] = "key of the message a click sends, default \"pick\": { pick = id }",
		["to"] = "mod to send messages to, if not the drawing one",
	},
	styles = {
		[".menu"] = { gap = 1, padding = 4 },
		[".menu-item"] = { padding = { 5, 8 }, radius = 6, gap = 8 },
		[".menu-item:hover"] = { background = "list-hover" },
		[".menu-item:selected"] = { background = "accent/15", color = "accent" },
		[".menu-item-danger"] = { color = "danger" },
		[".menu-item-danger:hover"] = { background = "danger/12" },
		[".menu-item-disabled"] = { opacity = 0.45 },
		[".menu-icon"] = { color = "muted-foreground" },
		[".menu-item-danger-icon"] = { color = "danger" },
		[".menu-check"] = { color = "accent" },
		[".menu-chevron"] = { color = "muted-foreground" },
		[".menu-submenu"] = { padding = 0, radius = 10 },
		[".menu-detail"] = { color = "muted-foreground", size = "small" },
		[".menu-separator"] = { padding = { 4, 2 } },
		[".menu-heading"] = { padding = { 6, 8, 2, 8 }, color = "muted-foreground", size = "small", weight = "medium" },
	},
	example = {
		highlighted = "rename",
		expanded = "move",
		submenu_highlighted = "move-right",
		items = EXAMPLE,
	},

	build = draw,
}
