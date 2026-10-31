-- A wrapping row of badges: labels, states, names.

type Props = {
	tags: { string },
	tone: any,
}

return component {
	title = "Tags",
	category = "Data",
	icon = "star",
	description = "Badges in a row that wraps, for labels, states or names.",
	when = "A wrapping set of small labels: languages, topics, states.",
	props = {
		["tags"] = "list of strings",
		["tone"] = "badge tone",
	},
	example = { tags = { "rust", "gpui", "luau", "wasm" } },

	build = function(props: Props)
		local badges = {}
		for index, tag in props.tags do
			badges[index] = ui.badge { tag, tone = props.tone }
		end
		return ui.row { gap = "small", wrap = true, table.unpack(badges) }
	end,
}
