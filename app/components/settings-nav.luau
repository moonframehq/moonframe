-- The left column of a settings window: an optional title, a search slot,
-- groups of pages under small uppercase headings (icon, label, an optional
-- dot for unsaved or attention), the selected one in a pill, and a footer.

type Item = {
	id: string,
	label: string,
	icon: string?,
	-- A small dot after the label: `true` (accent) or a tone name.
	dot: any,
	msg: any,
}

type Group = {
	title: string?,
	items: { Item },
}

type Props = {
	groups: { Group },
	selected: string?,
	title: string?,
	subtitle: string?,
	search: any,
	footer: any,
	event: string?,
	width: number?,
	-- Scroll the groups (give it a height, or put it in a column that fills the window).
	scroll: boolean?,
	height: number?,
	to: string?,
}

return component {
	title = "Settings nav",
	category = "Navigation",
	icon = "list",
	description = "A settings window's left column: search slot, grouped pages with icons under uppercase headings, a selected pill, a footer.",
	when = "Choosing the page of a settings or preferences window; pair it with `settings-page`, which uses it. For an app's main sidebar use `sidebar`; for a flat list use `list`; for tabs across the top use `tabs`.",
	props = {
		["groups"] = "list of { title = \"Appearance\", items = { { id, label, icon, dot, msg } } }",
		["selected"] = "id of the page shown",
		["title"] = "bold title at the top (\"Moonframe\")",
		["subtitle"] = "muted line under the title (\"Settings\")",
		["search"] = "an element at the top, such as a search input",
		["footer"] = "an element pinned at the bottom (version, a reset button)",
		["event"] = "key of the click msg; default `select`: { select = id }",
		["width"] = "pixels; default 220",
		["scroll"] = "scroll the groups when they don't fit; needs a bounded height (a filling parent or `height`)",
		["height"] = "pixels; otherwise it grows to fill its parent",
		["to"] = "mod to send clicks to, if not the drawing one",
	},
	styles = {
		[".settings-nav"] = { background = "sidebar", border = "right", border_color = "border", padding = { 14, 10 }, gap = 0 },
		[".settings-nav-head"] = { padding = { 4, 8, 12, 8 }, gap = 0 },
		[".settings-nav-title"] = { weight = "semibold", size = "large" },
		[".settings-nav-subtitle"] = { size = "small", color = "muted-foreground" },
		[".settings-nav-search"] = { padding = { 0, 0, 8, 0 } },
		[".settings-nav-groups"] = { gap = 0 },
		[".settings-nav-heading"] = {
			case = "upper",
			size = "small",
			color = "muted-foreground",
			weight = "medium",
			padding = { 12, 10, 4, 10 },
		},
		[".settings-nav-item"] = { padding = { 6, 10 }, gap = 10, radius = 8 },
		[".settings-nav-item:hover"] = { background = "list-hover" },
		[".settings-nav-item:selected"] = { background = "list-active" },
		[".settings-nav-icon"] = { color = "muted-foreground" },
		[".settings-nav-dot"] = { background = "accent", radius = 3 },
		[".settings-nav-dot-warning"] = { background = "warning" },
		[".settings-nav-dot-danger"] = { background = "danger" },
		[".settings-nav-dot-success"] = { background = "success" },
		[".settings-nav-footer"] = { padding = { 8, 8, 0, 8 }, border = "top", border_color = "border" },
	},
	example = {
		selected = "sidebar",
		groups = {
			{
				title = "Appearance",
				items = {
					{ id = "theme", label = "Theme", icon = "palette" },
					{ id = "sidebar", label = "Sidebar", icon = "panel-left" },
					{ id = "fonts", label = "Fonts", icon = "a-large-small" },
					{ id = "status-bar", label = "Status bar", icon = "panel-bottom" },
				},
			},
			{
				title = "Behaviour",
				items = {
					{ id = "general", label = "General", icon = "settings" },
					{ id = "tabs", label = "Tabs & panes", icon = "columns-2" },
					{ id = "notifications", label = "Notifications", icon = "bell", dot = "warning" },
				},
			},
			{
				title = "Connect",
				items = {
					{ id = "integrations", label = "Integrations", icon = "square-terminal" },
					{ id = "plugins", label = "Plugins", icon = "plug" },
					{ id = "cloud", label = "Cloud devices", icon = "globe" },
				},
			},
		},
	},

	build = function(props: Props)
		local event = props.event or "select"
		local children: { any } = {}
		for _, group in ipairs(props.groups or {}) do
			if group.title then
				children[#children + 1] = ui.text { group.title, class = "settings-nav-heading" }
			end
			for _, item in ipairs(group.items or {}) do
				local selected = item.id == props.selected
				local dot: any = false
				if item.dot then
					local tone = if type(item.dot) == "string" then item.dot else nil
					dot = ui.row {
						class = if tone then "settings-nav-dot settings-nav-dot-" .. tone else "settings-nav-dot",
						width = 6,
						height = 6,
					}
				end
				children[#children + 1] = ui.row {
					class = "settings-nav-item",
					shrink = false,
					selected = selected,
					align = "center",
					msg = if item.msg ~= nil then item.msg else { [event] = item.id },
					to = props.to,
					item.icon ~= nil and ui.icon {
						name = item.icon :: any,
						class = "settings-nav-icon",
						tone = if selected then "accent" else nil,
					},
					ui.text { item.label, truncate = true },
					ui.spacer(),
					dot,
				}
			end
		end

		local head: any = false
		if props.title or props.subtitle then
			head = ui.column {
				class = "settings-nav-head",
				props.title ~= nil and ui.text { props.title, class = "settings-nav-title" },
				props.subtitle ~= nil and ui.text { props.subtitle, class = "settings-nav-subtitle" },
			}
		end

		return ui.column {
			class = "settings-nav",
			width = props.width or 220,
			height = props.height,
			head,
			props.search ~= nil and ui.column { class = "settings-nav-search", props.search },
			ui.column { class = "settings-nav-groups", grow = props.scroll == true, scroll = props.scroll, table.unpack(children) },
			props.footer ~= nil and ui.column { class = "settings-nav-footer", props.footer },
		}
	end,
}
