-- A field for formatted text written as markdown: a toolbar of formatting
-- buttons over a text area that grows with its lines, and a Write | Preview
-- switch that shows the text rendered. Each button edits the field around
-- its selection through `app:edit` (bold wraps it in `**`, a list starts
-- each line with `- `, and pressing it again takes them off), so the mod
-- only keeps the text from `change`, as for any field.

local segmented = use "segmented"

type Props = {
	id: string,
	value: string?,
	change: any,
	blur: any,
	submit: any,
	placeholder: string?,
	rows: number?,
	max_rows: number?,
	preview: boolean?,
	switch: any,
	tools: { string }?,
	empty: string?,
	focus: boolean?,
	to: string?,
}

-- Each button: its icon, its name, and what it does to the selection.
local TOOLS: { [string]: { icon: string, tooltip: string, wrap: { string }?, lines: string? } } = {
	bold = { icon = "bold", tooltip = "Bold", wrap = { "**", "**" } },
	italic = { icon = "italic", tooltip = "Italic", wrap = { "_", "_" } },
	strike = { icon = "strikethrough", tooltip = "Strikethrough", wrap = { "~~", "~~" } },
	code = { icon = "code", tooltip = "Code", wrap = { "`", "`" } },
	link = { icon = "link", tooltip = "Link", wrap = { "[", "](https://)" } },
	heading = { icon = "heading", tooltip = "Heading", lines = "### " },
	bullets = { icon = "list", tooltip = "Bulleted list", lines = "- " },
	numbers = { icon = "list-ordered", tooltip = "Numbered list", lines = "1. " },
	checklist = { icon = "list-checks", tooltip = "Checklist", lines = "- [ ] " },
	quote = { icon = "text-quote", tooltip = "Quote", lines = "> " },
}

-- The buttons in order; "|" draws a divider.
local DEFAULT_TOOLS = { "bold", "italic", "strike", "code", "link", "|", "heading", "bullets", "numbers", "checklist", "quote" }

return component {
	title = "Text Editor",
	category = "Forms",
	icon = "text-quote",
	description = "Formatted text written as markdown: a toolbar (bold, italic, code, link, heading, lists, quote) over a text area that grows with its lines, and a Write | Preview switch.",
	when = "A task's or project's description, release notes, a long comment: text with some formatting, edited in place. For a markdown file of the project use `markdown-editor`; for markdown only shown use `markdown`; for a chat message use `composer`; for one line use `ui.input`.",
	props = {
		["id"] = "the field's id; the toolbar edits it through `app:edit`",
		["value"] = "the markdown, kept from `change`",
		["change"] = "sent on each edit (typed or from the toolbar), with `value`",
		["blur"] = "sent with `value` when the field loses focus: save it then",
		["submit"] = "sent on ⌘↩",
		["placeholder"] = "grey text while empty",
		["rows"] = "lines shown when empty (default 4)",
		["max_rows"] = "the most lines it grows to before it scrolls (default 16)",
		["preview"] = "show the text rendered instead of the field",
		["switch"] = "key of the message the Write | Preview switch sends, { [switch] = \"write\" | \"preview\" }; no switch without it",
		["tools"] = "the buttons, in order: bold, italic, strike, code, link, heading, bullets, numbers, checklist, quote, and \"|\" for a divider (default all)",
		["empty"] = "what the preview says when there is no text (default \"Nothing to preview\")",
		["focus"] = "take focus when it appears",
		["to"] = "mod `change`, `blur`, `submit` and `switch` go to, if not the drawing one",
	},
	styles = {
		[".text-editor"] = { border = true, border_color = "border", radius = 8, background = "background", clip = true },
		[".text-editor:focus"] = { border_color = "muted-foreground/50" },
		[".text-editor-bar"] = { padding = { 4, 6 }, gap = 2, border = "bottom", border_color = "border/70" },
		[".text-editor-tool"] = { radius = 5, padding = 5, color = "muted-foreground" },
		[".text-editor-tool:hover"] = { background = "list-hover", color = "foreground" },
		[".text-editor-divider"] = { background = "border", margin = { 0, 4 } },
		[".text-editor-input"] = { border = "none", background = "#00000000", padding = { 8, 12 }, shadow = "none" },
		[".text-editor-preview"] = { padding = { 8, 12 } },
		[".text-editor-empty"] = { color = "muted-foreground", size = "small" },
	},
	example = {
		id = "description",
		value = "Rebuild the site with a **clearer** information architecture.\n\n- Navigation\n- Responsive layouts",
		switch = "view",
	},
	usage = [[local text_editor = use "text-editor"

text_editor {
	id = "description", value = state.text, change = "type", blur = "save",
	preview = state.preview, switch = "view", placeholder = "Add a description…",
}
-- update: msg.msg == "type" keeps state.text = msg.value; "save" stores it;
-- msg.view == "preview" sets state.preview (and "write" clears it).]],

	build = function(props: Props)
		local tools = {}
		local names: { string } = props.tools or DEFAULT_TOOLS
		for _, name in names do
			if name == "|" then
				table.insert(tools, ui.row { class = "text-editor-divider", width = 1, height = 16 })
				continue
			end
			local tool = TOOLS[name]
			if not tool then
				continue
			end
			-- Sent straight to the app: the button never takes the focus, so
			-- the selection is still there to edit.
			local edit = if tool.wrap
				then { field = props.id, wrap = tool.wrap }
				else { field = props.id, lines = tool.lines }
			table.insert(tools, ui.row {
				class = "text-editor-tool",
				tooltip = tool.tooltip,
				msg = edit,
				to = "app:edit",
				ui.icon { name = tool.icon :: any, size = "small" },
			})
		end
		local preview = props.preview == true
		local value = props.value or ""
		local body
		if preview then
			body = ui.column {
				class = "text-editor-preview",
				height = (props.rows or 4) * 20 + 16,
				grow = true,
				if value == ""
					then ui.text { props.empty or "Nothing to preview", class = "text-editor-empty" }
					else ui.markdown { text = value, grow = true },
			}
		else
			body = ui.input {
				id = props.id,
				class = "text-editor-input",
				value = props.value,
				placeholder = props.placeholder,
				change = props.change,
				blur = props.blur,
				submit = props.submit,
				to = props.to,
				multiline = true,
				rows = props.rows or 4,
				max_rows = props.max_rows or 16,
				focus = props.focus,
				grow = true,
			}
		end
		return ui.column {
			class = "text-editor",
			gap = "none",
			ui.row {
				class = "text-editor-bar",
				align = "center",
				not preview and ui.row { align = "center", gap = "none", table.unpack(tools) },
				ui.spacer(),
				props.switch ~= nil and segmented {
					small = true,
					selected = if preview then "preview" else "write",
					event = props.switch,
					to = props.to,
					options = { { id = "write", label = "Write" }, { id = "preview", label = "Preview" } },
				},
			},
			body,
		}
	end,
}
