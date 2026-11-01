-- One work tree's terminals: tabs, each a tree of split panes, each pane a
-- shell started in the work tree. The shell mounts one instance per work
-- tree (`panes#<id>`), so each keeps its tabs, splits and shells while
-- another is shown, and gets them back at the next launch; with herdr-core
-- or tmux the shells themselves outlive the window too.
--
-- A tab's layout is plain data: a leaf `{ id, kind = "term", slot }` or a
-- split `{ id, kind = "row" | "column", a, b }`. A slot names the shell
-- (`t1` … `t8`, Moonframe's limit for one instance); a closed pane frees
-- its slot, and the next pane in it finds that shell again.

local icon_button = use "icon-button"

type Node = { id: string, kind: string, slot: string?, a: Node?, b: Node? }
type Tab = { id: string, title: string, root: Node, focus: string, zoom: string? }
type State = { tabs: { Tab }, selected: string?, made: number, full: boolean }
type Props = { cwd: string, name: string?, sidebar: boolean? }

local SLOTS = 8

local function leaves(node: Node, into: { Node })
	if node.kind == "term" then
		table.insert(into, node)
	else
		leaves(node.a :: Node, into)
		leaves(node.b :: Node, into)
	end
end

local function all_leaves(node: Node): { Node }
	local list = {}
	leaves(node, list)
	return list
end

-- The lowest slot no pane of any tab holds.
local function free_slot(state: State): string?
	local used = {}
	for _, tab in state.tabs do
		for _, leaf in all_leaves(tab.root) do
			used[leaf.slot :: string] = true
		end
	end
	for index = 1, SLOTS do
		if not used[`t{index}`] then
			return `t{index}`
		end
	end
	return nil
end

local function fresh(state: State, prefix: string): string
	state.made += 1
	return `{prefix}{state.made}`
end

local function current(state: State): Tab?
	for _, tab in state.tabs do
		if tab.id == state.selected then
			return tab
		end
	end
	return state.tabs[1]
end

local function new_tab(state: State)
	local slot = free_slot(state)
	if not slot then
		state.full = true
		return
	end
	local leaf = { id = fresh(state, "p"), kind = "term", slot = slot }
	local tab = { id = fresh(state, "tab"), title = `Terminal {#state.tabs + 1}`, root = leaf, focus = leaf.id }
	if #state.tabs == 0 then
		tab.title = "Terminal"
	end
	table.insert(state.tabs, tab)
	state.selected = tab.id
end

-- `node` with the leaf `id` swapped for what `swap` makes of it (nil drops it).
local function replace(node: Node, id: string, swap: (Node) -> Node?): Node?
	if node.kind == "term" then
		if node.id == id then
			return swap(node)
		end
		return node
	end
	local a = replace(node.a :: Node, id, swap)
	local b = replace(node.b :: Node, id, swap)
	if not a then
		return b
	elseif not b then
		return a
	end
	node.a, node.b = a, b
	return node
end

local function close_tab(state: State, id: string)
	for index, tab in state.tabs do
		if tab.id == id then
			table.remove(state.tabs, index)
			if state.selected == id then
				local next = state.tabs[index] or state.tabs[index - 1]
				state.selected = next and next.id
			end
			state.full = false
			return
		end
	end
end

local function split(state: State, kind: string, at: string?)
	local tab = current(state)
	if not tab then
		return new_tab(state)
	end
	local slot = free_slot(state)
	if not slot then
		state.full = true
		return
	end
	local target = at or tab.focus
	local made = { id = fresh(state, "p"), kind = "term", slot = slot }
	local root = replace(tab.root, target, function(leaf: Node): Node?
		return { id = fresh(state, "s"), kind = kind, a = leaf, b = made }
	end)
	tab.root = root :: Node
	tab.focus = made.id
	tab.zoom = nil
end

local function close_pane(state: State, at: string?)
	local tab = current(state)
	if not tab then
		return
	end
	local target = at or tab.focus
	local root = replace(tab.root, target, function(): Node?
		return nil
	end)
	state.full = false
	if not root then
		close_tab(state, tab.id)
		return
	end
	tab.root = root
	tab.zoom = nil
	if tab.focus == target then
		tab.focus = all_leaves(root)[1].id
	end
end

return mod {
	title = "Panes",

	styles = {
		[".panes-strip"] = { background = "background", border = "bottom", border_color = "border", padding = { 0, 0, 0, 0 } },
		[".panes-tab"] = { padding = { 0, 10, 0, 14 }, gap = 8, border = "right", border_color = "border", color = "muted-foreground" },
		[".panes-tab:hover"] = { background = "list-hover", color = "foreground" },
		[".panes-tab:selected"] = { background = "card", color = "foreground" },
		[".panes-tab-close"] = { color = "muted-foreground", radius = 4, padding = 2 },
		[".panes-tab-close:hover"] = { background = "list-hover", color = "foreground" },
		[".panes-count"] = { background = "secondary", radius = 5, padding = { 0, 5 }, size = "small", color = "muted-foreground" },
		[".panes-add"] = { padding = { 0, 12 }, color = "muted-foreground" },
		[".panes-add:hover"] = { color = "foreground" },
		[".panes-leaf"] = { border = "all", border_color = "background", radius = 0 },
		[".panes-leaf:focused"] = { border_color = "accent/45" },
		[".panes-head"] = { padding = { 0, 4, 0, 10 }, gap = 6, background = "sidebar", color = "muted-foreground" },
		[".panes-head:focused"] = { color = "foreground" },
		[".panes-head-title"] = { size = "small", weight = "medium" },
		[".panes-head-slot"] = { size = "small", font = "mono", color = "muted-foreground" },
		[".panes-full"] = { background = "warning/12", color = "warning", padding = { 4, 12 }, size = "small", gap = 6 },
		[".panes-empty"] = { gap = 10 },
	},

	init = function(previous: State?): State
		if previous and #previous.tabs > 0 then
			previous.full = false
			return previous
		end
		local state: State = { tabs = {}, selected = nil, made = 0, full = false }
		new_tab(state)
		return state
	end,

	update = function(state: State, msg)
		if msg == "new-tab" then
			new_tab(state)
		elseif msg == "close" then
			close_pane(state, nil)
		elseif msg == "zoom" then
			local tab = current(state)
			if tab then
				tab.zoom = if tab.zoom then nil else tab.focus
			end
		elseif type(msg) == "table" then
			local tab = current(state)
			if msg.tab then
				state.selected = msg.tab
			elseif msg.close_tab then
				close_tab(state, msg.close_tab)
			elseif msg.split then
				split(state, msg.split, msg.at)
			elseif msg.close then
				close_pane(state, msg.close)
			elseif msg.focus and tab then
				tab.focus = msg.focus
			elseif msg.zoom and tab then
				tab.focus = msg.zoom
				tab.zoom = if tab.zoom then nil else msg.zoom
			end
		end
	end,

	view = function(state: State, app, given: any)
		-- Drawn as itself (a check, the gallery) it has no work tree: home.
		local props: Props = given or { cwd = "~", name = "Home" }
		local tab = current(state)
		local window = app.window
		local inset = not props.sidebar and window.platform == "macos" and not window.fullscreen

		local strip = {}
		for _, each in state.tabs do
			local count = #all_leaves(each.root)
			local selected = tab ~= nil and each.id == tab.id
			table.insert(strip, ui.row {
				class = "panes-tab",
				key = each.id,
				selected = selected,
				align = "center",
				width = 214,
				msg = { tab = each.id },
				ui.icon { name = "square-terminal", size = "small" },
				ui.text { each.title, truncate = true },
				count > 1 and ui.text { tostring(count), class = "panes-count", tooltip = `{count} panes` },
				ui.spacer(),
				selected and ui.row {
					class = "panes-tab-close",
					msg = { close_tab = each.id },
					tooltip = "Close tab",
					ui.icon { name = "close", size = "small" },
				},
			})
		end

		local top = ui.row {
			class = "panes-strip",
			drag = true,
			height = 32,
			shrink = false,
			align = "stretch",
			gap = "none",
			inset and ui.row { width = 78 },
			table.unpack(strip),
		}
		-- The + after the tabs (unpack must come last).
		local bar = ui.row {
			class = "panes-strip",
			height = 32,
			shrink = false,
			align = "stretch",
			gap = "none",
			top,
			ui.row {
				class = "panes-add",
				align = "center",
				msg = "new-tab",
				tooltip = "New tab (⌘T)",
				ui.icon { name = "plus" },
			},
			ui.row { grow = true, drag = true },
		}

		if not tab then
			return ui.column {
				grow = true,
				gap = "none",
				bar,
				ui.column {
					class = "panes-empty",
					grow = true,
					align = "center",
					justify = "center",
					ui.icon { name = "square-terminal", size = "title", tone = "muted" },
					ui.text { "No terminals open here", tone = "muted" },
					ui.button { "New Tab", icon = "plus", small = true, variant = "outline", msg = "new-tab" },
				},
			}
		end

		local many = #all_leaves(tab.root) > 1
		local function draw(node: Node): any
			if node.kind == "term" then
				local focused = node.id == tab.focus
				local state_name = if focused and many then "focused" else nil
				return ui.column {
					class = "panes-leaf",
					state = state_name,
					grow = true,
					gap = "none",
					many and ui.row {
						class = "panes-head",
						state = state_name,
						height = 26,
						shrink = false,
						align = "center",
						ui.row {
							grow = true,
							align = "center",
							gap = "small",
							msg = { focus = node.id },
							ui.icon { name = "square-terminal", size = "small" },
							ui.text { props.name or "shell", class = "panes-head-title", truncate = true },
							ui.text { node.slot or "", class = "panes-head-slot" },
						},
						icon_button { icon = "square-split-horizontal", size = "small", tooltip = "Split right", msg = { split = "row", at = node.id } },
						icon_button { icon = "square-split-vertical", size = "small", tooltip = "Split down", msg = { split = "column", at = node.id } },
						icon_button {
							icon = if tab.zoom then "minimize" else "maximize",
							size = "small",
							tooltip = if tab.zoom then "Show all panes" else "Zoom",
							msg = { zoom = node.id },
						},
						icon_button { icon = "close", size = "small", tooltip = "Close pane", msg = { close = node.id } },
					},
					ui.terminal { id = node.slot or "t1", cwd = props.cwd, engine = "ghostty" },
				}
			end
			local a, b = draw(node.a :: Node), draw(node.b :: Node)
			if node.kind == "row" then
				return ui.hsplit { id = node.id, ui.pane { min = 160, a }, ui.pane { min = 160, b } }
			end
			return ui.vsplit { id = node.id, ui.pane { min = 90, a }, ui.pane { min = 90, b } }
		end

		local body
		if tab.zoom then
			local zoomed
			for _, leaf in all_leaves(tab.root) do
				if leaf.id == tab.zoom then
					zoomed = leaf
				end
			end
			body = draw(zoomed or tab.root)
		else
			body = draw(tab.root)
		end

		return ui.column {
			grow = true,
			gap = "none",
			bar,
			state.full and ui.row {
				class = "panes-full",
				align = "center",
				ui.icon { name = "circle-alert", size = "small" },
				ui.text { `This work tree has its {SLOTS} shells; close a pane to open another.` },
			},
			ui.column { grow = true, gap = "none", body },
		}
	end,
}
