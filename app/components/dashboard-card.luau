-- A dashboard section: a tinted frame with a small uppercase heading (an
-- icon, the title, things on the right, a ⋮ menu) around a white card
-- holding the content, as finance and analytics dashboards set them out.

type Props = {
	title: string,
	icon: string?,
	meta: any,
	menu: any,
	to: string?,
	grow: boolean?,
	padding: boolean?,
	footer: any,
	[number]: any,
}

-- The children in order. `ipairs` and `#` stop at the first `nil`, which a
-- child such as `cond and ui.text "x"` leaves, and drop what follows it.
local function positional(props: any): { any }
	local keys = {}
	for key, child in props do
		if type(key) == "number" and child then
			table.insert(keys, key)
		end
	end
	table.sort(keys)
	local list = table.create(#keys)
	for _, key in keys do
		table.insert(list, props[key])
	end
	return list
end

return component {
	title = "Dashboard Card",
	category = "Containers",
	icon = "layout-dashboard",
	description = "A section of a dashboard: a tinted frame with an uppercase heading, meta and a ⋮ menu, around a white card of content.",
	when = "Each block of a dashboard or overview (a chart, a list of things waiting, a table). For a titled box inside a page use `card`; for a sidebar panel use `ui.panel`; for a tiled pane with controls use `pane-frame`.",
	props = {
		["title"] = "the heading, shown in capitals",
		["icon"] = "icon before the heading",
		["meta"] = "text or elements on the heading's right (a legend, a count, \"Closes Dec 31\")",
		["menu"] = "message the ⋮ button sends; no button without it",
		["to"] = "mod to send `menu` to, if not the drawing one",
		["grow"] = "fill the parent's free space",
		["padding"] = "false: the content touches the card's edges (a table)",
		["footer"] = "elements under the card, on the frame",
		["[children]"] = "the content",
	},
	styles = {
		[".dashboard-card"] = { background = "secondary", radius = 16, padding = { 0, 6, 6, 6 }, gap = 0, border = true, border_color = "border" },
		[".dashboard-card-head"] = { padding = { 12, 12, 10, 14 }, gap = 8 },
		[".dashboard-card-title"] = { size = "small", weight = "medium", case = "upper", color = "muted-foreground" },
		[".dashboard-card-meta"] = { size = "small", color = "muted-foreground" },
		[".dashboard-card-body"] = { background = "background", radius = 12, border = true, border_color = "border", padding = 18, gap = 14 },
		[".dashboard-card-flush"] = { padding = 0, gap = 0 },
		[".dashboard-card-foot"] = { padding = { 10, 12, 4, 12 }, gap = 8 },
	},
	example = { title = "Cash · next 13 weeks", icon = "chart-column", meta = "Synced 08:36", menu = "more" },
	usage = [[local dashboard_card = use "dashboard-card"

dashboard_card {
	title = "Waiting on you", icon = "inbox", meta = ui.badge "4", menu = "waiting-menu",
	ui.text "…the content…",
}]],

	build = function(props: Props)
		local children = positional(props)
		local meta = props.meta
		if type(meta) == "string" then
			meta = ui.text { meta, class = "dashboard-card-meta" }
		end
		return ui.column {
			class = "dashboard-card",
			grow = props.grow,
			ui.row {
				class = "dashboard-card-head",
				align = "center",
				props.icon and ui.icon { name = props.icon :: any, size = "small", tone = "muted" },
				ui.text { props.title, class = "dashboard-card-title" },
				ui.spacer(),
				meta,
				props.menu ~= nil and ui.button { icon = "ellipsis-vertical", variant = "ghost", small = true, msg = props.menu, to = props.to },
			},
			ui.column {
				class = if props.padding == false then { "dashboard-card-body", "dashboard-card-flush" } else "dashboard-card-body",
				grow = props.grow,
				table.unpack(children),
			},
			props.footer and ui.row { class = "dashboard-card-foot", align = "center", props.footer },
		}
	end,
}
