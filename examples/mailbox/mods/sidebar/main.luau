-- The sidebar: the account, actions, folders with counts, labels and the
-- rest, folding to an icon rail. The chosen folder goes out on the topic
-- `mail/folder` for the message list.

local side_nav = use "side-nav"

type State = { collapsed: boolean, selected: string }

-- Item lists mix optional fields; typed loosely for the checker.
local function loose(list: { any }): { any }
	return list
end

local SECTIONS: { any } = {
	{ items = loose {
		{ id = "compose", icon = "square-pen", label = ui.t "folder-compose", primary = true },
		{ id = "search", icon = "search", label = ui.t "folder-search" },
		{ id = "calendar", icon = "calendar-days", label = ui.t "folder-calendar" },
		{ id = "automation", icon = "wand-sparkles", label = ui.t "folder-automation" },
	} },
	{ title = ui.t "section-view", items = loose {
		{ id = "inbox", icon = "inbox", label = ui.t "folder-inbox", count = 128 },
		{ id = "starred", icon = "star", label = ui.t "folder-starred", count = 2 },
		{ id = "scheduled", icon = "clock", label = ui.t "folder-scheduled", count = 12 },
		{ id = "drafts", icon = "pencil", label = ui.t "folder-drafts", count = 4 },
		{ id = "sent", icon = "send", label = ui.t "folder-sent", count = 12 },
		{ id = "spam", icon = "triangle-alert", label = ui.t "folder-spam", count = 230 },
		{ id = "trash", icon = "trash", label = ui.t "folder-trash", count = 412 },
	} },
	{ title = ui.t "section-label", items = loose {
		{ id = "label-1", dot = "#8b5cf6", label = ui.t("label-name", { number = 1 }), count = 12 },
		{ id = "label-3", dot = "#e07a45", label = ui.t("label-name", { number = 3 }), count = 11 },
		{ id = "label-4", dot = "#7fa83a", label = ui.t("label-name", { number = 4 }), count = 12 },
		{ id = "add-label", icon = "plus", label = ui.t "label-add" },
	} },
	{ title = ui.t "section-other", items = loose {
		{ id = "settings", icon = "settings", label = ui.t "folder-settings" },
		{ id = "support", icon = "message-square", label = ui.t "folder-support" },
	} },
}

return mod {
	title = "Sidebar",

	init = function(previous: State?): (State, { any })
		local state = previous or { collapsed = false, selected = "inbox" }
		return state, { { to = "topic:mail/folder", msg = state.selected } }
	end,

	update = function(state: State, msg)
		if msg == "fold" then
			state.collapsed = not state.collapsed
		elseif typeof(msg) == "table" and msg.select then
			state.selected = msg.select
			return state, { { to = "topic:mail/folder", msg = msg.select } }
		end
		return state
	end,

	view = function(state: State)
		return side_nav {
			-- No `toggle`: the title bar's button folds it, so the head has none.
			collapsed = state.collapsed,
			selected = state.selected,
			account = { name = "Jay", badge = ui.t "account-personal", subtitle = ui.t("account-subtitle", { name = "Alex" }), color = "#7c3aed", menu = "account" },
			sections = SECTIONS,
		}
	end,
}
