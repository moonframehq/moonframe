-- Steps or points in order, each with its number in a muted column so the
-- text lines up when it wraps.

type Props = {
	items: { string },
	start: number?,
}

return component {
	title = "Numbered List",
	category = "Data",
	icon = "list",
	description = "Items in order, numbered in a muted column, wrapping text aligned.",
	when = "Ordered steps or suggestions in a message, a note or a help page. For a checklist use `task-row`; for events in time use `timeline`.",
	props = {
		["items"] = "the items' text",
		["start"] = "the first number (default 1)",
	},
	styles = {
		[".numbered-list"] = { gap = 8 },
		[".numbered-list-item"] = { gap = 8 },
		[".numbered-list-number"] = { color = "muted-foreground" },
	},
	example = { items = { "Send the revised pricing deck", "Book a call with procurement", "Offer a two-year term" } },

	build = function(props: Props)
		local rows = {}
		for index, item in props.items do
			table.insert(rows, ui.row {
				class = "numbered-list-item",
				align = "start",
				ui.row { width = 18, shrink = false, ui.text { `{(props.start or 1) + index - 1}.`, class = "numbered-list-number" } },
				ui.column { grow = true, ui.text { item, wrap = true } },
			})
		end
		return ui.column { class = "numbered-list", table.unpack(rows) }
	end,
}
