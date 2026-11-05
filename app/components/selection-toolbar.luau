-- Formatting buttons that show over a text field while text is selected,
-- for fields with no toolbar of their own. The buttons edit the field through
-- `app:edit`. The bar sits at the field's edge, not at the selection.

type Props = {
	[number]: any,
	field: string,
	selected: number?,
	tools: { string }?,
	side: any,
}

local TOOLS: { [string]: { icon: any, tooltip: string, edit: { [string]: any } } } = {
	bold = { icon = "bold", tooltip = "Bold", edit = { wrap = { "**", "**" } } },
	italic = { icon = "italic", tooltip = "Italic", edit = { wrap = { "_", "_" } } },
	strike = { icon = "strikethrough", tooltip = "Strikethrough", edit = { wrap = { "~~", "~~" } } },
	code = { icon = "code", tooltip = "Code", edit = { wrap = { "`", "`" } } },
	link = { icon = "link", tooltip = "Link", edit = { wrap = { "[", "](url)" } } },
	quote = { icon = "message-square", tooltip = "Quote", edit = { lines = "> " } },
}

return component {
	title = "Selection Toolbar",
	category = "Forms",
	icon = "bold",
	description = "Bold, italic, code and link buttons that float over a text field while text is selected, and edit it in place.",
	when = "A `ui.input` or `prose-editor` that keeps its chrome out of the way until words are selected. For a bar that always shows use `text-editor`; for actions on a row use `context-menu`.",
	props = {
		["[children]"] = "the field",
		["field"] = "the `id` of the `ui.input` it edits",
		["selected"] = "bytes selected, from the field's `select` message (`msg.selected`); it shows above 0",
		["tools"] = "the buttons, in order: bold, italic, strike, code, link, quote (default all)",
		["side"] = "\"top\" (default) or \"bottom\" of the field",
	},
	styles = {
		[".selection-toolbar"] = { padding = 3, radius = 8 },
		[".selection-toolbar-tool"] = { padding = 5, radius = 5, color = "muted-foreground" },
		[".selection-toolbar-tool:hover"] = { background = "list-hover", color = "foreground" },
	},
	example = { field = "example", selected = 4 },
	usage = [[local selection_toolbar = use "selection-toolbar"

selection_toolbar { field = "notes", selected = state.selected,
	ui.input { id = "notes", multiline = true, change = "type", select = "moved" },
}
-- update: msg.msg == "moved" keeps state.selected = msg.selected.]],

	build = function(props: Props)
		local buttons = {}
		local tools: { string } = props.tools or { "bold", "italic", "strike", "code", "link", "quote" }
		for _, name in tools do
			local tool = TOOLS[name]
			if not tool then
				continue
			end
			local edit = table.clone(tool.edit)
			edit.field = props.field
			table.insert(buttons, ui.row {
				class = "selection-toolbar-tool",
				tooltip = tool.tooltip,
				msg = edit,
				to = "app:edit",
				ui.icon(tool.icon),
			})
		end
		return ui.popover {
			class = "selection-toolbar",
			open = (props.selected or 0) > 0,
			anchor_clicks = true,
			side = props.side or "top",
			align = "start",
			gap = 4,
			content = ui.row { gap = "none", table.unpack(buttons) },
			props[1] or ui.input { id = props.field, value = "Some words" },
		}
	end,
}
