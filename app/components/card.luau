-- A titled, rounded box for a group of content, with optional actions in
-- its header. Its children go after the title.

type Props = {
	title: string?,
	subtitle: string?,
	actions: { any }?,
	[number]: any,
}

return component {
	title = "Card",
	category = "Containers",
	icon = "frame",
	description = "A rounded, bordered box with a title, an optional subtitle and header actions, and any content.",
	usage = [[local card = use "card"

card { title = "Worktrees", subtitle = "3 checked out",
	actions = { ui.button { "Add", small = true, variant = "ghost", msg = "add" } },
	ui.text "Content goes here.",
}]],
	example = { title = "Worktrees", subtitle = "3 checked out" },

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
		local children = { header }
		for _, child in ipairs(props) do
			children[#children + 1] = child
		end
		if #children == 1 then
			children[2] = ui.text { "Content goes here.", size = "small", tone = "muted" }
		end
		return ui.column {
			gap = "small",
			style = { background = "background", border = "all", radius = 10, padding = 12 },
			table.unpack(children),
		}
	end,
}
