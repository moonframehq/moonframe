-- Popover content in a workspace-popover style: a header (title,
-- subtitle, a trailing element), a grid of large icon tiles, then plain rows
-- and one destructive row. Put it in `ui.popover { content = ... }`.
-- A tile or row with an `id` sends `{ action = id }`; one with `msg` sends it.

type Tile = {
	id: string?,
	icon: string,
	label: string,
	accent: boolean?,
	disabled: boolean?,
	tooltip: string?,
	msg: any,
}

type Row = {
	id: string?,
	icon: string?,
	label: string,
	detail: string?,
	msg: any,
}

type Props = {
	title: string?,
	subtitle: string?,
	trailing: any,
	tiles: { Tile }?,
	columns: number?,
	rows: { Row }?,
	destructive: Row?,
	width: number?,
	event: string?,
	to: string?,
}

-- Lists in the preview mix optional fields; typed loosely for the checker.
local function loose(list: { any }): { any }
	return list
end

return component {
	title = "Action tiles",
	category = "Overlays",
	icon = "layout-dashboard",
	description = "Popover content: a header, a grid of large icon tiles, plain rows and a destructive row; sends { action = id }.",
	when = "The menu a workspace, worktree or agent opens on click, when its few main actions deserve big targets (new worktree, fan out, rename, close). Wrap it in `ui.popover`. For a plain list of commands use `menu` or `context-menu`; for a yes/no question use `confirm`; for picking one of several options use `choice-cards`.",
	props = {
		["title"] = "header title (the workspace's name)",
		["subtitle"] = "muted line under it (the branch)",
		["trailing"] = "element on the header's right (a status chip, an icon button)",
		["tiles"] = "list of { id, icon, label, accent? (tinted, the main action), disabled?, tooltip?, msg? }",
		["columns"] = "tiles per row (default 3)",
		["rows"] = "plain rows under the tiles: { id, icon?, label, detail? (muted, right), msg? }",
		["destructive"] = "one red row at the bottom: { id, icon?, label, msg? }",
		["width"] = "content width in pixels, for the tile sizes (default 320; match the popover's width minus its padding)",
		["event"] = "key of the default msg; default `action`, sending `{ action = id }`",
		["to"] = "mod to send the msgs to, if not the drawing one",
	},
	styles = {
		[".action-tiles"] = { gap = 8 },
		[".action-tiles-header"] = { padding = { 2, 4, 6, 4 }, gap = 8, border = "bottom", border_color = "border" },
		[".action-tiles-title"] = { weight = "bold" },
		[".action-tiles-subtitle"] = { color = "muted-foreground", size = "small", font = "mono" },
		[".action-tiles-grid"] = { gap = 6 },
		[".action-tiles-tile"] = { background = "secondary", radius = 8, gap = 6, padding = { 10, 4 } },
		[".action-tiles-tile:hover"] = { background = "list-hover" },
		[".action-tiles-tile-accent"] = { background = "accent/15", color = "accent" },
		[".action-tiles-tile-accent:hover"] = { background = "accent/25" },
		[".action-tiles-tile-disabled"] = { opacity = 0.45 },
		[".action-tiles-tile-icon"] = { size = "large" },
		[".action-tiles-tile-label"] = { size = "small" },
		[".action-tiles-row"] = { radius = 8, padding = { 7, 10 }, gap = 10 },
		[".action-tiles-row:hover"] = { background = "list-hover" },
		[".action-tiles-row-icon"] = { color = "muted-foreground" },
		[".action-tiles-row-detail"] = { color = "muted-foreground", size = "small" },
		[".action-tiles-destructive"] = { background = "danger/10", color = "danger" },
		[".action-tiles-destructive:hover"] = { background = "danger/18" },
	},
	usage = [[local action_tiles = use "action-tiles"

ui.popover { open = state.menu == id, dismiss = "close-menu", width = 344,
	content = action_tiles {
		title = "feat-multi-currency", subtitle = "feat/multi-currency",
		tiles = loose {
			{ id = "worktree", icon = "plus", label = "New worktree", accent = true },
			{ id = "fan-out", icon = "git-compare", label = "Fan out" },
			{ id = "rename", icon = "pencil", label = "Rename" },
		},
		rows = { { id = "checkpoints", icon = "history", label = "Checkpoints…" } },
		destructive = { id = "delete", icon = "trash-2", label = "Delete worktree checkout" },
	},
	ui.row { msg = { menu = id }, ui.icon "ellipsis" },
}
-- update: msg.action is the chosen tile or row id.]],
	example = {
		title = "feat-multi-currency",
		subtitle = "feat/multi-currency",
		trailing = ui.badge { "working", tone = "warning" },
		tiles = loose {
			{ id = "worktree", icon = "plus", label = "New worktree", accent = true },
			{ id = "fan-out", icon = "git-compare", label = "Fan out" },
			{ id = "rename", icon = "pencil", label = "Rename" },
			{ id = "teleport", icon = "external-link", label = "Teleport" },
			{ id = "teleport-back", icon = "arrow-down", label = "Teleport back", disabled = true },
			{ id = "close", icon = "close", label = "Close" },
		},
		rows = {
			{ id = "checkpoints", icon = "history", label = "Checkpoints…", detail = "4" },
		},
		destructive = { id = "delete", icon = "trash-2", label = "Delete worktree checkout" },
	},

	build = function(props: Props)
		local event = props.event or "action"
		local columns = math.max(1, props.columns or 3)
		local width = props.width or 320
		local gap = 6
		local tile_width = math.floor((width - gap * (columns - 1)) / columns)

		local children = {}
		if props.title or props.subtitle or props.trailing then
			children[#children + 1] = ui.row {
				class = "action-tiles-header",
				align = "center",
				ui.column {
					gap = "none",
					grow = true,
					props.title ~= nil and ui.text { props.title, class = "action-tiles-title", truncate = true },
					props.subtitle ~= nil and ui.text { props.subtitle, class = "action-tiles-subtitle", truncate = true },
				},
				props.trailing or false,
			}
		end

		local tiles = props.tiles or {}
		local grid = {}
		local index = 1
		while index <= #tiles do
			local line = {}
			for column = 0, columns - 1 do
				local tile = tiles[index + column]
				if tile == nil then
					break
				end
				local classes = { "action-tiles-tile" }
				if tile.accent then
					classes[#classes + 1] = "action-tiles-tile-accent"
				end
				if tile.disabled then
					classes[#classes + 1] = "action-tiles-tile-disabled"
				end
				local msg = nil
				if not tile.disabled then
					msg = tile.msg or (tile.id and { [event] = tile.id })
				end
				line[#line + 1] = ui.column {
					class = classes,
					width = tile_width,
					align = "center",
					justify = "center",
					msg = msg,
					to = props.to,
					tooltip = tile.tooltip,
					ui.icon { name = tile.icon :: any, class = "action-tiles-tile-icon" },
					ui.text { tile.label, class = "action-tiles-tile-label", truncate = true },
				}
			end
			grid[#grid + 1] = ui.row { class = "action-tiles-grid", table.unpack(line) }
			index += columns
		end
		if #grid > 0 then
			children[#children + 1] = ui.column { class = "action-tiles-grid", table.unpack(grid) }
		end

		local function row(entry: Row, extra: string?)
			return ui.row {
				class = if extra then { "action-tiles-row", extra } else "action-tiles-row",
				align = "center",
				msg = entry.msg or (entry.id and { [event] = entry.id }),
				to = props.to,
				entry.icon ~= nil and ui.icon { name = entry.icon :: any, class = if extra then nil else "action-tiles-row-icon" },
				ui.text { entry.label },
				ui.spacer(),
				entry.detail ~= nil and ui.text { entry.detail, class = "action-tiles-row-detail" },
			}
		end
		local rows = {}
		for _, entry in ipairs(props.rows or {}) do
			rows[#rows + 1] = row(entry)
		end
		if props.destructive then
			rows[#rows + 1] = row(props.destructive, "action-tiles-destructive")
		end
		if #rows > 0 then
			children[#children + 1] = ui.column { gap = "none", table.unpack(rows) }
		end

		return ui.column { class = "action-tiles", width = width, table.unpack(children) }
	end,
}
