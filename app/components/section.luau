-- A small heading over a group of content, as in a sidebar or inspector.

type Props = {
	title: string,
	[number]: any,
}

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
	example = { title = "Worktrees" },

	build = function(props: Props)
		local children = { ui.text { props.title, size = "small", tone = "muted", weight = "medium" } }
		for _, child in ipairs(props) do
			children[#children + 1] = child
		end
		if #children == 1 then
			children[2] = ui.text { "Content goes here.", size = "small" }
		end
		return ui.column { gap = "small", table.unpack(children) }
	end,
}
