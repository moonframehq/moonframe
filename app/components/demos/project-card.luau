-- A grid of projects: click the star to keep one at hand, click a card to
-- select it; a right-click names the menu it would open.
local project_card = use "project-card"

local PROJECTS: { any } = {
	{
		id = "mobile",
		name = "Mobile app v2",
		icon = "rocket",
		color = "#7c5cff",
		status = "On track",
		tone = "success",
		description = "Rebuild onboarding, offline sync and the new board view for iOS and Android.",
		progress = 0.62,
		tasks = "31/50",
		due = "Nov 28",
		updated = "Updated 2h ago",
		people = { { name = "Ana Ruiz" }, { name = "Ben Ito" }, { name = "Cleo Park" }, { name = "Dev Shah" } },
	},
	{
		id = "billing",
		name = "Billing migration",
		icon = "database",
		color = "#f08a24",
		status = "At risk",
		tone = "danger",
		description = "Move every plan to the new invoices, keep old receipts readable and refund the double charges from October.",
		progress = 0.28,
		tasks = "7/25",
		due = "Dec 4",
		updated = "Updated yesterday",
		people = { { name = "Eli Moss" }, { name = "Fay Lin" } },
	},
	{
		id = "site",
		name = "Marketing site",
		icon = "globe",
		color = "#14a38b",
		status = "Planned",
		description = "New pricing page and customer stories.",
		progress = 0,
		tasks = "0/12",
		due = "Jan 15",
		updated = "Updated 5d ago",
		people = { { name = "Gus Hale" } },
	},
}

return mod {
	init = function(previous)
		return previous or { starred = { mobile = true }, selected = "mobile", menu = nil }
	end,

	update = function(state, msg)
		if type(msg) ~= "table" then
			return
		end
		if msg.star then
			state.starred[msg.star] = not state.starred[msg.star] or nil
		elseif msg.open then
			state.selected = msg.open
		elseif msg.menu then
			state.menu = msg.menu
		end
	end,

	view = function(state)
		local cards = {}
		for index, project in PROJECTS do
			cards[index] = project_card {
				name = project.name,
				icon = project.icon,
				color = project.color,
				status = project.status,
				tone = project.tone,
				description = project.description,
				progress = project.progress,
				tasks = project.tasks,
				due = project.due,
				updated = project.updated,
				people = project.people,
				starred = state.starred[project.id] == true,
				star = { star = project.id },
				selected = state.selected == project.id,
				msg = { open = project.id },
				context = { menu = project.id },
				width = 290,
			}
		end
		local starred = {}
		for _, project in PROJECTS do
			if state.starred[project.id] then
				table.insert(starred, project.name)
			end
		end
		return ui.column {
			gap = "normal",
			ui.row { gap = "normal", wrap = true, align = "start", table.unpack(cards) },
			ui.text {
				`Starred: {if #starred > 0 then table.concat(starred, ", ") else "none"}`
					.. (if state.menu then ` · menu for {state.menu}` else ""),
				tone = "muted",
				size = "small",
			},
		}
	end,
}
