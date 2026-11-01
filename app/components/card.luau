-- A titled, rounded box for a group of content, with optional actions in
-- its header. Its children go after the title.

type Props = {
	title: string?,
	subtitle: string?,
	actions: { any }?,
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
	title = "Card",
	category = "Containers",
	icon = "frame",
	description = "A rounded, bordered box with a title, an optional subtitle and header actions, and any content.",
	when = "A titled box around related content inside a panel or page; for a whole sidebar section use `ui.panel`, for a plain heading use `section`.",
	props = {
		["title"] = "heading text",
		["subtitle"] = "muted line under the title",
		["actions"] = "elements on the header's right (buttons, icons)",
		["[children]"] = "the body",
	},
	usage = [[local card = use "card"

card { title = "Worktrees", subtitle = "3 checked out",
	actions = { ui.button { "Add", small = true, variant = "ghost", msg = "add" } },
	ui.text "Content goes here.",
}]],
	example = {
		title = "Worktrees",
		subtitle = "3 checked out",
		ui.text { "Content goes here.", size = "small", tone = "muted" },
	},

	build = function(props: Props)
		local header = ui.row {
			align = "center",
			gap = "small",
			ui.column {
				grow = true,
				gap = "none",
				props.title and ui.text { props.title, weight = "medium" },
				props.subtitle and ui.text { props.subtitle, size = "small", tone = "muted" },
			},
			table.unpack(props.actions or {}),
		}
		return ui.column {
			gap = "small",
			style = { background = "background", border = "all", radius = 10, padding = 12 },
			header,
			table.unpack(positional(props)),
		}
	end,
}
