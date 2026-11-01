-- A notification drawer: a white panel that slides in from the right over
-- a dimmed window, with a "Mark all read" link and ×, filter tabs whose
-- selected pill slides from one to the next, and the notifications as
-- soft cards (a coloured status icon, a bold title, one truncated line, the
-- time and a coloured tag). Switching tabs fades the list over.
--
-- It keeps nothing itself: the mod keeps the items and the chosen filter,
-- and handles the messages it sends.

type Item = {
	id: any,
	title: string,
	body: string?,
	time: string?,
	level: string?,
	tag: string?,
	read: boolean?,
	msg: any,
}

type Filter = {
	id: string,
	label: string,
	-- Levels this filter shows; none means every item.
	levels: { string }?,
	count: number?,
}

type Props = {
	open: boolean?,
	items: { Item }?,
	filter: string?,
	filters: { Filter }?,
	title: string?,
	width: number?,
	top: number?,
	backdrop: boolean?,
	dismiss: any,
	mark_read: any,
	mark_read_label: string?,
	event: string?,
	remove: any,
	empty: string?,
	to: string?,
}

-- Each level's icon and tag word. Colours are in the styles, by class.
local LEVELS: { [string]: { icon: string, tag: string } } = {
	error = { icon = "circle-alert", tag = "Error" },
	success = { icon = "circle-check", tag = "Success" },
	info = { icon = "info", tag = "Info" },
	warning = { icon = "triangle-alert", tag = "Warning" },
}

local DEFAULT_FILTERS: { Filter } = {
	{ id = "all", label = "All" },
	{ id = "error", label = "Error", levels = { "error" } },
	{ id = "success", label = "Success", levels = { "success" } },
	{ id = "info", label = "Info", levels = { "info" } },
}

-- The track's padding and the drawer's side padding, in pixels: the pill's
-- width and offset are computed from them.
local SIDE = 16
local TRACK = 4

local function shows(filter: Filter?, item: Item): boolean
	if filter == nil or filter.levels == nil then
		return true
	end
	for _, level in filter.levels do
		if (item.level or "info") == level then
			return true
		end
	end
	return false
end

-- "09" for the first tab (everything), as the design pads it; plain after.
local function count_text(count: number, first: boolean): string
	if first and count < 10 then
		return string.format("%02d", count)
	end
	return tostring(count)
end

return component {
	title = "Notification Drawer",
	category = "Overlays",
	icon = "bell",
	description = "A side drawer of notifications: Mark all read and ×, filter tabs with a sliding pill and counts, and cards with a status icon, title, one line, time and a coloured tag.",
	when = "The app's notification centre, opened from a bell: a list the user filters by kind (All, Error, Success, Info), marks read and dismisses. For transient pop-ups use `toasts`; for a message that stays in the page use `notice`; for any other side panel use `sheet`.",
	props = {
		["open"] = "show it",
		["items"] = "list of { id, title, body?, time?, level? (error, success, info, warning), tag? (the tag's word; default the level's), read?, msg? (sent on a click) }, newest first",
		["filter"] = "id of the chosen filter tab (default the first)",
		["filters"] = "tabs: { id, label, levels? (the levels it shows; none shows all), count? }; default All / Error / Success / Info",
		["title"] = "header title (default \"Notification\")",
		["width"] = "pixels (default 420)",
		["top"] = "pixels to leave above it, to keep a top bar in view (default 0)",
		["backdrop"] = "dim the window behind (default true)",
		["dismiss"] = "msg sent by ×, Escape and a click outside",
		["mark_read"] = "msg the Mark all read link sends; no link without it",
		["mark_read_label"] = "the link's text (default \"Mark all read\")",
		["event"] = "key of the msg a tab sends, default \"filter\": { filter = id }",
		["remove"] = "key of the msg an item's × sends, default \"remove\": { remove = id }; false hides the ×",
		["empty"] = "text when the chosen tab has nothing (default \"You're all caught up\")",
		["to"] = "mod to send every msg to, if not the drawing one",
	},
	styles = {
		[".notification-drawer"] = {
			background = "background",
			radius = 0,
			padding = 0,
			border = "left",
			border_color = "border",
			shadow = "none",
			transition = "slide 280ms cubic-bezier(0.2, 0, 0, 1)",
		},
		[".notification-drawer-head"] = { padding = { 18, SIDE, 14, SIDE }, gap = 10 },
		[".notification-drawer-title"] = { size = "large", weight = "medium" },
		[".notification-drawer-read"] = { size = "small", color = "muted-foreground", padding = { 2, 4 }, radius = 4 },
		[".notification-drawer-read:hover"] = { color = "foreground" },
		[".notification-drawer-close"] = {
			padding = 4,
			radius = 6,
			border = true,
			border_color = "border",
			color = "muted-foreground",
			background = "background",
			shadow = "0 1px 1px #0000000a",
		},
		[".notification-drawer-close:hover"] = { color = "foreground", background = "list-hover" },
		[".notification-drawer-track"] = { background = "muted", radius = 10, margin = { 0, SIDE, 10, SIDE } },
		[".notification-drawer-pill-row"] = { padding = TRACK },
		[".notification-drawer-offset"] = { transition = "260ms cubic-bezier(0.3, 0, 0, 1)" },
		[".notification-drawer-pill"] = {
			background = "background",
			radius = 8,
			shadow = "0 1px 2px #00000014, 0 0 0 1px #0000000a",
		},
		[".notification-drawer-tabs"] = { padding = TRACK },
		[".notification-drawer-tab"] = { padding = { 7, 0 }, gap = 8, color = "muted-foreground", radius = 8 },
		[".notification-drawer-tab:hover"] = { color = "foreground" },
		[".notification-drawer-tab:selected"] = { color = "foreground" },
		[".notification-drawer-count"] = {
			size = "small",
			color = "muted-foreground",
			background = "#00000000",
			radius = 9,
			padding = { 0, 6 },
		},
		[".notification-drawer-count:selected"] = { background = "muted", color = "foreground" },
		[".notification-drawer-list"] = { padding = { 0, SIDE, SIDE, SIDE }, gap = 10 },
		[".notification-drawer-item"] = {
			background = "#f7f7f8",
			radius = 10,
			padding = { 12, 12, 12, 12 },
			gap = 12,
			transition = "fade 240ms ease-out",
		},
		[".notification-drawer-item:hover"] = { background = "#f2f2f4" },
		[".notification-drawer-read-item"] = { background = "#00000000", border = true, border_color = "border" },
		[".notification-drawer-read-item:hover"] = { background = "#fafafa" },
		[".notification-drawer-tile"] = {
			background = "background",
			radius = 7,
			border = true,
			border_color = "border",
			shadow = "0 1px 1px #0000000d",
		},
		[".notification-drawer-dot"] = { radius = 8 },
		[".notification-drawer-error"] = { background = "#ef444426", color = "#e5484d" },
		[".notification-drawer-success"] = { background = "#22c55e2e", color = "#16a34a" },
		[".notification-drawer-info"] = { background = "#4f6bed29", color = "#4f6bed" },
		[".notification-drawer-warning"] = { background = "#eab30833", color = "#b7860b" },
		[".notification-drawer-item-title"] = { weight = "medium" },
		[".notification-drawer-item-title-read"] = { weight = "normal", color = "muted-foreground" },
		[".notification-drawer-body"] = { size = "small", color = "muted-foreground" },
		[".notification-drawer-time"] = { size = "small", color = "muted-foreground" },
		[".notification-drawer-tag"] = { radius = 9, padding = { 1, 7 } },
		[".notification-drawer-tag-text"] = { size = "small" },
		[".notification-drawer-unread"] = { background = "accent", radius = 3 },
		[".notification-drawer-remove"] = { color = "#00000040", radius = 5, padding = 2 },
		[".notification-drawer-remove:hover"] = { color = "foreground", background = "#0000000a" },
		[".notification-drawer-empty"] = { padding = { 48, SIDE }, gap = 8, color = "muted-foreground" },
	},
	usage = [[local notification_drawer = use "notification-drawer"

notification_drawer {
	open = state.open, dismiss = "close", mark_read = "read-all",
	filter = state.filter, items = state.items, -- { id, title, body, time, level, read }
}
-- update: msg.filter picks a tab, msg.remove drops an item,
-- "read-all" sets every item's read, "close" sets state.open = false.]],
	example = {
		open = true,
		dismiss = "close",
		mark_read = "read-all",
		filter = "all",
		items = {
			{ id = 1, title = "CRM sync failed - auth token expired", body = "The OAuth token for the CRM has expired. Re-authenticate to resume syncing.", time = "15m ago", level = "error" },
			{ id = 2, title = "Lead scoring pipeline completed", body = "Processed and scored 284 new leads from the sales inbox.", time = "30m ago", level = "success" },
			{ id = 3, title = "New template: Chat Router", body = "A new community template is now available in the library.", time = "1h ago", level = "info", read = true },
		},
	},

	build = function(props: Props)
		local width = props.width or 420
		local filters = props.filters or DEFAULT_FILTERS
		local items: { Item } = props.items or {}
		local event = props.event or "filter"
		local remove = props.remove or "remove"

		-- The chosen tab, and what each counts.
		local index = 1
		for i, filter in filters do
			if filter.id == props.filter then
				index = i
			end
		end
		local chosen = filters[index]

		-- Each tab's width: the track's inside shared equally.
		local inside = width - 2 * SIDE - 2 * TRACK
		local cell = inside / math.max(1, #filters)

		local tabs = {}
		for i, filter in filters do
			local count = filter.count
			if count == nil then
				local n = 0
				for _, item in items do
					if shows(filter, item) then
						n += 1
					end
				end
				count = n
			end
			local on = i == index
			tabs[#tabs + 1] = ui.row {
				class = "notification-drawer-tab",
				align = "center",
				justify = "center",
				width = cell,
				selected = on,
				msg = { [event] = filter.id },
				to = props.to,
				ui.text { filter.label },
				ui.row {
					class = "notification-drawer-count",
					selected = on,
					hug = true,
					ui.text { count_text(count :: number, i == 1) },
				},
			}
		end

		-- The pill sits under the labels: a spacer before it whose width eases
		-- to the chosen tab's offset, so it slides from one tab to the next.
		local track = ui.column {
			class = "notification-drawer-track",
			shrink = false,
			ui.row {
				class = "notification-drawer-pill-row",
				float = "fill",
				gap = "none",
				ui.row {
					class = "notification-drawer-offset",
					state = `tab-{index}`,
					style = { width = (index - 1) * cell },
				},
				ui.row { class = "notification-drawer-pill", width = cell },
			},
			ui.row { class = "notification-drawer-tabs", gap = "none", table.unpack(tabs) },
		}

		-- The cards of the chosen tab, keyed by tab too, so a switch fades the
		-- old list out and the new one in.
		local cards = {}
		for _, item in items do
			if not shows(chosen, item) then
				continue
			end
			local level = item.level or "info"
			local look = LEVELS[level] or LEVELS.info
			local tone = `notification-drawer-{level}`
			cards[#cards + 1] = ui.row {
				key = `{chosen.id}-{item.id}`,
				class = if item.read then "notification-drawer-item notification-drawer-read-item" else "notification-drawer-item",
				align = "start",
				msg = item.msg,
				to = props.to,
				ui.row {
					class = "notification-drawer-tile",
					width = 26,
					height = 26,
					align = "center",
					justify = "center",
					shrink = false,
					ui.row {
						class = { "notification-drawer-dot", tone },
						width = 16,
						height = 16,
						align = "center",
						justify = "center",
						ui.icon { name = look.icon :: any, size = "small" },
					},
				},
				ui.column {
					grow = true,
					gap = "small",
					ui.row {
						align = "center",
						gap = "small",
						ui.text {
							item.title,
							class = item.read and "notification-drawer-item-title notification-drawer-item-title-read"
								or "notification-drawer-item-title",
							truncate = true,
						},
						ui.spacer(),
						not item.read and ui.row { class = "notification-drawer-unread", width = 6, height = 6 },
					},
					item.body ~= nil and ui.text { item.body, class = "notification-drawer-body", truncate = true },
					ui.row {
						align = "center",
						gap = "normal",
						item.time ~= nil and ui.text { item.time, class = "notification-drawer-time" },
						ui.row {
							class = { "notification-drawer-tag", tone },
							hug = true,
							ui.text { item.tag or look.tag, class = "notification-drawer-tag-text" },
						},
					},
				},
				remove ~= false and ui.row {
					class = "notification-drawer-remove",
					tooltip = "Dismiss",
					msg = { [remove] = item.id },
					to = props.to,
					ui.icon { name = "close", size = "small" },
				},
			}
		end
		if #cards == 0 then
			cards[1] = ui.column {
				key = `{chosen.id}-empty`,
				class = "notification-drawer-empty",
				align = "center",
				ui.icon { name = "bell-off" },
				ui.text { props.empty or "You're all caught up" },
			}
		end

		return ui.overlay {
			open = props.open,
			dismiss = props.dismiss,
			to = props.to,
			place = "right",
			backdrop = props.backdrop ~= false,
			width = width,
			class = "notification-drawer",
			style = if props.top then { margin = { top = props.top } } else nil,
			content = ui.column {
				gap = "none",
				grow = true,
				ui.row {
					class = "notification-drawer-head",
					shrink = false,
					align = "center",
					ui.text { props.title or "Notification", class = "notification-drawer-title" },
					ui.spacer(),
					props.mark_read ~= nil and ui.row {
						class = "notification-drawer-read",
						msg = props.mark_read,
						to = props.to,
						ui.text { props.mark_read_label or "Mark all read" },
					},
					props.dismiss ~= nil and ui.row {
						class = "notification-drawer-close",
						msg = props.dismiss,
						to = props.to,
						tooltip = "Close",
						ui.icon { name = "close", size = "small" },
					},
				},
				track,
				ui.column {
					class = "notification-drawer-list",
					grow = true,
					scroll = true,
					ui.column { gap = "normal", shrink = false, table.unpack(cards) },
				},
			},
		}
	end,
}
