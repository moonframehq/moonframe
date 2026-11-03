local avatar = use "avatar"
local menu = use "menu"

return component {
	title = "Workspace switcher",
	category = "Navigation",
	icon = "chevrons-up-down",
	description = "Switch context without losing your place.",
	when = "Show the current workspace before offering alternatives. Keep context switching separate from workspace creation, and keep menu state in the caller.",
	props = {
		["name"] = "Current workspace name.",
		["selected"] = "Current workspace id.",
		["workspaces"] = "List of { id, label, detail? }.",
		["open"] = "Whether the menu is open.",
		["toggle"] = "Message that toggles the menu.",
		["dismiss"] = "Message on Escape or outside click.",
		["create"] = "Optional message for Create workspace.",
		["highlighted"] = "Keyboard-highlighted row id.",
		["keys"] = "Popover key messages; caller handles up, down and enter.",
		["event"] = "Key sent when selecting a workspace; default workspace.",
		["to"] = "Message recipient, optional.",
	},
	styles = {
		[".workspace-switcher-trigger"] = { padding = { 8, 10 }, gap = 10, radius = 8 },
		[".workspace-switcher-trigger:hover"] = { background = "list-hover" },
	},
	example = { name = "Acme Studio", selected = "acme", workspaces = { { id = "acme", label = "Acme Studio" }, { id = "personal", label = "Personal" } } },
	build = function(props: any)
		local items: { any } = {}
		for _, workspace in props.workspaces or {} do
			table.insert(items, { id = workspace.id, label = workspace.label, detail = workspace.detail, avatar = workspace.label, checked = workspace.id == props.selected })
		end
		if props.create then
			table.insert(items, "separator")
			table.insert(items, { label = "Create workspace", icon = "plus", msg = props.create })
		end
		return ui.popover {
			open = props.open, dismiss = props.dismiss, keys = props.keys, to = props.to, width = 280,
			content = menu { items = items, highlighted = props.highlighted, event = props.event or "workspace", to = props.to },
			ui.row { class = "workspace-switcher-trigger", align = "center", msg = props.toggle, to = props.to,
				avatar { name = props.name or "Workspace", shape = "square" },
				ui.text { props.name or "Workspace", weight = "medium", truncate = true }, ui.spacer(), ui.icon "chevron-down",
			},
		}
	end,
}
