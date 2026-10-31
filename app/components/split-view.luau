-- The usual window body: a sidebar on the left, the main area, an inspector
-- on the right and a bottom panel under the main area, each user-resizable.
-- It only composes `ui.hsplit`, `ui.vsplit` and `ui.pane` (splits take only
-- panes as children); the mod keeps which sides are visible.

type Props = {
	id: string?,
	sidebar: any,
	main: any,
	inspector: any,
	bottom_panel: any,
	left: boolean?,
	right: boolean?,
	bottom: boolean?,
	sidebar_size: number?,
	inspector_size: number?,
	bottom_size: number?,
	height: number?,
}

return component {
	title = "Split view",
	category = "Layout",
	icon = "columns-2",
	description = "Sidebar, main area, inspector and bottom panel in resizable splits, each side shown or hidden by a prop.",
	when = "The body of a shell or a large mod: a sidebar, the main content and optional inspector and bottom panes the user can resize and toggle. It is plain `ui.hsplit`/`ui.vsplit` with `ui.pane` children; write those directly for any other arrangement. For framing each pane use `pane-frame`; for a scrolling page inside the main area use `page`.",
	props = {
		["id"] = "prefix for the splits' ids, so the user's sizes are kept (default \"split-view\")",
		["sidebar"] = "element in the left pane (a `sidebar`, a `ui.region`)",
		["main"] = "element in the main pane",
		["inspector"] = "element in the right pane",
		["bottom_panel"] = "element in the pane under the main area (a terminal, logs)",
		["left"] = "show the sidebar (default true when given)",
		["right"] = "show the inspector (default true when given)",
		["bottom"] = "show the bottom panel (default true when given)",
		["sidebar_size"] = "sidebar width in pixels (default 260)",
		["inspector_size"] = "inspector width in pixels (default 300)",
		["bottom_size"] = "bottom panel height in pixels (default 200)",
		["height"] = "fixed height in pixels; leave unset to fill the parent, as splits do",
	},
	styles = {
		[".split-view-sidebar"] = { background = "sidebar", transition = "slide normal ease-out" },
		[".split-view-main"] = { background = "background" },
		[".split-view-inspector"] = { background = "sidebar", transition = "slide normal ease-out" },
		[".split-view-bottom"] = { background = "sidebar", transition = "slide normal ease-out" },
	},
	usage = [[local split_view = use "split-view"
local sidebar = use "sidebar"

split_view {
	left = state.left, right = state.right, bottom = state.bottom,
	sidebar = ui.region "left",
	main = ui.region "main",
	inspector = ui.region "right",
	bottom_panel = ui.terminal { id = "bottom", cwd = "~/code/ledger-api" },
}
-- update: flip state.left / state.right / state.bottom from toolbar buttons.]],
	example = {
		sidebar = ui.column { padding = "normal", gap = "small", ui.text { "WORKTREES", size = "small", tone = "muted" }, ui.item { "main", selected = true }, ui.item { "feat/multi-currency" }, ui.item { "fix/rounding" } },
		main = ui.column { padding = "large", gap = "small", ui.text { "ledger-api", size = "large", weight = "bold" }, ui.text { "Drag the dividers to resize the panes.", tone = "muted" } },
		inspector = ui.column { padding = "normal", gap = "small", ui.text { "INSPECTOR", size = "small", tone = "muted" }, ui.text { "src/ledger.rs · 2 changes" } },
		bottom_panel = ui.column { padding = "normal", ui.text { "cargo test — 2 passed", tone = "success" } },
		sidebar_size = 180,
		inspector_size = 180,
		bottom_size = 80,
		height = 300,
	},

	build = function(props: Props)
		local id = props.id or "split-view"
		local center = props.main or ui.column { grow = true }
		local main_pane
		if props.bottom_panel then
			main_pane = ui.pane {
				class = "split-view-main",
				ui.vsplit {
					id = id .. "-center",
					ui.pane { class = "split-view-main", center },
					ui.pane {
						class = "split-view-bottom",
						size = props.bottom_size or 200,
						min = 80,
						fixed = true,
						visible = props.bottom ~= false,
						props.bottom_panel,
					},
				},
			}
		else
			main_pane = ui.pane { class = "split-view-main", center }
		end
		local split = ui.hsplit {
			id = id,
			props.sidebar ~= nil and ui.pane {
				class = "split-view-sidebar",
				size = props.sidebar_size or 260,
				min = 140,
				max = 520,
				fixed = true,
				visible = props.left ~= false,
				props.sidebar,
			},
			main_pane,
			props.inspector ~= nil and ui.pane {
				class = "split-view-inspector",
				size = props.inspector_size or 300,
				min = 160,
				max = 560,
				fixed = true,
				visible = props.right ~= false,
				props.inspector,
			},
		}
		if props.height then
			return ui.column { class = "split-view", height = props.height, gap = "none", split }
		end
		return split
	end,
}
