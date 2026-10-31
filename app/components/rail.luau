-- The sidebar folded to a narrow rail: one compact tile per workspace (its
-- number, initials or an icon) with a status mark under it, the current one
-- highlighted. A click sends `{ select = id }`.

local status_dot = use "status-dot"

type Item = {
	id: string,
	label: string?,
	icon: string?,
	status: string?,
	tooltip: string?,
	msg: any,
}

type Props = {
	items: { Item },
	selected: string?,
	event: string?,
	to: string?,
	-- Elements above the tiles (an expand button) and pinned below them.
	header: { any }?,
	footer: { any }?,
	height: number?,
	fill: boolean?,
}

-- Lists in the preview mix optional fields; typed loosely for the checker.
local function loose(list: { any }): { any }
	return list
end

return component {
	title = "Rail",
	category = "Navigation",
	icon = "panel-left-close",
	description = "A collapsed sidebar: compact numbered or icon tiles with a status mark, the selected one highlighted.",
	when = "The sidebar folded to a narrow strip, one tile per workspace or agent with its status. For switching the app's views use `activity-bar`; for the full width version use `sidebar`.",
	props = {
		["items"] = "list of { id, label? (a number or two letters), icon?, status? (working, blocked, done, idle), tooltip?, msg? }",
		["selected"] = "id of the current item",
		["event"] = "key of the msg sent on click; default `select`, sending `{ select = id }`",
		["to"] = "mod to send the msg to, if not the drawing one",
		["header"] = "elements above the tiles (an expand button)",
		["footer"] = "elements pinned to the bottom (settings)",
		["height"] = "fixed height in pixels; in a row it stretches to the row's height",
		["fill"] = "grow to fill a column parent's height (in a `ui.pane`, say); in a row it already stretches",
	},
	styles = {
		[".rail"] = { background = "sidebar", border = "right", border_color = "border", padding = { 8, 6 }, gap = 4 },
		[".rail-item"] = { radius = 8, gap = 3, padding = { 6, 0, 4, 0 }, color = "muted-foreground" },
		[".rail-item:hover"] = { background = "list-hover", color = "foreground" },
		[".rail-item:selected"] = { background = "list-active", color = "foreground", border = "all", border_color = "border" },
		[".rail-label"] = { weight = "bold", font = "mono" },
	},
	usage = [[local rail = use "rail"

rail { selected = state.selected,
	header = { ui.row { class = "rail-item", width = 36, justify = "center", msg = "expand", ui.icon "panel-left-open" } },
	items = {
		{ id = "1", label = "1", status = "idle", tooltip = "ledger-api · main" },
		{ id = "2", label = "2", status = "working", tooltip = "feat/multi-currency" },
		{ id = "3", label = "3", status = "blocked", tooltip = "fix/rounding · needs you" },
	},
}
-- update: msg.select is the clicked item's id.]],
	example = {
		selected = "2",
		height = 360,
		header = { ui.row { class = "rail-item", width = 36, justify = "center", msg = "expand", tooltip = "Expand sidebar", ui.icon "panel-left-open" } },
		items = loose {
			{ id = "1", label = "1", status = "idle", tooltip = "ledger-api · main" },
			{ id = "2", label = "2", status = "working", tooltip = "feat/multi-currency" },
			{ id = "3", label = "3", status = "blocked", tooltip = "fix/rounding · needs you" },
			{ id = "4", label = "PF", status = "done", tooltip = "pixel-forge" },
			{ id = "docs", icon = "book-open", tooltip = "harbor-docs" },
		},
		footer = { ui.row { class = "rail-item", width = 36, justify = "center", msg = "settings", tooltip = "Settings", ui.icon "settings" } },
	},

	build = function(props: Props)
		local tiles = {}
		for _, child in ipairs(props.header or {}) do
			tiles[#tiles + 1] = child
		end
		for _, item in props.items do
			local face = if item.icon
				then ui.icon { name = item.icon :: any, class = "rail-icon" }
				else ui.text { item.label or string.sub(item.id, 1, 2), class = "rail-label" }
			tiles[#tiles + 1] = ui.column {
				class = "rail-item",
				selected = item.id == props.selected,
				width = 36,
				align = "center",
				msg = item.msg or { [props.event or "select"] = item.id },
				to = props.to,
				tooltip = item.tooltip,
				face,
				ui.row {
					height = 6,
					align = "center",
					item.status ~= nil and status_dot { status = item.status, size = "small" },
				},
			}
		end
		if props.footer and #props.footer > 0 then
			tiles[#tiles + 1] = ui.spacer()
			for _, child in ipairs(props.footer) do
				tiles[#tiles + 1] = child
			end
		end
		return ui.column {
			class = "rail",
			align = "center",
			width = 52,
			grow = props.fill == true,
			height = props.height,
			table.unpack(tiles),
		}
	end,
}
