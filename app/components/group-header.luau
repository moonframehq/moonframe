-- A heading over a group of list rows: a status dot or an icon, the title,
-- a count, a fold chevron, and a trailing `+`. For "Needs you 3" over
-- blocked agents, "Waiting 3" over merge requests, or a machine's header
-- with its online dot. Clicking the heading sends `{ toggle = id }`.

local status_dot = use "status-dot"

type Props = {
	title: string,
	id: string?,
	count: (number | string)?,
	status: string?,
	icon: string?,
	detail: string?,
	collapsed: boolean?,
	toggle: any,
	add: any,
	add_tooltip: string?,
	actions: { any }?,
	band: boolean?,
	to: string?,
}

return component {
	title = "Group header",
	category = "Lists",
	icon = "list",
	description = "A list group heading: status dot or icon, title, count, fold chevron and a trailing + action.",
	when = "A heading inside a list that groups its rows by state or owner (\"Needs you 3\", \"Waiting 3\", a machine with its online dot). For a sidebar's own sections use `sidebar-section`; for a heading over arbitrary content use `section`; for nested rows use `tree`.",
	props = {
		["title"] = "heading text",
		["id"] = "id for the default toggle msg (default the title)",
		["count"] = "number after the title",
		["status"] = "a status dot before the title: working, blocked, done, idle, unknown",
		["icon"] = "an icon before the title instead of a dot (server, laptop, agent-claude)",
		["detail"] = "muted text after the count (a host name, `online`)",
		["collapsed"] = "draw the chevron folded",
		["toggle"] = "msg sent when the heading is clicked; default `{ toggle = id }`. `false` makes it a plain heading with no chevron",
		["add"] = "msg for a trailing + button (omit for none)",
		["add_tooltip"] = "tooltip of the + button",
		["actions"] = "more elements before the +",
		["band"] = "draw it on a tinted band across the list, as over merge requests",
		["to"] = "mod to send the msgs to, if not the drawing one",
	},
	styles = {
		[".group-header"] = { padding = { 4, 8 }, gap = 4, radius = 6 },
		[".group-header-band"] = { background = "secondary", radius = 0, padding = { 6, 12 }, border = "bottom", border_color = "border" },
		[".group-header-toggle"] = { gap = 7, radius = 6 },
		[".group-header-title"] = { weight = "medium" },
		[".group-header-icon"] = { color = "muted-foreground" },
		[".group-header-count"] = { color = "muted-foreground" },
		[".group-header-detail"] = { color = "muted-foreground", size = "small" },
		[".group-header-chevron"] = { color = "muted-foreground", size = "small" },
		[".group-header-button"] = { radius = 5, padding = 3, color = "muted-foreground" },
		[".group-header-button:hover"] = { background = "list-hover", color = "foreground" },
	},
	usage = [[local group_header = use "group-header"

ui.column { gap = "none",
	group_header { title = "Needs you", status = "blocked", count = 2, collapsed = state.folded.blocked,
		toggle = { fold = "blocked" }, add = "new-agent" },
	-- the group's rows...
	group_header { title = "studio", icon = "server", status = "idle", detail = "online", count = 4, add = { new_on = "studio" } },
}]],
	example = {
		title = "Needs you",
		status = "blocked",
		count = 3,
		toggle = { fold = "blocked" },
		add = "new-agent",
		add_tooltip = "New agent",
	},

	build = function(props: Props)
		local lead = nil
		if props.icon then
			lead = ui.icon { name = props.icon :: any, class = "group-header-icon" }
		elseif props.status then
			-- Wrapped: the dot's own `self_align = "start"` would lift it to the row's top.
			lead = ui.row { align = "center", status_dot { status = props.status, size = "small" } }
		end
		local foldable = props.toggle ~= false
		local parts: { any } = {
			lead or false,
			ui.text { props.title, class = "group-header-title" },
			props.count ~= nil and ui.text { tostring(props.count), class = "group-header-count" },
			-- With an icon, the status dot goes after the title (a machine's online mark).
			props.icon ~= nil and props.status ~= nil and ui.row { align = "center", status_dot { status = props.status, size = "small" } },
			props.detail ~= nil and ui.text { props.detail, class = "group-header-detail" },
			foldable and ui.icon {
				name = (if props.collapsed then "chevron-right" else "chevron-down") :: any,
				class = "group-header-chevron",
			},
		}
		local heading = ui.row {
			class = "group-header-toggle",
			grow = true,
			align = "center",
			msg = if foldable then (props.toggle or { toggle = props.id or props.title }) else nil,
			to = props.to,
			table.unpack(parts),
		}
		local trailing = {}
		for _, action in ipairs(props.actions or {}) do
			trailing[#trailing + 1] = action
		end
		if props.add ~= nil then
			trailing[#trailing + 1] = ui.row {
				class = "group-header-button",
				msg = props.add,
				to = props.to,
				tooltip = props.add_tooltip or "Add",
				ui.icon "plus",
			}
		end
		return ui.row {
			class = if props.band then "group-header group-header-band" else "group-header",
			align = "center",
			heading,
			table.unpack(trailing),
		}
	end,
}
