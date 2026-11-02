-- Labelled controls in two aligned columns, tight enough for a tool's
-- sidebar: a label on the left of each row, its control on the right, an
-- optional muted hint under it.

type Row = {
	label: string?,
	control: any,
	hint: string?,
}

type Props = {
	rows: { Row },
	label_width: number?,
	gap: number?,
}

return component {
	title = "Form Grid",
	category = "Forms",
	icon = "layout-grid",
	description = "Rows of a label and its control in two aligned columns, with an optional hint under a control: a compact form.",
	when = "A dense form in a tool's sidebar or inspector: an output's size, a scanner's holder, a file name. For a settings page with descriptions use `settings-row`; for read-only label/value pairs use `key-values`.",
	props = {
		["rows"] = "list of { label?, control (an element, or a list of them in a row), hint? }; a row without a label puts its control under the others",
		["label_width"] = "the label column's width in pixels (default 96)",
		["gap"] = "pixels between rows (default 8)",
	},
	styles = {
		[".form-grid-label"] = { size = "small", color = "muted-foreground" },
		[".form-grid-hint"] = { size = "small", color = "muted-foreground" },
	},
	example = {
		rows = {
			{ label = "Name", control = ui.input { id = "form-grid-name", value = "scan", width = 140 } },
			{ label = "Format", control = ui.badge "TIFF 16-bit", hint = "Written beside the scan" },
		},
	},

	build = function(props: Props)
		local label_width = props.label_width or 96
		local rows = {}
		for index, row in props.rows do
			local control = row.control
			if typeof(control) == "table" and control.type == nil then
				control = ui.row { align = "center", gap = "small", table.unpack(control) }
			end
			rows[index] = ui.row {
				align = "start",
				gap = "normal",
				ui.row {
					width = label_width,
					shrink = false,
					style = { padding = { 4, 0 } },
					row.label and ui.text { row.label, class = "form-grid-label", truncate = true },
				},
				ui.column {
					grow = true,
					gap = "small",
					control,
					row.hint and ui.text { row.hint, class = "form-grid-hint" },
				},
			}
		end
		return ui.column { style = { gap = props.gap or 8 }, table.unpack(rows) }
	end,
}
