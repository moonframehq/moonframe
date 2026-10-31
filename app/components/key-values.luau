-- Labels and values in two aligned columns: properties, details, metadata.

type Props = {
	-- `{ label, value }` pairs, in order.
	rows: { { string } },
	-- Pixels for the label column.
	width: number?,
}

return component {
	title = "Key values",
	category = "Data",
	icon = "file-text",
	description = "Label and value pairs in two aligned columns, for properties and details.",
	when = "Read-only properties or details: aligned label/value lines. For editable settings use `settings-row`.",
	props = {
		["rows"] = "list of { label, value }",
		["width"] = "label column width in pixels",
	},
	example = { rows = { { "Branch", "main" }, { "Ahead", "2 commits" }, { "Changed", "14 files" } } },

	build = function(props: Props)
		local rows = {}
		for index, pair in props.rows do
			rows[index] = ui.row {
				gap = "small",
				ui.row { width = props.width or 80, ui.text { pair[1], size = "small", tone = "muted" } },
				ui.text { pair[2] or "", size = "small", truncate = true },
			}
		end
		return ui.column { gap = "small", table.unpack(rows) }
	end,
}
