-- A filter as a chip over a table or list: an icon, what it filters to
-- ("Any author" until one is picked) and a chevron; it opens a menu of
-- options. With `multiple`, several can be on at once and each pick
-- toggles one. A chip with a filter on takes the accent, and `clear`
-- adds an × that resets it. The mod keeps `open` and `selected`.

local menu = use "menu"

type Option = { id: string, label: string, icon: string?, detail: string? }

type Props = {
	icon: string?,
	label: string?,
	placeholder: string?,
	options: { any },
	selected: any,
	multiple: boolean?,
	any: string?,
	open: boolean?,
	toggle: any,
	dismiss: any,
	event: string?,
	clear: any,
	width: number?,
	plain: boolean?,
	to: string?,
}

-- The preview's options, typed loosely: they mix kinds of entries.
local EXAMPLE: { any } = {
	{ id = "orders", label = "Orders", icon = "file-text" },
	{ id = "products", label = "Products", icon = "layers" },
	{ id = "customers", label = "Customers", icon = "user" },
	"separator",
	{ id = "apps", label = "Apps", icon = "plug" },
}

return component {
	title = "Filter Dropdown",
	category = "Controls",
	icon = "funnel",
	description = "A filter chip with an icon, its value (\"Any author\") and a chevron that opens a menu; one or several options, an accent look when on, and an optional × to clear.",
	when = "Filters in a row over a table or list (activity type, author, date range), each narrowing it to one or a few values. For toggles that are each on or off use `chips`; for a setting rather than a filter use `dropdown`; for a free-text filter use `search-field`.",
	props = {
		["icon"] = "icon before the value",
		["label"] = "muted text before the value (\"Author\")",
		["placeholder"] = "text while nothing is picked (\"Any author\")",
		["options"] = "`menu` items: { id, label, icon?, detail? }, \"separator\", { heading = ... }",
		["selected"] = "the picked id, or with `multiple` a list of ids",
		["multiple"] = "several options can be on; a pick toggles one and the menu stays open",
		["any"] = "label of an option at the top that clears the filter (sends { [event] = \"\" }); none when unset",
		["open"] = "true while the menu shows",
		["toggle"] = "message the chip sends (open or close)",
		["dismiss"] = "message sent on a click outside or Escape",
		["event"] = "key of the message a pick sends, default \"pick\": { pick = id }",
		["clear"] = "message for an × on the chip while a filter is on; no × when unset",
		["width"] = "menu width in pixels (default 220)",
		["plain"] = "keep the resting look when a filter is on (no accent)",
		["to"] = "mod to send messages to, if not the drawing one",
	},
	styles = {
		[".filter-dropdown"] = {
			padding = { 0, 10 },
			gap = 6,
			radius = 7,
			border = "all",
			border_color = "border",
			background = "background",
			weight = "medium",
		},
		[".filter-dropdown:hover"] = { background = "list-hover" },
		[".filter-dropdown:selected"] = { background = "list-hover" },
		[".filter-dropdown-on"] = { border_color = "accent/50", background = "accent/8", color = "accent" },
		[".filter-dropdown-on:hover"] = { background = "accent/12" },
		[".filter-dropdown-icon"] = { size = "small" },
		[".filter-dropdown-label"] = { color = "muted-foreground", weight = "normal" },
		[".filter-dropdown-chevron"] = { size = "small", color = "muted-foreground" },
		[".filter-dropdown-clear"] = { radius = 4, padding = 1, color = "muted-foreground", size = "small" },
		[".filter-dropdown-clear:hover"] = { background = "muted-foreground/20", color = "foreground" },
		[".filter-dropdown-popover"] = { padding = 0, radius = 10 },
	},
	example = {
		open = true,
		icon = "square-check",
		placeholder = "Any activities",
		any = "Any activities",
		multiple = true,
		selected = { "orders", "products" },
		options = EXAMPLE,
		toggle = "toggle",
		dismiss = "close",
	},
	usage = [[local filter_dropdown = use "filter-dropdown"

filter_dropdown {
	icon = "user", placeholder = "Any author", any = "Any author",
	open = state.open == "author", toggle = { open = "author" }, dismiss = "close",
	event = "author", selected = state.author,
	options = { { id = "nadia", label = "Nadia Okafor" }, { id = "theo", label = "Theo Lind" } },
}
-- update: msg.author sets state.author ("" clears it) and closes the menu.
-- With multiple = true, selected is a list and each pick toggles one id.]],

	build = function(props: Props)
		local event = props.event or "pick"
		local picked: { [string]: boolean } = {}
		local count = 0
		if props.multiple and type(props.selected) == "table" then
			for _, id in props.selected do
				picked[id] = true
				count += 1
			end
		elseif type(props.selected) == "string" and props.selected ~= "" then
			picked[props.selected] = true
			count = 1
		end

		local items: { any } = {}
		local labels = {}
		if props.any then
			table.insert(items, { id = "", label = props.any, checked = count == 0, msg = { [event] = "" } })
			table.insert(items, "separator")
		end
		for _, option in props.options do
			if type(option) == "table" and option.id then
				local copy = table.clone(option)
				copy.checked = picked[option.id] == true
				table.insert(items, copy)
				if picked[option.id] then
					table.insert(labels, option.label or option.id)
				end
			else
				table.insert(items, option)
			end
		end

		local value = props.placeholder or "Any"
		if #labels == 1 then
			value = labels[1]
		elseif #labels > 1 then
			value = `{labels[1]} +{#labels - 1}`
		end
		local on = count > 0 and not props.plain

		local chip = ui.row {
			class = if on then { "filter-dropdown", "filter-dropdown-on" } else "filter-dropdown",
			height = 32,
			align = "center",
			shrink = false,
			selected = props.open == true,
			msg = props.toggle,
			to = props.to,
			props.icon and ui.icon { name = props.icon :: any, class = "filter-dropdown-icon" },
			props.label and ui.text { props.label, class = "filter-dropdown-label" },
			ui.text { value, truncate = true },
			if count > 0 and props.clear ~= nil
				then ui.row {
					class = "filter-dropdown-clear",
					msg = props.clear,
					to = props.to,
					tooltip = "Clear",
					ui.icon "close",
				}
				else ui.icon { name = "chevron-down", class = "filter-dropdown-chevron" },
		}

		return ui.popover {
			class = "filter-dropdown-popover",
			open = props.open == true,
			dismiss = props.dismiss,
			to = props.to,
			side = "bottom",
			align = "start",
			gap = 4,
			width = props.width or 220,
			keys = { escape = props.dismiss },
			content = menu { items = items, event = event, to = props.to },
			chip,
		}
	end,
}
