-- A field's value drawn as a quiet pill (a status dot, an avatar and a
-- name, label pills) that opens a popover: a search field over the options,
-- filtered by label as the user types, and a "Create" row when nothing
-- matches exactly. Up and down move the highlight, Enter picks it. The mod
-- keeps `open`, `selected`, `query` and `highlighted`.

local avatar = use "avatar"
local menu = use "menu"
local pill = use "pill"

type Option = {
	id: string,
	label: string?,
	detail: string?,
	icon: string?,
	avatar: string?,
	dot: string?,
	color: string?,
	disabled: boolean?,
}

type Props = {
	options: { any },
	selected: any,
	multiple: boolean?,
	open: boolean?,
	toggle: any,
	dismiss: any,
	event: string?,
	query: string?,
	search: any,
	id: string?,
	create: any,
	highlighted: string?,
	highlight: string?,
	keys: { [string]: any }?,
	placeholder: string?,
	icon: string?,
	empty: string?,
	width: number?,
	align: string?,
	trigger: any,
	to: string?,
}

-- The id the "Create" row is highlighted by.
local CREATE = "+create"

-- The preview's options: labels, each with a colour.
local EXAMPLE: { any } = {
	{ id = "bug", label = "Bug", color = "#e5484d" },
	{ id = "feature", label = "Feature", color = "#8e4ec6" },
	{ id = "design", label = "Design", color = "#3e63dd" },
	{ id = "docs", label = "Docs", color = "#12a594" },
	{ id = "perf", label = "Performance", color = "#f76b15" },
}

-- `msg` with the typed text, shaped as a text field sends it.
local function with_value(msg: any, value: string): any
	if type(msg) == "table" then
		local copy = table.clone(msg)
		copy.value = value
		return copy
	end
	return { msg = msg, value = value }
end

-- What leads a picked value in the trigger: its avatar, dot or icon.
local function face(option: Option, fallback: string?): any
	if option.avatar then
		return avatar { name = option.avatar, size = "small", shape = "circle" }
	elseif option.dot then
		-- In an icon's width, so labels line up with those after an icon.
		return ui.row {
			width = 16,
			justify = "center",
			shrink = false,
			ui.row { class = "picker-dot", width = 8, height = 8, style = { background = option.dot } },
		}
	end
	local icon = option.icon or fallback
	return icon and ui.icon { name = icon :: any, class = "picker-icon" }
end

-- The trigger's content for the picked options.
local function value(picked: { Option }, props: Props): { any }
	if #picked == 0 then
		return {
			props.icon and ui.icon { name = props.icon :: any, class = "picker-icon" },
			ui.text { props.placeholder or "Choose…", class = "picker-placeholder", truncate = true },
		}
	end
	local coloured, people = true, true
	for _, option in picked do
		coloured = coloured and option.color ~= nil
		people = people and option.avatar ~= nil
	end
	if coloured then
		local pills = {}
		for _, option in picked do
			table.insert(pills, pill { label = option.label or option.id, color = option.color })
		end
		return { ui.row { class = "picker-pills", align = "center", wrap = true, table.unpack(pills) } }
	end
	if #picked == 1 then
		local option = picked[1]
		return { face(option, props.icon), ui.text { option.label or option.id, truncate = true } }
	end
	if people then
		local faces = {}
		for _, option in picked do
			table.insert(faces, avatar { name = option.avatar, size = "small", shape = "circle" })
		end
		return { ui.row { class = "picker-faces", align = "center", table.unpack(faces) } }
	end
	return {
		face(picked[1], props.icon),
		ui.text { `{picked[1].label or picked[1].id} +{#picked - 1}`, truncate = true },
	}
end

return component {
	title = "Picker",
	category = "Controls",
	icon = "list-checks",
	description = "A field's value as a quiet pill that opens a searchable list of options (people, labels, statuses); one or several, with a Create row; picking sends { pick = id }.",
	when = "A property of a record set from many options: assignee, labels, project, status, priority, \"blocked by\", in a task's detail panel or a row. For a setting in a form use `dropdown`; for a filter over a list use `filter-dropdown`; for 2 to 4 short choices use `segmented`; for addresses use `recipient-field`.",
	props = {
		["options"] = "list of { id, label, detail? (muted, on the right), icon?, avatar? (a person's name: a small round `avatar`), dot? (a colour: a small dot, as for a status), color? (a coloured label: a dot in the list, a `pill` on the trigger), disabled? }",
		["selected"] = "the picked id, or with `multiple` a list of ids",
		["multiple"] = "several can be picked: each pick toggles one (the mod keeps the popover open) and the trigger shows them all (pills, avatars, or the first label and how many more)",
		["open"] = "true while the popover shows",
		["toggle"] = "message the trigger sends (open or close)",
		["dismiss"] = "message sent on a click outside or Escape",
		["event"] = "key of the message a pick sends, default \"pick\": { pick = id }",
		["query"] = "the search text; the list shows the options whose label holds it, case aside (set it to \"\" on close)",
		["search"] = "message sent on each edit of the search field, with `value` set to the text; no search field when unset",
		["id"] = "keeps the search text across renders (default \"picker\")",
		["create"] = "message of a \"Create “text”\" row under the list, shown while the query matches no label exactly; sent with `value` set to the query",
		["highlighted"] = "id of the keyboard-highlighted option (\"+create\" for the Create row); the first shown one when unset or filtered out",
		["highlight"] = "key of the message up and down send, default \"highlight\": { highlight = id } of the next row; Enter picks the highlighted row (or sends `create`)",
		["keys"] = "more keys while open, e.g. { tab = \"next-field\" }; they replace the built-in up, down and enter",
		["placeholder"] = "trigger text while nothing is picked (\"Add labels\"), muted after `icon`",
		["icon"] = "icon before the placeholder, and before a picked option that has no icon, avatar or dot of its own",
		["empty"] = "text while no option matches the query (default \"No matches\")",
		["width"] = "popover width in pixels (default 240)",
		["align"] = "\"start\" (default) or \"end\": which edge of the trigger the popover lines up with",
		["trigger"] = "an element to open it from instead of the pill (it sends `toggle` itself)",
		["to"] = "mod to send messages to, if not the drawing one",
	},
	styles = {
		[".picker-trigger"] = { padding = { 3, 6 }, gap = 6, radius = 6 },
		[".picker-trigger:hover"] = { background = "list-hover" },
		[".picker-trigger:selected"] = { background = "list-hover" },
		[".picker-placeholder"] = { color = "muted-foreground" },
		[".picker-icon"] = { color = "muted-foreground" },
		[".picker-dot"] = { radius = 999 },
		[".picker-pills"] = { gap = 4 },
		[".picker-faces"] = { gap = 2 },
		[".picker-popover"] = { padding = 0, radius = 10 },
		[".picker-search"] = { padding = { 4, 12 }, gap = 8, border = "bottom", border_color = "border" },
		[".picker-search-icon"] = { color = "muted-foreground" },
		[".picker-search-input"] = { background = "#00000000", border = "none", shadow = "none", padding = { 4, 0 } },
		[".picker-empty"] = { padding = { 10, 12 }, color = "muted-foreground", size = "small" },
	},
	example = {
		open = true,
		multiple = true,
		icon = "tag",
		placeholder = "Add labels",
		selected = { "bug", "design" },
		query = "",
		search = "search",
		create = "create",
		toggle = "toggle",
		dismiss = "close",
		options = EXAMPLE,
	},
	usage = [[local picker = use "picker"

picker {
	options = PEOPLE, selected = task.assignee, icon = "user", placeholder = "Assign",
	open = state.open == "assignee", toggle = { open = "assignee" }, dismiss = "close",
	query = state.query, search = "search", highlighted = state.highlighted,
}
-- update: msg.open opens (or closes) it; "close" closes it and sets
-- state.query = ""; msg.msg == "search" sets state.query = msg.value;
-- msg.highlight sets state.highlighted; msg.pick sets task.assignee and
-- closes it. With multiple = true, selected is a list and each pick
-- toggles one id; with create = "create", msg.value is the new label.]],

	build = function(props: Props)
		local event = props.event or "pick"
		local query = props.query or ""
		local needle = string.lower(query)

		local chosen: { [string]: boolean } = {}
		if props.multiple and type(props.selected) == "table" then
			for _, id in props.selected do
				chosen[id] = true
			end
		elseif type(props.selected) == "string" then
			chosen[props.selected] = true
		end

		-- The picked options, in the options' order, and the ones the query
		-- shows; a label equal to the query rules out the Create row.
		local picked: { Option } = {}
		local items: { any } = {}
		local order: { string } = {}
		local exact = false
		for _, entry in props.options do
			local option = entry :: Option
			local label = option.label or option.id
			if chosen[option.id] then
				table.insert(picked, option)
			end
			local lower = string.lower(label)
			exact = exact or lower == needle
			if needle == "" or string.find(lower, needle, 1, true) then
				table.insert(items, {
					id = option.id,
					label = label,
					detail = option.detail,
					icon = option.icon,
					avatar = option.avatar,
					dot = option.dot or option.color,
					disabled = option.disabled,
					checked = chosen[option.id] == true,
				})
				if not option.disabled then
					table.insert(order, option.id)
				end
			end
		end
		local matched = #items > 0
		local creating = props.create ~= nil and string.match(query, "%S") ~= nil and not exact
		if creating then
			if matched then
				table.insert(items, "separator")
			end
			table.insert(items, {
				id = CREATE,
				label = `Create “{query}”`,
				icon = "plus",
				msg = with_value(props.create, query),
			})
			table.insert(order, CREATE)
		end

		-- The highlighted row, the first one when the mod's is not shown.
		local at = (props.highlighted and table.find(order, props.highlighted)) or 1
		local current = order[at]
		local highlight = props.highlight or "highlight"
		local keys: { [string]: any } = { escape = props.dismiss }
		if #order > 0 then
			keys.up = { [highlight] = order[math.max(at - 1, 1)] }
			keys.down = { [highlight] = order[math.min(at + 1, #order)] }
			keys.enter = if current == CREATE then with_value(props.create, query) else { [event] = current }
		end
		for key, msg in pairs(props.keys or {}) do
			keys[key] = msg
		end
		-- The field takes Enter as its submit, and the rest as its keys.
		local field_keys = table.clone(keys)
		field_keys.enter = nil

		local trigger = props.trigger
			or ui.row {
				class = "picker-trigger",
				align = "center",
				hug = true,
				selected = props.open == true,
				msg = props.toggle,
				to = props.to,
				table.unpack(value(picked, props)),
			}

		return ui.popover {
			class = "picker-popover",
			open = props.open == true,
			dismiss = props.dismiss,
			to = props.to,
			side = "bottom",
			align = (props.align or "start") :: any,
			gap = 4,
			width = props.width or 240,
			keys = keys,
			content = ui.column {
				gap = "none",
				props.search ~= nil and ui.row {
					class = "picker-search",
					align = "center",
					ui.icon { name = "search", class = "picker-search-icon" },
					ui.input {
						id = props.id or "picker",
						value = query,
						placeholder = "Search…",
						change = props.search,
						submit = keys.enter,
						keys = field_keys,
						focus = true,
						to = props.to,
						grow = true,
						class = "picker-search-input",
					},
				},
				if #items > 0
					then ui.column {
						max_height = 300,
						scroll = true,
						menu { items = items, highlighted = current, event = event, to = props.to },
					}
					else ui.row { class = "picker-empty", ui.text { props.empty or "No matches" } },
			},
			trigger,
		}
	end,
}
