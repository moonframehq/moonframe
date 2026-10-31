-- A list of selectable rows, each with a label and optional detail and
-- badge. Clicking a row sends `{ select = <id> }` to the mod that draws it.

type Row = {
	id: string?,
	label: string,
	detail: string?,
	badge: string?,
}

type Props = {
	rows: { Row },
	selected: string?,
	-- The mod to tell, if not the one drawing the list.
	to: string?,
}

return component {
	title = "List",
	category = "Lists",
	icon = "menu",
	description = "Selectable rows with a label, detail and badge; a click sends { select = id } to the mod drawing it.",
	when = "A plain selectable list of rows with label, detail and badge. For rich rows (status, icons, two lines) use `agent-row`, `workspace-row` or `tree`.",
	props = {
		["rows"] = "list of { id, label, detail, badge }",
		["selected"] = "id of the selected row",
		["to"] = "mod to send { select = id } to, if not the drawing one",
	},
	example = {
		selected = "main",
		rows = {
			{ id = "main", label = "main", detail = "~/code/app", badge = "3" },
			{ id = "feature", label = "feature/inspect", detail = "~/code/app-inspect", badge = "new" },
			{ id = "fix", label = "fix/menus", detail = "~/code/app-menus", badge = "1" },
		},
	},

	build = function(props: Props)
		local items = {}
		for index, row in props.rows do
			local id = row.id or row.label
			items[index] = ui.item {
				label = row.label,
				detail = row.detail,
				badge = row.badge,
				selected = id == props.selected,
				msg = { select = id },
				to = props.to,
			}
		end
		return ui.column { gap = "none", table.unpack(items) }
	end,
}
