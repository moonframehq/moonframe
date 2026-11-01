-- A writing app's body: a sidebar, a list beside it, and the main area.
-- Hiding the sidebar or the list folds it away to the left while it fades,
-- and the panes to its right slide over into the room it leaves. Each side
-- keeps its content at full width while it moves, so its text is cut, not
-- wrapped again on every frame.

type Props = {
	id: string?,
	sidebar: any,
	list: any,
	main: any,
	left: boolean?,
	middle: boolean?,
	sidebar_size: number?,
	list_size: number?,
	height: number?,
}

return component {
	title = "Side Panels",
	category = "Layout",
	icon = "panel-left",
	description = "Sidebar, list and main area side by side; hiding the sidebar or the list folds it away while it fades, and the rest slides over.",
	when = "A three-column app (notes, mail, writing) whose sidebar and list the user hides to focus on the main area. The mod keeps `left` and `middle` and flips them from buttons in the panes' headers. For resizable panes, an inspector or a bottom panel use `split-view`; for a switcher over the list use `pill-tabs`.",
	props = {
		["id"] = "the split's id (default \"side-panels\")",
		["sidebar"] = "element in the left pane",
		["list"] = "element in the middle pane",
		["main"] = "element in the main area",
		["left"] = "show the sidebar (default true)",
		["middle"] = "show the list (default true)",
		["sidebar_size"] = "sidebar width in pixels (default 220)",
		["list_size"] = "list width in pixels (default 280)",
		["height"] = "fixed height in pixels; leave unset to fill the parent",
	},
	styles = {
		[".side-panels-sidebar"] = {
			background = "sidebar",
			transition = "slide-fade 360ms cubic-bezier(0.32, 0.72, 0, 1)",
		},
		[".side-panels-list"] = {
			background = "background",
			border = "left",
			border_color = "border",
			transition = "slide-fade 360ms cubic-bezier(0.32, 0.72, 0, 1)",
		},
		[".side-panels-main"] = { background = "background" },
	},
	usage = [[local side_panels = use "side-panels"

side_panels {
	left = state.left, middle = state.middle,
	sidebar = ui.region "left",
	list = ui.region "list",
	main = ui.region "main",
}
-- update: flip state.left / state.middle from icon buttons in the panes' headers.]],
	example = {
		sidebar = ui.column { padding = "normal", ui.text { "Neverland.", weight = "bold", size = "large" } },
		list = ui.column { padding = "normal", ui.text { "Inbox", weight = "bold" } },
		main = ui.column { padding = "large", ui.text { "# Hello world!", size = "title", weight = "bold" } },
		sidebar_size = 160,
		list_size = 180,
		height = 220,
	},

	build = function(props: Props)
		local sidebar_size = props.sidebar_size or 220
		local list_size = props.list_size or 280
		-- The content keeps the pane's width; the pane clips it as it folds.
		local function pinned(size: number, content: any)
			return ui.column { width = size, shrink = false, grow = true, gap = "none", content }
		end
		local split = ui.hsplit {
			id = props.id or "side-panels",
			divider = false,
			props.sidebar ~= nil and ui.pane {
				class = "side-panels-sidebar",
				size = sidebar_size,
				min = sidebar_size,
				max = sidebar_size,
				fixed = true,
				visible = props.left ~= false,
				pinned(sidebar_size, props.sidebar),
			},
			props.list ~= nil and ui.pane {
				class = "side-panels-list",
				size = list_size,
				min = list_size,
				max = list_size,
				fixed = true,
				visible = props.middle ~= false,
				pinned(list_size, props.list),
			},
			ui.pane { class = "side-panels-main", props.main or ui.column { grow = true } },
		}
		if props.height then
			return ui.column { height = props.height, gap = "none", split }
		end
		return split
	end,
}
