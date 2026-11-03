local activity_bar = use "activity-bar"
local side_nav = use "side-nav"
local title_bar = use "title-bar"
local function loose(items: { any }): { any } return items end

return component {
	title = "Workspace shell",
	category = "Shells",
	icon = "layout-dashboard",
	description = "Give complex tools a calm, familiar home.",
	when = "Keep global navigation in the rail, local navigation in the sidebar, and the work in a generous content area. The caller owns navigation and content.",
	props = {
		["title"] = "Workspace title; use header for a custom workspace switcher.",
		["window"] = "app.window for a real window's traffic-light inset; omit when embedded.",
		["header"] = "Optional custom title-bar content.",
		["activities"] = "activity-bar items.",
		["bottom"] = "activity-bar bottom items.",
		["activity"] = "Selected activity id; sends { activity = id }.",
		["sections"] = "side-nav sections with icons, labels and counts.",
		["selected"] = "Selected sidebar id; sends { select = id }.",
		["sidebar_visible"] = "false hides the sidebar.",
		["content"] = "Main content element.",
		["height"] = "Optional fixed preview height; otherwise fills its parent.",
		["id"] = "Split id, unique for each shell in a mod.",
		["to"] = "Recipient of navigation messages.",
	},
	styles = {
		[".workspace-shell"] = { background = "background", border = "all", border_color = "border", radius = 10, clip = true },
		[".workspace-shell-content"] = { padding = 20, gap = 16 },
	},
	example = { title = "Acme Studio", height = 280, activity = "home", activities = { { id = "home", icon = "house", tooltip = "Home" }, { id = "workflows", icon = "workflow", tooltip = "Workflows" } }, selected = "inbox", sections = { { items = loose { { id = "inbox", label = "Inbox", icon = "inbox", count = 16 }, { id = "sessions", label = "Sessions", icon = "messages-square" } } } } },
	build = function(props: any)
		return ui.column { class = "workspace-shell", height = props.height, grow = props.height == nil, gap = "none",
			title_bar { window = props.window or { platform = "macos", titlebar = "native" }, title = props.title, props.header },
			ui.row { grow = true, gap = "none",
				activity_bar { items = props.activities or {}, selected = props.activity, bottom = props.bottom, to = props.to },
				ui.hsplit { id = props.id or "workspace-shell",
					ui.pane { size = 252, min = 252, max = 252, fixed = true, visible = props.sidebar_visible ~= false,
						ui.row { grow = true, gap = "none",
							side_nav { sections = props.sections or {}, selected = props.selected, to = props.to },
						},
					},
					ui.pane { ui.column { class = "workspace-shell-content", grow = true, props.content } },
				},
			},
		}
	end,
}
