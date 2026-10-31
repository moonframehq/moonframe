-- One file's diff: a header (collapse chevron, status letter, path, +/−
-- counts) over its hunks, each an @@ row then its lines with old and new
-- line numbers, +/− and green or red washes, in the mono font. `split`
-- shows the old and new sides next to each other. The body scrolls.

type Line = {
	kind: string,
	old: number?,
	new: number?,
	text: string,
}

type Hunk = {
	header: string?,
	lines: { Line },
}

type Props = {
	path: string?,
	status: string?,
	hunks: { Hunk },
	split: boolean?,
	collapsed: boolean?,
	height: number?,
	grow: boolean?,
	header: boolean?,
	event: string?,
	to: string?,
	actions: { any }?,
	switch: string?,
}

local diff_counts = use "diff-counts"
local button_group = use "button-group"

-- Each status letter's class, as git marks them.
local STATUS = { A = "added", M = "modified", D = "deleted", R = "renamed", U = "conflict", ["?"] = "untracked" }

local SIGNS = { add = "+", del = "−", ctx = " " }

local EXAMPLE_HUNKS: { Hunk } = {
	{
		header = "@@ -1,8 +1,14 @@",
		lines = {
			{ kind = "add", new = 1, text = "use std::collections::HashMap;" },
			{ kind = "add", new = 2, text = "use crate::currency::Currency;" },
			{ kind = "add", new = 3, text = "" },
			{ kind = "ctx", old = 1, new = 4, text = "#[derive(Default)]" },
			{ kind = "ctx", old = 2, new = 5, text = "pub struct Book {" },
			{ kind = "del", old = 3, text = "    entries: Vec<(String, i64)>," },
			{ kind = "add", new = 6, text = "    entries: Vec<Posting>," },
			{ kind = "ctx", old = 4, new = 7, text = "}" },
		},
	},
	{
		header = "@@ -11,4 +17,9 @@ impl Book {",
		lines = {
			{ kind = "del", old = 11, text = "    pub fn balance(&self) -> i64 {" },
			{ kind = "del", old = 12, text = "        self.entries.iter().map(|(_, c)| c).sum()" },
			{ kind = "add", new = 17, text = "    pub fn balances(&self) -> HashMap<Currency, i64> {" },
			{ kind = "add", new = 18, text = "        let mut totals = HashMap::new();" },
			{ kind = "add", new = 19, text = "        for posting in &self.entries {" },
			{ kind = "add", new = 20, text = "            *totals.entry(posting.currency).or_default() += posting.cents;" },
			{ kind = "add", new = 21, text = "        }" },
			{ kind = "add", new = 22, text = "        totals" },
			{ kind = "ctx", old = 13, new = 23, text = "    }" },
		},
	},
}

return component {
	title = "Diff View",
	category = "Git",
	icon = "git-compare",
	description = "One file's diff: header with status, path and +/− counts, @@ hunk rows, two line-number gutters, green and red washes; unified or split, collapsible, scrolling.",
	when = "Show a file's changes for review: a review pane, a checkpoint, a commit. Stack one per file in a scrolling column. For only the size of a change use `diff-counts`; for a file without changes use `code-view`.",
	props = {
		["path"] = "file path shown in the header",
		["status"] = "git status letter: A, M, D, R, U or ?",
		["hunks"] = "list of { header = \"@@ -1,15 +1,34 @@\", lines = { { kind = add|del|ctx, old, new, text } } }",
		["split"] = "old and new side by side instead of unified",
		["collapsed"] = "show only the header",
		["header"] = "false hides the file header",
		["height"] = "pixels: the body scrolls within it",
		["grow"] = "fill the parent and scroll the body",
		["event"] = "header click message key instead of \"toggle\": sends { [event] = path }",
		["to"] = "the mod to send it to",
		["actions"] = "elements at the header's right (buttons)",
		["switch"] = "message key for a unified/split toggle in the header: sends { [switch] = \"unified\" | \"split\" }",
	},
	styles = {
		[".diff-view"] = { gap = 0, border = "all", border_color = "border", radius = 8 },
		[".diff-view-header"] = {
			gap = 8,
			padding = { 6, 10 },
			background = "muted-foreground/8",
			border = "bottom",
			border_color = "border",
		},
		[".diff-view-header:hover"] = { background = "muted-foreground/12" },
		[".diff-view-chevron"] = { size = "small", color = "muted-foreground" },
		[".diff-view-path"] = { font = "mono", weight = "bold", size = "small" },
		[".diff-view-status"] = { font = "mono", weight = "bold", size = "small", color = "muted-foreground" },
		[".diff-view-status-added"] = { color = "success" },
		[".diff-view-status-modified"] = { color = "warning" },
		[".diff-view-status-deleted"] = { color = "danger" },
		[".diff-view-status-renamed"] = { color = "accent" },
		[".diff-view-status-conflict"] = { color = "danger" },
		[".diff-view-status-untracked"] = { color = "success" },
		[".diff-view-body"] = { gap = 0 },
		[".diff-view-hunk"] = { background = "accent/8", padding = { 2, 0 } },
		[".diff-view-hunk-text"] = { font = "mono", size = "small", color = "muted-foreground" },
		[".diff-view-line"] = { gap = 0 },
		[".diff-view-add"] = { background = "success/14" },
		[".diff-view-del"] = { background = "danger/14" },
		[".diff-view-blank"] = { background = "muted-foreground/6" },
		[".diff-view-gutter"] = { padding = { 0, 6 } },
		[".diff-view-number"] = { font = "mono", size = "small", color = "muted-foreground/70" },
		[".diff-view-sign"] = { font = "mono", size = "small", color = "muted-foreground" },
		[".diff-view-sign-add"] = { color = "success" },
		[".diff-view-sign-del"] = { color = "danger" },
		[".diff-view-text"] = { font = "mono", size = "small" },
		[".diff-view-split"] = { border = "left", border_color = "border" },
	},
	usage = [[local diff_view = use "diff-view"

	diff_view {
		path = "src/ledger.rs", status = "M", split = state.split,
		collapsed = state.collapsed["src/ledger.rs"], grow = true,
		hunks = {
			{ header = "@@ -1,8 +1,14 @@", lines = {
				{ kind = "add", new = 1, text = "use std::collections::HashMap;" },
				{ kind = "ctx", old = 1, new = 2, text = "#[derive(Default)]" },
				{ kind = "del", old = 2, text = "    entries: Vec<(String, i64)>," },
				{ kind = "add", new = 3, text = "    entries: Vec<Posting>," },
			} },
		},
	} -- the header sends { toggle = path }]],
	example = {
		path = "src/ledger.rs",
		status = "M",
		height = 330,
		hunks = EXAMPLE_HUNKS,
		switch = "layout",
	},

	build = function(props: Props)
		local added, removed = 0, 0
		for _, hunk in props.hunks do
			for _, line in hunk.lines do
				if line.kind == "add" then
					added += 1
				elseif line.kind == "del" then
					removed += 1
				end
			end
		end

		local function number(value: number?)
			return ui.row {
				class = "diff-view-gutter",
				width = 44,
				justify = "end",
				ui.text { if value then tostring(value) else "", class = "diff-view-number" },
			}
		end

		local function sign(kind: string)
			return ui.row {
				width = 14,
				ui.text { SIGNS[kind] or " ", class = { "diff-view-sign", `diff-view-sign-{kind}` } },
			}
		end

		local function code(text: string)
			-- A space keeps an empty line its height.
			return ui.text { if text == "" then " " else text, class = "diff-view-text", wrap = false }
		end

		-- Unified: both numbers, the sign, the text.
		local function unified(line: Line)
			return ui.row {
				class = { "diff-view-line", `diff-view-{line.kind}` },
				shrink = false,
				align = "center",
				number(line.old),
				number(line.new),
				sign(line.kind),
				code(line.text),
			}
		end

		-- One side of a split row; nil draws an empty, shaded side.
		local function side(line: Line?, which: string, extra: string?)
			if line == nil then
				return ui.row {
					class = `diff-view-line diff-view-blank {extra or ""}`,
					grow = true,
					style = { gap = 0 },
					number(nil),
					ui.row { width = 14 },
				}
			end
			local kind = line.kind
			return ui.row {
				class = `diff-view-line diff-view-{kind} {extra or ""}`,
				grow = true,
				align = "center",
				number(if which == "old" then line.old else line.new),
				sign(kind),
				ui.row { grow = true, shrink = true, code(line.text) },
			}
		end

		local rows = {}
		for _, hunk in props.hunks do
			if hunk.header then
				table.insert(rows, ui.row {
					class = "diff-view-hunk",
					shrink = false,
					align = "center",
					ui.row { width = if props.split then 58 else 102 },
					ui.text { hunk.header, class = "diff-view-hunk-text", truncate = true },
				})
			end
			if not props.split then
				for _, line in hunk.lines do
					table.insert(rows, unified(line))
				end
			else
				-- Pair each run of removed lines with the added lines after it.
				local index = 1
				local lines = hunk.lines
				while index <= #lines do
					local line = lines[index]
					if line.kind == "ctx" then
						table.insert(rows, ui.row { gap = "none", shrink = false, side(line, "old"), side(line, "new", "diff-view-split") })
						index += 1
					else
						local dels, adds = {}, {}
						while index <= #lines and lines[index].kind == "del" do
							table.insert(dels, lines[index])
							index += 1
						end
						while index <= #lines and lines[index].kind == "add" do
							table.insert(adds, lines[index])
							index += 1
						end
						for pair = 1, math.max(#dels, #adds) do
							table.insert(rows, ui.row {
								gap = "none",
								shrink = false,
								side(dels[pair], "old"),
								side(adds[pair], "new", "diff-view-split"),
							})
						end
					end
				end
			end
		end

		local status = props.status
		local header = if props.header == false
			then nil
			else ui.row {
				class = "diff-view-header",
				shrink = false,
				align = "center",
				msg = if props.path then { [props.event or "toggle"] = props.path } else nil,
				to = props.to,
				ui.icon { name = if props.collapsed then "chevron-right" else "chevron-down", class = "diff-view-chevron" },
				if status
					then ui.text {
						status,
						class = { "diff-view-status", `diff-view-status-{STATUS[status] or "modified"}` },
					}
					else nil,
				ui.text { props.path or "", class = "diff-view-path", truncate = true },
				ui.spacer(),
				diff_counts { added = added, removed = removed },
				props.switch and button_group {
					small = true,
					event = props.switch,
					to = props.to,
					selected = if props.split then "split" else "unified",
					buttons = {
						{ id = "unified", icon = "list", tooltip = "Unified" },
						{ id = "split", icon = "square-split-horizontal", tooltip = "Side by side" },
					},
				},
				table.unpack(props.actions or {}),
			}

		local scrolls = props.height ~= nil or props.grow == true
		return ui.column {
			class = "diff-view",
			grow = props.grow,
			gap = "none",
			header,
			if props.collapsed then nil else ui.column {
				class = "diff-view-body",
				gap = "none",
				height = if props.height then props.height else nil,
				grow = props.grow,
				scroll = scrolls and "both",
				table.unpack(rows),
			},
		}
	end,
}
