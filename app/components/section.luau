-- A small heading over a group of content, as in a sidebar or inspector.

type Props = {
	title: string,
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
	title = "Section",
	category = "Containers",
	icon = "layout-dashboard",
	description = "A small muted heading over its content, for grouping inside panels.",
	when = "A small muted heading over a group of content inside a panel or settings page.",
	props = {
		["title"] = "heading text",
		["[children]"] = "the content",
	},
	usage = [[local section = use "section"

section { title = "Worktrees",
	ui.text "Content goes here.",
}]],
	example = { title = "Worktrees", ui.text { "Content goes here.", size = "small" } },

	build = function(props: Props)
		return ui.column {
			gap = "small",
			ui.text { props.title, size = "small", tone = "muted", weight = "medium" },
			table.unpack(positional(props)),
		}
	end,
}
