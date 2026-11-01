-- A mail sidebar that folds to an icon rail: the button in the header (or
-- the one over the content) folds it, the avatar unfolds it, and a row
-- selects its folder. Watch the labels fade down the list as it moves.
local side_nav = use "side-nav"

-- Item lists mix optional fields; typed loosely for the checker.
local function loose(list: { any }): { any }
	return list
end

local SECTIONS: { any } = {
	{
		items = loose {
			{ id = "compose", icon = "square-pen", label = "Compose", primary = true },
			{ id = "search", icon = "search", label = "Search" },
			{ id = "calendar", icon = "calendar-days", label = "Calendar" },
			{ id = "automation", icon = "wand-sparkles", label = "Automation" },
		},
	},
	{
		title = "View",
		items = loose {
			{ id = "inbox", icon = "inbox", label = "Inbox", count = 128 },
			{ id = "starred", icon = "star", label = "Starred", count = 2 },
			{ id = "scheduled", icon = "clock", label = "Scheduled", count = 12 },
			{ id = "drafts", icon = "pencil", label = "Drafts", count = 4 },
			{ id = "sent", icon = "send", label = "Sent", count = 12 },
		},
	},
	{
		title = "Label",
		items = loose {
			{ id = "l1", dot = "#8b5cf6", label = "Label 1", count = 12 },
			{ id = "l3", dot = "#e07a45", label = "Label 3", count = 11 },
			{ id = "add-label", icon = "plus", label = "Add Label" },
		},
	},
}

return mod {
	init = function(previous)
		return previous or { collapsed = false, selected = "calendar" }
	end,

	update = function(state, msg)
		if msg == "fold" then
			state.collapsed = not state.collapsed
		elseif typeof(msg) == "table" and msg.select then
			state.selected = msg.select
		end
	end,

	view = function(state)
		return ui.row {
			height = 560,
			gap = "none",
			side_nav {
				collapsed = state.collapsed,
				toggle = "fold",
				selected = state.selected,
				account = { name = "Jay", badge = "Personal", subtitle = "Alex's Workspace", color = "#7c3aed", menu = "account" },
				sections = SECTIONS,
				footer = {
					{ id = "settings", icon = "settings", label = "Settings" },
					{ id = "support", icon = "message-square", label = "Support & Feedback" },
				} :: { any },
			},
			ui.column {
				grow = true,
				padding = "large",
				gap = "normal",
				ui.row {
					ui.button {
						if state.collapsed then "Unfold" else "Fold",
						icon = if state.collapsed then "panel-left-open" else "panel-left-close",
						variant = "outline",
						small = true,
						msg = "fold",
					},
				},
				ui.text { `Selected: {state.selected}`, tone = "muted" },
			},
		}
	end,
}
