-- Read-only lines of code with line numbers, in the mono font: a file, a
-- snippet, a log excerpt. Lines can be highlighted, numbering can start
-- anywhere, long lines are cut rather than wrapped, and it scrolls.

type Props = {
	text: string?,
	lines: { string }?,
	start: number?,
	highlight: { number }?,
	numbers: boolean?,
	path: string?,
	height: number?,
	grow: boolean?,
	event: string?,
	to: string?,
	id: string?,
}

-- Past this many lines a view that scrolls is a `ui.list`, which builds only
-- the lines that show: a view holds at most 10,000 elements. Its long lines
-- are then cut at the edge rather than scrolled to.
local LIST_AFTER = 500
local LINE_HEIGHT = 20

return component {
	title = "Code View",
	category = "Data",
	icon = "file-text",
	description = "Read-only mono lines with line numbers, highlighted lines, a start line and an optional path header; no wrapping; scrolls.",
	when = "Show code or a log read-only: a file at a line, a snippet in a comment, a test's output. For changes use `diff-view`; for a live shell use `terminal`.",
	props = {
		["text"] = "the code; split into lines on \\n",
		["lines"] = "the lines instead of `text`",
		["start"] = "number of the first line (default 1)",
		["highlight"] = "line numbers to highlight",
		["numbers"] = "false hides line numbers",
		["path"] = "a header with the file path",
		["height"] = "pixels: scrolls within it",
		["grow"] = "fill the parent and scroll",
		["event"] = "line click message key: sends { [event] = line number } (no click without it)",
		["to"] = "the mod to send it to",
		["id"] = "names the list a long scrolling view becomes (default \"code-view\"); two long views in one mod need two",
	},
	styles = {
		[".code-view"] = { gap = 0, border = "all", border_color = "border", radius = 8, background = "background" },
		[".code-view-header"] = {
			padding = { 6, 10 },
			gap = 6,
			border = "bottom",
			border_color = "border",
			background = "muted-foreground/8",
		},
		[".code-view-path"] = { font = "mono", size = "small", weight = "medium" },
		[".code-view-header-icon"] = { size = "small", color = "muted-foreground" },
		[".code-view-body"] = { gap = 0, padding = { 6, 0 } },
		[".code-view-line"] = { gap = 0 },
		[".code-view-line:hover"] = { background = "list-hover" },
		[".code-view-highlight"] = { background = "warning/14" },
		[".code-view-gutter"] = { padding = { 0, 12, 0, 8 } },
		[".code-view-number"] = { font = "mono", size = "small", color = "muted-foreground/60" },
		[".code-view-text"] = { font = "mono", size = "small" },
	},
	usage = [[local code_view = use "code-view"

code_view {
	path = "src/ledger.rs", start = 24, highlight = { 26, 27 }, height = 230,
	text = state.source, -- or lines = { "impl Book {", ... }
}]],
	example = {
		path = "src/ledger.rs",
		start = 24,
		highlight = { 26, 27 },
		height = 230,
		text = "impl Book {\n    /// Balance per currency; amounts in different currencies never mix.\n    pub fn balances(&self) -> HashMap<Currency, i64> {\n        let mut totals = HashMap::new();\n        for posting in &self.entries {\n            *totals.entry(posting.currency).or_default() += posting.cents;\n        }\n        totals\n    }\n}",
	},

	build = function(props: Props)
		local lines: { string } = props.lines or string.split(props.text or "", "\n")
		local start = props.start or 1
		local highlighted = {}
		local highlight: { number } = props.highlight or {}
		for _, line in highlight do
			highlighted[line] = true
		end
		local digits = #tostring(start + #lines - 1)
		local scrolls = props.height ~= nil or props.grow == true
		local listed = scrolls and #lines > LIST_AFTER

		local function draw(index: number)
			local text = lines[index]
			local number = start + index - 1
			local classes = { "code-view-line" }
			if highlighted[number] then
				table.insert(classes, "code-view-highlight")
			end
			return ui.row {
				class = classes,
				shrink = false,
				align = "center",
				height = if listed then LINE_HEIGHT else nil,
				msg = if props.event then { [props.event] = number } else nil,
				to = props.to,
				props.numbers ~= false and ui.row {
					class = "code-view-gutter",
					width = digits * 9 + 22,
					justify = "end",
					ui.text { tostring(number), class = "code-view-number" },
				},
				ui.text { if text == "" then " " else text, class = "code-view-text", wrap = false },
			}
		end

		local body
		if listed then
			body = ui.list {
				id = props.id or "code-view",
				class = "code-view-body",
				height = props.height,
				grow = props.grow,
				count = #lines,
				row_height = LINE_HEIGHT,
				row = draw,
			}
		else
			local rows = table.create(#lines)
			for index = 1, #lines do
				rows[index] = draw(index)
			end
			body = ui.column {
				class = "code-view-body",
				height = props.height,
				grow = props.grow,
				scroll = scrolls and "both",
				table.unpack(rows),
			}
		end
		return ui.column {
			class = "code-view",
			grow = props.grow,
			props.path and ui.row {
				class = "code-view-header",
				shrink = false,
				align = "center",
				ui.icon { name = "file", class = "code-view-header-icon" },
				ui.text { props.path, class = "code-view-path", truncate = true },
			},
			body,
		}
	end,
}
