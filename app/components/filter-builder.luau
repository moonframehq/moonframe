-- The body of a list's Filter popover: whether all or any conditions must
-- hold, one row per condition (a field, an operator and a value, each a
-- dropdown, or a text field for a value without options, and × to remove
-- it), "Add filter", and Clear all and Done under them. The mod keeps the
-- conditions and the open dropdown, and gets fixed messages: { open },
-- { set }, { remove }, { match }, "add", "clear" and "done".

local dropdown = use "dropdown"
local icon_button = use "icon-button"
local segmented = use "segmented"

type Condition = { id: string, field: string?, operator: string?, value: string? }

type Field = {
	id: string,
	label: string?,
	icon: string?,
	operators: { any }?,
	values: { any }?,
}

type Props = {
	match: string?,
	conditions: { Condition }?,
	fields: { Field },
	open: string?,
	to: string?,
}

-- The preview's fields, typed loosely: options mix kinds of entries.
local EXAMPLE_FIELDS: { any } = {
	{
		id = "status",
		label = "Status",
		icon = "circle-dashed",
		operators = { "is", "is not" },
		values = {
			{ id = "todo", label = "Todo", dot = "#b9bbc6" },
			{ id = "progress", label = "In Progress", dot = "#f5a524" },
			{ id = "done", label = "Done", dot = "#30a46c" },
		},
	},
	{
		id = "assignee",
		label = "Assignee",
		icon = "user",
		operators = { "is", "is not" },
		values = {
			{ id = "sarah", label = "Sarah Chen", avatar = "Sarah Chen" },
			{ id = "marcus", label = "Marcus Webb", avatar = "Marcus Webb" },
		},
	},
	{ id = "title", label = "Title", icon = "file-text", operators = { "contains", "does not contain" } },
}

-- An operator given as a word ("is") or as { id, label }.
local function operator(entry: any): (string, string)
	if type(entry) == "table" then
		return entry.id, entry.label or entry.id
	end
	return entry, entry
end

-- Options of a dropdown that each set one key of a condition.
local function setters(options: { any }, id: string, key: string): { any }
	local items = {}
	for _, option in options do
		if type(option) == "table" and option.id then
			local copy = table.clone(option)
			copy.msg = { set = { id = id, key = key, value = option.id } }
			table.insert(items, copy)
		else
			table.insert(items, option)
		end
	end
	return items
end

return component {
	title = "Filter Builder",
	category = "Forms",
	icon = "funnel",
	description = "The body of a Filter popover: match all or any, rows of field, operator and value with ×, Add filter, and Clear all and Done (they send \"add\", \"clear\" and \"done\").",
	when = "Filtering a list or board by conditions the user builds (status is Done, assignee is not Sarah), inside a `ui.popover` under a Filter button. For one filter per chip over a table use `filter-dropdown`; for toggles use `chips`; for a text query use `search-field`.",
	props = {
		["match"] = "\"all\" (default) or \"any\": whether every condition must hold or one is enough; its control sends { match = \"any\" }",
		["conditions"] = "list of { id, field, operator, value }: one row each (none shows a line saying so); a pick in a row sends { set = { id, key = \"field\" | \"operator\" | \"value\", value } }, typing a value { set = { id, key = \"value\" }, value = text } (the text beside `set`), its × { remove = id }",
		["fields"] = "list of { id, label, icon?, operators (words, \"is\", or { id, label }), values? (`menu` options { id, label, icon?, dot?, avatar? }: the value is then a dropdown; without them, a text field) }",
		["open"] = "the open dropdown, as \"<condition id>:field\", \":operator\" or \":value\"; a click on a dropdown sends { open = that }, a click outside or Escape { open = \"\" }",
		["to"] = "mod to send messages to, if not the drawing one",
	},
	styles = {
		[".filter-builder"] = { gap = 10 },
		[".filter-builder-match"] = { gap = 8, color = "muted-foreground" },
		[".filter-builder-rows"] = { gap = 6 },
		[".filter-builder-row"] = { gap = 6 },
		[".filter-builder-empty"] = { color = "muted-foreground", size = "small", padding = { 4, 0 } },
		[".filter-builder-input"] = { padding = { 5, 10 }, radius = 7 },
		[".filter-builder-footer"] = { gap = 8, padding = { 10, 0, 0, 0 }, border = "top", border_color = "border" },
	},
	example = {
		match = "all",
		fields = EXAMPLE_FIELDS,
		conditions = {
			{ id = "c1", field = "status", operator = "is not", value = "done" },
			{ id = "c2", field = "assignee", operator = "is", value = "sarah" },
			{ id = "c3", field = "title", operator = "contains", value = "login" },
		},
	},
	usage = [[local filter_builder = use "filter-builder"

ui.popover {
	open = state.filtering, dismiss = "close-filter", width = 500,
	content = filter_builder {
		match = state.match, conditions = state.conditions, fields = FIELDS, open = state.open,
	},
	ui.button { "Filter", icon = "funnel", small = true, msg = "filter" },
}
-- update: msg.open sets state.open (or clears it when the same comes
-- again); msg.set changes one key of that condition (msg.set.value, or
-- msg.value when typed), and a new field resets its operator and value;
-- msg.remove drops it; msg.match sets state.match; "add" appends
-- { id = new_id(), field = FIELDS[1].id, operator = "is", value = "" };
-- "clear" empties the list; "done" closes the popover.]],

	build = function(props: Props)
		local fields: { [string]: Field } = {}
		local field_items = {}
		for _, field in props.fields do
			fields[field.id] = field
			table.insert(field_items, { id = field.id, label = field.label or field.id, icon = field.icon })
		end
		local close = { open = "" }

		-- One dropdown of a condition, named "<id>:<key>" for `open`.
		local function choice(condition: Condition, key: string, options: { any }, width: number, placeholder: string, icon: string?)
			local name = `{condition.id}:{key}`
			return dropdown {
				options = setters(options, condition.id, key),
				selected = (condition :: any)[key],
				placeholder = placeholder,
				icon = icon,
				open = props.open == name,
				toggle = { open = name },
				dismiss = close,
				trigger_width = width,
				width = math.max(width, 180),
				to = props.to,
			}
		end

		local rows = {}
		local conditions: { Condition } = props.conditions or {}
		for _, condition in conditions do
			local field = fields[condition.field or ""]
			local operators = {}
			local entries: { any } = (if field then field.operators else nil) or {}
			for _, entry in entries do
				local id, label = operator(entry)
				table.insert(operators, { id = id, label = label })
			end
			local value = if field and field.values
				then choice(condition, "value", field.values, 150, "Value")
				else ui.input {
					id = `filter-builder-{condition.id}`,
					value = condition.value or "",
					placeholder = "Value",
					change = { set = { id = condition.id, key = "value" } },
					width = 150,
					to = props.to,
					class = "filter-builder-input",
				}
			table.insert(rows, ui.row {
				class = "filter-builder-row",
				align = "center",
				choice(condition, "field", field_items, 130, "Field", if field then field.icon else nil),
				choice(condition, "operator", operators, 120, "Operator"),
				value,
				ui.spacer(),
				icon_button { icon = "close", tooltip = "Remove filter", size = "small", msg = { remove = condition.id }, to = props.to },
			})
		end
		if #rows == 0 then
			table.insert(rows, ui.text { "No filters yet: add one to narrow the list.", class = "filter-builder-empty" })
		end

		return ui.column {
			class = "filter-builder",
			ui.row {
				class = "filter-builder-match",
				align = "center",
				ui.text "Match",
				segmented {
					small = true,
					selected = props.match or "all",
					event = "match",
					to = props.to,
					options = { { id = "all", label = "all" }, { id = "any", label = "any" } },
				},
				ui.text "of these filters",
			},
			ui.column { class = "filter-builder-rows", table.unpack(rows) },
			ui.row {
				ui.button { "Add filter", icon = "plus", small = true, variant = "ghost", msg = "add", to = props.to },
			},
			ui.row {
				class = "filter-builder-footer",
				align = "center",
				ui.button { "Clear all", small = true, variant = "ghost", msg = "clear", to = props.to },
				ui.spacer(),
				ui.button { "Done", small = true, variant = "primary", msg = "done", to = props.to },
			},
		}
	end,
}
