-- Nested rows: each node indents under its parent, with a fold chevron when
-- it has children and, optionally, guide lines (│ ├ └) drawn as thin rules.
-- Stateless: the mod keeps `expanded` on each node and the selected id.
-- Clicking a row sends `{ select = id }`; clicking a chevron `{ fold = id }`.

type Node = {
	id: string,
	label: string,
	detail: string?,
	icon: string?,
	tone: string?,
	trailing: { any }?,
	children: { Node }?,
	expanded: boolean?,
	msg: any,
}

type Props = {
	nodes: { Node },
	selected: string?,
	guides: boolean?,
	select: string?,
	fold: string?,
	to: string?,
	-- Pixels per level.
	indent: number?,
}

local ROW = 26

-- One cell of guide lines for a level: `pipe` (│, an ancestor continues),
-- `tee` (├, a sibling follows), `elbow` (└, the last child), or `blank`.
local function guide(kind: string, indent: number)
	local half = indent // 2
	if kind == "blank" then
		return ui.row { width = indent }
	end
	local vertical = ui.column {
		class = "tree-guide",
		width = 1,
		height = if kind == "elbow" then ROW // 2 else ROW,
	}
	local cell = { vertical }
	if kind ~= "pipe" then
		cell[2] = ui.column {
			class = "tree-guide-arm",
			width = indent - half - 2,
			height = ROW // 2,
		}
	end
	return ui.row {
		width = indent,
		height = ROW,
		gap = "none",
		align = "start",
		style = { padding = { left = half } },
		table.unpack(cell),
	}
end

local function rows(nodes: { Node }, props: Props, prefix: { string }, out: { any })
	local indent = props.indent or 16
	for index, node in nodes do
		local last = index == #nodes
		local has_children = node.children ~= nil and #node.children > 0
		local open = node.expanded == true
		local cells = {}
		if props.guides then
			for _, kind in prefix do
				cells[#cells + 1] = guide(kind, indent)
			end
		else
			for _ = 1, #prefix do
				cells[#cells + 1] = ui.row { width = indent }
			end
		end
		-- The innermost level is this node's own branch: ├ or └ for the last.
		if props.guides and #prefix > 0 then
			cells[#cells] = guide(if last then "elbow" else "tee", indent)
		end

		local chevron
		if has_children then
			chevron = ui.row {
				class = "tree-chevron",
				width = 18,
				height = 18,
				align = "center",
				justify = "center",
				msg = { [props.fold or "fold"] = node.id },
				to = props.to,
				ui.icon { name = (if open then "chevron-down" else "chevron-right") :: any },
			}
		else
			chevron = ui.row { width = 18 }
		end

		local icon = node.icon
		if icon == nil and has_children then
			icon = if open then "folder-open" else "folder-closed"
		end
		local content = ui.row {
			class = "tree-content",
			grow = true,
			align = "center",
			height = ROW,
			msg = node.msg or { [props.select or "select"] = node.id },
			to = props.to,
			icon ~= nil and ui.icon { name = icon :: any, class = "tree-icon", tone = node.tone :: any },
			ui.text { node.label, class = "tree-label", truncate = true },
			node.detail ~= nil and ui.text { node.detail, class = "tree-detail", truncate = true },
			ui.spacer(),
			table.unpack(node.trailing or {}),
		}

		out[#out + 1] = ui.row {
			class = "tree-row",
			selected = node.id == props.selected,
			height = ROW,
			gap = "none",
			align = "center",
			ui.row { gap = "none", align = "start", height = ROW, table.unpack(cells) },
			chevron,
			content,
		}

		if has_children and open then
			local next = table.clone(prefix)
			if #next > 0 then
				next[#next] = if last then "blank" else "pipe"
			end
			next[#next + 1] = "tee"
			rows(node.children :: { Node }, props, next, out)
		end
	end
end

-- Lists in the preview mix optional fields; typed loosely for the checker.
local function loose(list: { any }): { any }
	return list
end

return component {
	title = "Tree",
	category = "Lists",
	icon = "folder-open",
	description = "Indented rows with fold chevrons and optional │ ├ └ guide lines; sends { select = id } and { fold = id }.",
	when = "Nested rows: a file tree in a review, worktrees under a repository, agents and their sub-agents. For flat rows use `list`; for a heading over a group of rows use `group-header`; for one foldable block use `collapsible`.",
	props = {
		["nodes"] = "list of { id, label, detail?, icon?, tone? (icon tone), trailing? (elements on the right), children? (nodes), expanded?, msg? }; a node with children and no icon gets a folder",
		["selected"] = "id of the selected node",
		["guides"] = "draw │ ├ └ guide lines between levels",
		["indent"] = "pixels per level (default 16)",
		["select"] = "key of the msg sent on a row click; default `select`, sending `{ select = id }`",
		["fold"] = "key of the msg sent on a chevron click; default `fold`, sending `{ fold = id }`",
		["to"] = "mod to send the msgs to, if not the drawing one",
	},
	styles = {
		[".tree"] = { gap = 1 },
		[".tree-row"] = { radius = 6, padding = { 0, 6 } },
		[".tree-row:hover"] = { background = "list-hover" },
		[".tree-row:selected"] = { background = "list-active" },
		[".tree-chevron"] = { radius = 4, color = "muted-foreground", size = "small" },
		[".tree-chevron:hover"] = { background = "muted-foreground/15", color = "foreground" },
		[".tree-content"] = { gap = 6, padding = { right = 4 } },
		[".tree-icon"] = { color = "muted-foreground" },
		[".tree-label"] = {},
		[".tree-detail"] = { color = "muted-foreground", size = "small", font = "mono" },
		[".tree-guide"] = { background = "border" },
		[".tree-guide-arm"] = { border = "bottom", border_color = "border" },
	},
	usage = [[local tree = use "tree"

tree { selected = state.selected, guides = true, nodes = {
	{ id = "src", label = "src", expanded = true, children = loose {
		{ id = "src/ledger.rs", label = "ledger.rs", icon = "file", tone = "warning",
			trailing = { ui.badge { "M", tone = "warning" } } },
		{ id = "src/main.rs", label = "main.rs", icon = "file" },
	} },
} }
-- update: msg.select opens a node; msg.fold flips its `expanded` in state.]],
	example = {
		selected = "src/ledger.rs",
		guides = true,
		nodes = loose {
			{
				id = "ledger-api",
				label = "ledger-api",
				icon = "folder-git-2",
				detail = "main",
				expanded = true,
				children = loose {
					{
						id = "src",
						label = "src",
						expanded = true,
						children = loose {
							{ id = "src/ledger.rs", label = "ledger.rs", icon = "file-text", tone = "warning", trailing = { ui.text { "+12 −3", size = "small", tone = "muted" }, ui.badge { "M", tone = "warning" } } },
							{ id = "src/currency.rs", label = "currency.rs", icon = "file-text", tone = "success", trailing = { ui.badge { "A", tone = "success" } } },
							{ id = "src/main.rs", label = "main.rs", icon = "file-text" },
						},
					},
					{ id = "tests", label = "tests", children = { { id = "tests/ledger.rs", label = "ledger.rs" } } },
					{ id = "Cargo.toml", label = "Cargo.toml", icon = "file" },
				},
			},
			{
				id = "worktrees",
				label = "worktrees",
				icon = "list-tree",
				expanded = true,
				children = loose {
					{ id = "wt-multi", label = "feat/multi-currency", detail = "~/w/ledger-multi", icon = "git-branch" },
					{ id = "wt-round", label = "fix/rounding", detail = "~/w/ledger-rounding", icon = "git-branch", trailing = { ui.badge { "needs you", tone = "danger" } } },
				},
			},
		},
	},

	build = function(props: Props)
		local out = {}
		rows(props.nodes, props, {}, out)
		return ui.column { class = "tree", table.unpack(out) }
	end,
}
