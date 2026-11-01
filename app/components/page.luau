-- A scrolling page for settings and detail views: a title, a subtitle and
-- header actions over the content, which can be held to a fixed width, and
-- an optional footer pinned under the scroll (an "Unsaved" bar).

type Props = {
	title: string,
	subtitle: string?,
	icon: string?,
	actions: { any }?,
	width: number?,
	footer: any,
	height: number?,
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
	title = "Page",
	category = "Layout",
	icon = "file-text",
	description = "A scrolling page with a title, subtitle and header actions over its content, and an optional pinned footer.",
	when = "The main area of a settings screen, a detail view or a dashboard: a big title, then sections of content that scroll. Group its content with `section` or `card`; put it beside a `sidebar` in a `split-view`. For a framed pane use `pane-frame`; for a modal use `dialog`; for a panel in a sidebar use `ui.panel`.",
	props = {
		["title"] = "page title",
		["subtitle"] = "muted line under the title",
		["icon"] = "icon before the title",
		["actions"] = "elements on the header's right (buttons)",
		["width"] = "the content's width in pixels, centred (omit to fill the page)",
		["footer"] = "element pinned under the scroll (a save bar)",
		["height"] = "fixed height in pixels; leave unset to fill the parent",
		["[children]"] = "the content",
	},
	styles = {
		[".page"] = { background = "background", gap = 0 },
		[".page-scroll"] = { padding = { 28, 32 } },
		[".page-content"] = { gap = 20 },
		[".page-header"] = { gap = 12, padding = { bottom = 4 } },
		[".page-title"] = { size = "title", weight = "bold" },
		[".page-icon"] = { size = "title", color = "muted-foreground" },
		[".page-subtitle"] = { color = "muted-foreground" },
		[".page-footer"] = { background = "sidebar", border = "top", border_color = "border", padding = { 8, 16 }, gap = 8 },
	},
	usage = [[local page = use "page"
local section = use "section"

page { title = "Sidebar", subtitle = "Changes apply to every open window immediately.",
	actions = { ui.button { "Reset", small = true, variant = "ghost", msg = "reset" } },
	width = 720,
	footer = ui.row { align = "center", gap = "normal",
		ui.text { "Unsaved: sidebar", tone = "warning" }, ui.spacer(),
		ui.button { "Save now", small = true, variant = "outline", msg = "save" },
	},
	section { title = "LAYOUT", ui.text "Layout preset" },
	section { title = "SPACING", ui.text "Indent" },
}]],
	example = {
		title = "Sidebar",
		subtitle = "Changes apply to every open window immediately.",
		actions = { ui.button { "Reset", small = true, variant = "ghost", msg = "reset" } },
		height = 340,
		footer = ui.row {
			align = "center",
			gap = "normal",
			ui.text { "Unsaved: sidebar — saved when this window closes", size = "small", tone = "warning" },
			ui.spacer(),
			ui.button { "Save now", small = true, variant = "outline", msg = "save" },
			ui.button { "Reload", small = true, variant = "outline", msg = "reload" },
		},
		ui.column {
			gap = "small",
			ui.text { "LAYOUT", size = "small", tone = "muted", weight = "medium" },
			ui.divider(),
			ui.row { align = "center", ui.column { gap = "none", grow = true, ui.text "Layout preset", ui.text { "Row shape and density.", size = "small", tone = "muted" } }, ui.button { "Comfortable Rounded", small = true, variant = "outline", msg = "preset" } },
			ui.divider(),
			ui.row { align = "center", ui.column { gap = "none", grow = true, ui.text "Show agents", ui.text { "List running agents under their workspace.", size = "small", tone = "muted" } }, ui.badge { "on", tone = "accent" } },
		},
	},

	build = function(props: Props)
		local header = ui.row {
			class = "page-header",
			align = "start",
			props.icon ~= nil and ui.icon { name = props.icon :: any, class = "page-icon" },
			ui.column {
				gap = "small",
				grow = true,
				ui.text { props.title, class = "page-title" },
				props.subtitle ~= nil and ui.text { props.subtitle, class = "page-subtitle" },
			},
			table.unpack(props.actions or {}),
		}
		local content = ui.column {
			class = "page-content",
			width = props.width,
			header,
			table.unpack(positional(props)),
		}
		local scroll = ui.column {
			class = "page-scroll",
			grow = true,
			scroll = true,
			align = if props.width then "center" else "stretch",
			content,
		}
		return ui.column {
			class = "page",
			grow = props.height == nil,
			height = props.height,
			scroll,
			props.footer ~= nil and ui.column { class = "page-footer", shrink = false, props.footer },
		}
	end,
}
