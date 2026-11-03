-- A project in a grid of them: its icon on a tinted tile, the name and a
-- status pill, a star, two lines of description, a progress bar, then the
-- tasks done, the due date and the members, and when it last changed.

local avatar_stack = use "avatar-stack"
local pill = use "pill"

type Props = {
	name: string,
	icon: string?,
	color: string?,
	description: string?,
	status: string?,
	tone: string?,
	status_color: string?,
	starred: boolean?,
	star: any,
	progress: number?,
	tasks: string?,
	due: string?,
	updated: string?,
	people: { any }?,
	selected: boolean?,
	msg: any,
	context: any,
	to: string?,
	width: number?,
}

return component {
	title = "Project Card",
	category = "Data",
	icon = "layout-grid",
	description = "A project in a grid: its icon on a tinted tile, name, status pill and star, two lines of description, a progress bar, tasks, due date, members and last change.",
	when = "The projects page of a task manager, two to four cards to a row, each opening its project. For one project's row in a list use `list` or `table`; for a card on a board use `board-card`; for reading material use `guide-card`.",
	props = {
		["name"] = "the project's name",
		["icon"] = "icon on the tile (default \"layers\")",
		["color"] = "the project's colour: the icon and the tile's tint (default accent)",
		["description"] = "what it is about, cut to two lines",
		["status"] = "status text in a pill under the name (\"On track\", \"At risk\")",
		["tone"] = "the pill's tone: neutral (default), accent, success, warning or danger",
		["status_color"] = "any colour for the pill instead of a tone",
		["starred"] = "draw the star filled",
		["star"] = "message the star sends on a click; no star without it",
		["progress"] = "0 to 1: a bar and its percentage",
		["tasks"] = "tasks done of all, e.g. \"4/8\"",
		["due"] = "the due date, e.g. \"Nov 28\"",
		["updated"] = "when it last changed, e.g. \"Updated 2h ago\"",
		["people"] = "members, as `avatar-stack` takes them: list of { name, src? }",
		["selected"] = "highlight the card",
		["msg"] = "message a click sends (open the project)",
		["context"] = "message a right-click sends (a menu)",
		["to"] = "mod to send the messages to, if not the drawing one",
		["width"] = "pixels (default: grow to fill its share of the row)",
	},
	styles = {
		[".project-card"] = { radius = 12, border = true, border_color = "border", background = "background", padding = 14, gap = 12, shadow = "0 1px 2px #0000000a" },
		[".project-card:hover"] = { border_color = "muted-foreground/30" },
		[".project-card:selected"] = { border_color = "accent" },
		[".project-card-tile"] = { radius = 8 },
		[".project-card-name"] = { weight = "semibold" },
		[".project-card-star"] = { radius = 6, color = "muted-foreground/60" },
		[".project-card-star:hover"] = { background = "list-hover", color = "foreground" },
		[".project-card-starred"] = { color = "warning" },
		[".project-card-starred:hover"] = { color = "warning" },
		[".project-card-description"] = { size = "small", color = "muted-foreground" },
		[".project-card-track"] = { background = "muted-foreground/15", radius = 999 },
		[".project-card-fill"] = { radius = 999 },
		[".project-card-percent"] = { size = "small", color = "muted-foreground" },
		[".project-card-meta"] = { gap = 12, size = "small", color = "muted-foreground" },
		[".project-card-foot"] = { border = "top", border_color = "border", padding = { 10, 0, 0, 0 }, gap = 8 },
		[".project-card-updated"] = { size = "small", color = "muted-foreground" },
	},
	example = {
		name = "Mobile app v2",
		icon = "rocket",
		color = "#7c5cff",
		status = "On track",
		tone = "success",
		starred = true,
		star = "star",
		description = "Rebuild onboarding, offline sync and the new board view for iOS and Android.",
		progress = 0.62,
		tasks = "31/50",
		due = "Nov 28",
		updated = "Updated 2h ago",
		people = { { name = "Ana Ruiz" }, { name = "Ben Ito" }, { name = "Cleo Park" }, { name = "Dev Shah" } },
		width = 320,
	},
	usage = [[local project_card = use "project-card"

ui.row { gap = "large",
	project_card {
		name = "Mobile app v2", icon = "rocket", color = "#7c5cff",
		status = "On track", tone = "success", starred = project.starred, star = { star = project.id },
		description = "Rebuild onboarding, offline sync and the new board view.",
		progress = 0.62, tasks = "31/50", due = "Nov 28", updated = "Updated 2h ago",
		people = project.members, msg = { open = project.id }, context = { menu = project.id },
	},
}]],

	build = function(props: Props)
		local color = props.color or "accent"
		local meta = {}
		if props.tasks then
			table.insert(meta, ui.row {
				align = "center",
				style = { gap = 4 },
				ui.icon { name = "list-checks", size = "small" },
				ui.text { props.tasks, size = "small" },
			})
		end
		if props.due then
			table.insert(meta, ui.row {
				align = "center",
				style = { gap = 4 },
				ui.icon { name = "calendar", size = "small" },
				ui.text { props.due, size = "small" },
			})
		end
		local progress
		if props.progress then
			local value = math.clamp(props.progress, 0, 1)
			progress = ui.row {
				align = "center",
				gap = "normal",
				ui.row {
					class = "project-card-track",
					grow = true,
					height = 5,
					value > 0 and ui.row {
						class = "project-card-fill",
						area = { left = 0, top = 0, width = value, height = 1 },
						style = { background = color },
					},
				},
				ui.text { `{math.floor(value * 100 + 0.5)}%`, class = "project-card-percent" },
			}
		end
		local people = props.people
		local foot = if (people and #people > 0) or props.updated
			then ui.row {
				class = "project-card-foot",
				align = "center",
				people and #people > 0 and avatar_stack { people = people },
				ui.spacer(),
				props.updated and ui.text { props.updated, class = "project-card-updated", truncate = true },
			}
			else nil
		return ui.column {
			class = "project-card",
			width = props.width,
			grow = props.width == nil,
			selected = props.selected,
			msg = props.msg,
			context = props.context,
			to = props.to,
			ui.row {
				align = "start",
				style = { gap = 10 },
				ui.row {
					class = "project-card-tile",
					width = 34,
					height = 34,
					shrink = false,
					align = "center",
					justify = "center",
					style = { background = `{color}/14`, color = color },
					ui.icon { name = (props.icon or "layers") :: any },
				},
				ui.column {
					grow = true,
					style = { gap = 4 },
					ui.text { props.name, class = "project-card-name", truncate = true },
					props.status and ui.row {
						pill { label = props.status, tone = props.tone, color = props.status_color },
					},
				},
				props.star ~= nil and ui.row {
					class = if props.starred then { "project-card-star", "project-card-starred" } else "project-card-star",
					width = 24,
					height = 24,
					shrink = false,
					align = "center",
					justify = "center",
					msg = props.star,
					to = props.to,
					tooltip = if props.starred then "Unstar" else "Star",
					ui.icon { name = if props.starred then "star-fill" else "star" },
				},
			},
			props.description and ui.text { props.description, class = "project-card-description", lines = 2 },
			progress,
			#meta > 0 and ui.row { class = "project-card-meta", align = "center", table.unpack(meta) },
			foot,
		}
	end,
}
