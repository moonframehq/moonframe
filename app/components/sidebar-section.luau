-- One titled, foldable group in a sidebar: a small uppercase heading with a
-- count, trailing actions and a fold chevron, over its rows. Stateless: the
-- mod keeps `collapsed` and flips it when the heading sends `toggle`.

type Props = {
	title: string,
	count: (number | string)?,
	collapsed: boolean?,
	-- Sent when the heading is clicked; `{ toggle = id }` by default.
	toggle: any,
	id: string?,
	to: string?,
	actions: { any }?,
	-- No chevron and no click: a heading that always shows its rows.
	fixed: boolean?,
	[number]: any,
}

-- The children in order. `ipairs` and `#` stop at the first `nil`, which a
-- child such as `cond and ui.text "x"` leaves, and drop what follows it.
local function positional(props: any): { any }
	local keys = {}
	for key, child in props do
		if type(key) == "number" and child then
			table.insert(keys, key)
		end
	end
	table.sort(keys)
	local list = table.create(#keys)
	for _, key in keys do
		table.insert(list, props[key])
	end
	return list
end

return component {
	title = "Sidebar section",
	category = "Navigation",
	icon = "chevrons-up-down",
	description = "A foldable sidebar group: uppercase heading, count, trailing actions and chevron over its rows.",
	when = "A titled group of rows inside a sidebar (\"Spaces\", \"Agents\", \"APPEARANCE\"). The whole column is `sidebar`; a list heading with a status dot and `+` is `group-header`; a foldable card in a page is `collapsible`; a plain non-folding heading is `section`.",
	props = {
		["title"] = "heading text, shown uppercase",
		["count"] = "number shown after the title (omit for none)",
		["collapsed"] = "hide the rows",
		["toggle"] = "msg sent when the heading is clicked; default `{ toggle = id }`",
		["id"] = "id for the default toggle msg (default the title)",
		["to"] = "mod to send the msg to, if not the drawing one",
		["actions"] = "elements on the heading's right (icon buttons, small text buttons)",
		["fixed"] = "no chevron and no click: the rows always show",
		["[children]"] = "the rows",
	},
	styles = {
		[".sidebar-section"] = { gap = 2, padding = { 4, 0 } },
		[".sidebar-section-header"] = { padding = { 4, 12 }, gap = 6 },
		[".sidebar-section-toggle"] = { gap = 6, radius = 6, padding = { 2, 4 } },
		[".sidebar-section-toggle:hover"] = { color = "foreground" },
		[".sidebar-section-title"] = { size = "small", weight = "medium", color = "muted-foreground" },
		[".sidebar-section-count"] = { size = "small", color = "muted-foreground/70" },
		[".sidebar-section-chevron"] = { size = "small", color = "muted-foreground/70" },
		[".sidebar-section-body"] = { gap = 2, padding = { 0, 6 } },
	},
	usage = [[local section = use "sidebar-section"

section { title = "Agents", count = 3, collapsed = state.agents_folded, toggle = "fold-agents",
	actions = { ui.button { "grouped", small = true, variant = "ghost", msg = "group" } },
	ui.item { "ledger-api", detail = "claude", selected = true, msg = { open = "ledger-api" } },
	ui.item { "pixel-forge", detail = "codex", msg = { open = "pixel-forge" } },
	ui.item { "harbor-docs", detail = "pi", msg = { open = "harbor-docs" } },
}]],
	example = {
		title = "Agents",
		count = 3,
		toggle = "fold-agents",
		actions = { ui.button { "grouped", small = true, variant = "ghost", msg = "group" } },
		ui.item { "ledger-api", detail = "claude", selected = true, msg = { open = "ledger-api" } },
		ui.item { "pixel-forge", detail = "codex", msg = { open = "pixel-forge" } },
		ui.item { "harbor-docs", detail = "pi", msg = { open = "harbor-docs" } },
	},

	build = function(props: Props)
		local collapsed = props.collapsed == true and not props.fixed
		local label: { any } = {
			ui.text { string.upper(props.title), class = "sidebar-section-title", truncate = true },
			props.count ~= nil and ui.text { tostring(props.count), class = "sidebar-section-count" },
		}
		if not props.fixed then
			label[#label + 1] = ui.icon {
				name = (collapsed and "chevron-right" or "chevron-down") :: any,
				class = "sidebar-section-chevron",
			}
		end
		local heading
		if props.fixed then
			heading = ui.row { class = "sidebar-section-toggle", align = "center", grow = true, table.unpack(label) }
		else
			heading = ui.row {
				class = "sidebar-section-toggle",
				align = "center",
				grow = true,
				msg = props.toggle or { toggle = props.id or props.title },
				to = props.to,
				table.unpack(label),
			}
		end
		local header = ui.row {
			class = "sidebar-section-header",
			align = "center",
			heading,
			table.unpack(props.actions or {}),
		}
		local body = nil
		local children = positional(props)
		if not collapsed and #children > 0 then
			body = ui.column { class = "sidebar-section-body", table.unpack(children) }
		end
		return ui.column { class = "sidebar-section", header, body or false }
	end,
}
