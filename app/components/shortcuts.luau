-- A keyboard shortcut reference: a large modal whose groups of
-- { keys, label } are laid out in columns, each shortcut drawn as key caps
-- (`kbd`). An optional search slot sits over the groups.

local dialog = use "dialog"
local kbd = use "kbd"

type Item = {
	keys: any,
	label: string,
	msg: any,
}

type Group = {
	title: string,
	items: { Item },
}

type Props = {
	open: boolean?,
	dismiss: any,
	title: string?,
	subtitle: string?,
	groups: { Group },
	columns: number?,
	search: any,
	note: string?,
	width: number?,
	height: number?,
	-- Draw it inline (no overlay), e.g. in a settings page.
	inline: boolean?,
	to: string?,
}

return component {
	title = "Shortcuts",
	category = "Overlays",
	icon = "keyboard",
	description = "A keyboard shortcut reference: groups of key caps and labels laid out in columns in a large modal.",
	when = "Show the app's or a mod's keybindings (a ? or ⌘/ help overlay, a Shortcuts page). For one shortcut use `kbd`; for a line of hints under a list or palette use `key-hints`; for a command palette use `dialog` with `place = \"top\"`.",
	props = {
		["open"] = "show it",
		["dismiss"] = "msg for Escape, the × and backdrop clicks",
		["title"] = "default \"Keyboard Shortcuts\"",
		["subtitle"] = "muted line under the title",
		["groups"] = "list of { title, items = { { keys = \"cmd-shift-p\", label, msg } } }; a `msg` makes the row clickable",
		["columns"] = "number of columns; default 2",
		["search"] = "an element over the groups, such as a search input",
		["note"] = "footer text; default \"esc to close\"",
		["width"] = "pixels; default 720",
		["height"] = "pixels for the scrolling body",
		["inline"] = "draw the groups in place, without the modal",
		["to"] = "mod the rows send to, if not the drawing one",
	},
	styles = {
		[".shortcuts-columns"] = { gap = 28 },
		[".shortcuts-column"] = { gap = 18 },
		[".shortcuts-group"] = { gap = 2 },
		[".shortcuts-group-title"] = { case = "upper", size = "small", weight = "bold", color = "accent", padding = { 0, 0, 6, 0 } },
		[".shortcuts-row"] = { padding = { 4, 6 }, gap = 12, radius = 6, border = "bottom", border_color = "border/60" },
		[".shortcuts-row:hover"] = { background = "list-hover" },
		[".shortcuts-label"] = {},
		[".shortcuts-none"] = { size = "small", color = "muted-foreground" },
	},
	example = {
		open = true,
		dismiss = "close",
		width = 600,
		subtitle = "View and edit shortcuts",
		groups = {
			{
				title = "Workspaces & panes",
				items = {
					{ keys = "cmd-shift-n", label = "New workspace" },
					{ keys = "cmd-t", label = "New tab" },
					{ keys = "cmd-d", label = "Split right" },
					{ keys = "cmd-shift-d", label = "Split down" },
					{ keys = "cmd-w", label = "Close pane" },
				},
			},
			{
				title = "Navigation",
				items = {
					{ keys = "cmd-k", label = "Command palette" },
					{ keys = "cmd-p", label = "Go to workspace" },
					{ keys = "cmd-alt-left", label = "Previous pane" },
					{ keys = "cmd-alt-right", label = "Next pane" },
				},
			},
			{
				title = "Agents",
				items = {
					{ keys = "cmd-shift-a", label = "Start agent" },
					{ keys = "cmd-k cmd-r", label = "Review changes" },
					{ keys = "", label = "Edit worktree note" },
				},
			},
		},
	},

	build = function(props: Props)
		local columns = math.max(props.columns or 2, 1)
		local buckets: { { any } } = {}
		local sizes: { number } = {}
		for index = 1, columns do
			buckets[index] = {}
			sizes[index] = 0
		end

		for _, group in ipairs(props.groups or {}) do
			local rows: { any } = { ui.text { group.title, class = "shortcuts-group-title" } }
			for _, item in ipairs(group.items or {}) do
				local has_keys = item.keys ~= nil and item.keys ~= "" and not (type(item.keys) == "table" and #item.keys == 0)
				rows[#rows + 1] = ui.row {
					class = "shortcuts-row",
					shrink = false,
					align = "center",
					msg = item.msg,
					to = props.to,
					ui.text { item.label, class = "shortcuts-label", truncate = true },
					ui.spacer(),
					if has_keys then kbd { keys = item.keys, small = true } else ui.text { "Not set", class = "shortcuts-none" },
				}
			end
			-- The shortest column takes the next group, so columns stay even.
			local target = 1
			for index = 2, columns do
				if sizes[index] < sizes[target] then
					target = index
				end
			end
			local bucket = buckets[target]
			bucket[#bucket + 1] = ui.column { class = "shortcuts-group", shrink = false, table.unpack(rows) }
			sizes[target] += #rows + 2
		end

		local cols = {}
		for index, bucket in buckets do
			cols[index] = ui.column { class = "shortcuts-column", grow = true, table.unpack(bucket) }
		end
		local grid = ui.row { class = "shortcuts-columns", align = "start", table.unpack(cols) }

		if props.inline then
			return ui.column { gap = "large", props.search or false, grid }
		end
		return dialog {
			open = props.open,
			dismiss = props.dismiss,
			title = props.title or "Keyboard Shortcuts",
			subtitle = props.subtitle,
			icon = "keyboard",
			width = props.width or 720,
			height = props.height,
			note = props.note or "esc to close · click outside to dismiss",
			to = props.to,
			props.search or false,
			grid,
		}
	end,
}
