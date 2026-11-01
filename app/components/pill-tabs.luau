-- A view switcher where the chosen view is a wide pill with its icon, label
-- and count, and the others shrink to round icon buttons. Switching morphs:
-- the old pill narrows into a circle while its label fades, as the new one
-- widens and its label fades in. Like Inbox / All / Last 7 Days over a list.
--
-- Each item eases by its `on` and `off` states: the width comes from props
-- (inline style, which the state's tween reads), the colors and the label's
-- opacity from the rules below. The label sits in a row clipped by
-- `scroll = "x"`, so it is cut, not wrapped, while the pill is narrow; a
-- label longer than the pill has room for ends in "…".

type Item = {
	id: string,
	icon: string,
	label: string?,
	count: any,
	dot: boolean?,
	tooltip: string?,
	label_width: number?,
	msg: any,
}

type Props = {
	items: { Item },
	selected: string?,
	event: string?,
	width: number?,
	size: number?,
	to: string?,
}

-- About how wide a label is at the interface font's normal size: enough to
-- size the clip, not to lay it out (it never wraps).
local function label_width(item: Item): number
	if item.label_width then
		return item.label_width
	end
	local text = item.label or ""
	if item.count ~= nil then
		text = `{text} {item.count}`
	end
	return 8 + #text * 7.5
end

-- Lists that mix optional fields; typed loosely for the checker.
local function loose(list: { any }): { any }
	return list
end

return component {
	title = "Pill Tabs",
	category = "Navigation",
	icon = "inbox",
	description = "A view switcher whose chosen item is a wide pill with icon, label and count while the others are icon circles; switching morphs one into the other. Sends { select = id }.",
	when = "Switching the few views of one list from a sidebar or header (Inbox, All, Last 7 Days), where the current view deserves its name and the rest only an icon. For settings choices use `segmented`; for documents or sessions use `tabs`; for the app's main views use `activity-bar`.",
	props = {
		["items"] = "list of { id, icon, label?, count? (after the label), dot? (an unread dot on the circle), tooltip?, label_width? (pixels, when the estimate is off; a longer label ends in …), msg? }",
		["selected"] = "id of the current view",
		["event"] = "key of the message a click sends, default \"select\": { select = id }",
		["width"] = "the chosen pill's width in pixels (default 112)",
		["size"] = "the circles' diameter and the pills' height in pixels (default 34)",
		["to"] = "mod to send messages to, if not the drawing one",
	},
	styles = {
		[".pill-tabs"] = { gap = 6 },
		[".pill-tabs-item"] = { transition = "320ms cubic-bezier(0.32, 0.72, 0, 1)" },
		[".pill-tabs-item:off"] = { background = "muted", color = "muted-foreground" },
		[".pill-tabs-item:on"] = { background = "accent", color = "accent-foreground" },
		[".pill-tabs-idle:hover"] = { color = "foreground" },
		[".pill-tabs-label"] = { transition = "320ms cubic-bezier(0.32, 0.72, 0, 1)" },
		[".pill-tabs-label:off"] = { opacity = 0 },
		[".pill-tabs-label:on"] = { opacity = 1 },
		[".pill-tabs-count"] = { opacity = 0.7 },
		[".pill-tabs-dot"] = { background = "accent", radius = 3 },
	},
	example = {
		selected = "inbox",
		items = loose {
			{ id = "inbox", icon = "inbox", label = "Inbox", count = 1, dot = true },
			{ id = "all", icon = "copy", label = "All", count = 1 },
			{ id = "recent", icon = "clock", label = "Last 7 Days", count = 1 },
		},
	},

	build = function(props: Props)
		local event = props.event or "select"
		local size = props.size or 34
		local wide = props.width or 112
		local cells = {}
		for _, item in props.items do
			local on = item.id == props.selected
			local state = if on then "on" else "off"
			-- Room beside the icon, short of the pill's rounded end.
			local clip = math.min(label_width(item), wide - size - 6)
			cells[#cells + 1] = ui.row {
				class = if on then "pill-tabs-item" else "pill-tabs-item pill-tabs-idle",
				state = state,
				style = { width = if on then wide else size, radius = size / 2 },
				height = size,
				shrink = false,
				gap = "none",
				align = "center",
				justify = "center",
				tooltip = item.tooltip or item.label,
				msg = item.msg or { [event] = item.id },
				to = props.to,
				ui.icon { name = item.icon :: any },
				ui.row {
					class = "pill-tabs-label",
					state = state,
					style = { width = if on then clip else 0 },
					scroll = "x",
					gap = "small",
					align = "center",
					-- In the clip, so a closed pill keeps its icon centred.
					ui.row { width = 2, shrink = false },
					item.label and ui.text { item.label, truncate = true },
					item.count ~= nil and ui.text { tostring(item.count), class = "pill-tabs-count", wrap = false },
				},
				item.dot and not on and ui.row {
					class = "pill-tabs-dot",
					width = 6,
					height = 6,
					float = { top = 6, right = 6 },
				},
			}
		end
		return ui.row { class = "pill-tabs", align = "center", hug = true, table.unpack(cells) }
	end,
}
