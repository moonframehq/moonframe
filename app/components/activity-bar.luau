-- A vertical strip of icon tiles at the window's edge, as in VS Code: one
-- per view (terminals, pull requests, issues, agents), the current one on an
-- accent tile, settings pinned to the bottom, and a count bubble on a tile's
-- corner. A click sends `{ activity = id }`. The starter's shell does not
-- draw one; an app's shell that wants one uses it as in `usage` below.

type Item = {
	id: string,
	icon: string,
	tooltip: string?,
	count: (number | string)?,
	msg: any,
}

type Props = {
	items: { Item },
	bottom: { Item }?,
	selected: string?,
	event: string?,
	to: string?,
	height: number?,
	fill: boolean?,
}

local function tile(item: Item, props: Props)
	local count = item.count
	if typeof(count) == "number" then
		count = if count > 99 then "99+" elseif count > 0 then tostring(count) else nil
	end
	return ui.column {
		class = "activity-bar-item",
		selected = item.id == props.selected,
		width = 36,
		height = 40,
		gap = "none",
		align = "center",
		msg = item.msg or { [props.event or "activity"] = item.id },
		to = props.to,
		tooltip = item.tooltip,
		-- The count sits in the tile's top-right corner; an empty row of the
		-- same height under the icon keeps the icon centred.
		ui.row {
			width = 36,
			height = 10,
			justify = "end",
			count ~= nil and ui.row { class = "activity-bar-count", height = 14, align = "center", ui.text { count :: any } },
		},
		ui.icon { name = item.icon :: any, class = "activity-bar-icon" },
		ui.row { height = 10 },
	}
end

return component {
	title = "Activity bar",
	category = "Navigation",
	icon = "layout-dashboard",
	description = "A vertical column of icon tiles for switching views, the current one on an accent tile, with count bubbles.",
	when = "Switching between the app's main views from a thin strip at the window's edge (terminals, pull requests, issues, agents, settings). For a collapsed list of workspaces use `rail`; for text tabs use `tabs`; for a full list of rows use `sidebar`.",
	props = {
		["items"] = "list of { id, icon, tooltip?, count? (number or text: a bubble on the corner), msg? }",
		["bottom"] = "items pinned to the bottom (settings, account), same shape",
		["selected"] = "id of the current item, drawn on an accent tile",
		["event"] = "key of the msg sent on click; default `activity`, sending `{ activity = id }`",
		["to"] = "mod to send the msg to, if not the drawing one",
		["height"] = "fixed height in pixels; in a row it stretches to the row's height",
		["fill"] = "grow to fill a column parent's height (in a `ui.pane`, say); in a row it already stretches",
	},
	styles = {
		[".activity-bar"] = { background = "sidebar", border = "right", border_color = "border", padding = { 6, 6 }, gap = 4 },
		[".activity-bar-item"] = { radius = 8, color = "muted-foreground" },
		[".activity-bar-item:hover"] = { background = "list-hover", color = "foreground" },
		[".activity-bar-item:selected"] = { background = "accent/15", color = "accent" },
		[".activity-bar-icon"] = { size = "large" },
		[".activity-bar-count"] = {
			background = "accent",
			color = "accent-foreground",
			radius = 999,
			padding = { 0, 4 },
			size = "small",
			weight = "medium",
			margin = { top = 1, right = 0 },
		},
	},
	usage = [[-- An app's main.luau: the bar picks which region the side pane shows.
-- Each view's mods declare that region (`region = "files"`), and keep
-- their state while hidden. Clicking the current view folds the pane.
local activity_bar = use "activity-bar"

local VIEWS = {
	{ id = "files", icon = "file-text", tooltip = "Explorer" },
	{ id = "search", icon = "search", tooltip = "Search" },
	{ id = "changes", icon = "git-branch", tooltip = "Changes" },
}

return mod {
	init = function(previous)
		return previous or { activity = "files", open = true }
	end,

	update = function(state, msg)
		if typeof(msg) == "table" and msg.activity then
			state.open = not (state.open and state.activity == msg.activity)
			state.activity = msg.activity
		end
	end,

	view = function(state)
		return ui.row {
			gap = "none",
			grow = true,
			activity_bar {
				items = VIEWS,
				selected = state.open and state.activity or nil,
				bottom = { { id = "settings", icon = "settings", tooltip = "Settings" } },
			},
			ui.hsplit {
				id = "body",
				ui.pane { size = 260, min = 160, max = 480, fixed = true, visible = state.open, surface = "sidebar", ui.region(state.activity) },
				ui.pane { ui.region "main" },
			},
		}
	end,
}]],
	example = {
		selected = "pulls",
		height = 440,
		items = {
			{ id = "terminals", icon = "square-terminal", tooltip = "Terminals" },
			{ id = "activity", icon = "activity", tooltip = "Activity" },
			{ id = "pulls", icon = "git-pull-request", tooltip = "Pull requests", count = 3 },
			{ id = "issues", icon = "circle-dot", tooltip = "Issues", count = 1 },
			{ id = "security", icon = "shield", tooltip = "Security" },
			{ id = "agents", icon = "bot", tooltip = "Agents" },
			{ id = "usage", icon = "chart-column", tooltip = "Usage" },
		},
		bottom = {
			{ id = "settings", icon = "settings", tooltip = "Settings" },
		},
	},

	build = function(props: Props)
		local tiles = {}
		for _, item in props.items do
			tiles[#tiles + 1] = tile(item, props)
		end
		if props.bottom and #props.bottom > 0 then
			tiles[#tiles + 1] = ui.spacer()
			for _, item in props.bottom do
				tiles[#tiles + 1] = tile(item, props)
			end
		end
		return ui.column {
			class = "activity-bar",
			align = "center",
			width = 48,
			grow = props.fill == true,
			height = props.height,
			table.unpack(tiles),
		}
	end,
}
