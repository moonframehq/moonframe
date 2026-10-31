-- A path of crumbs with chevrons between them: earlier crumbs are links, the
-- last is where you are. Optional back arrow, a "2 of 3" position with up
-- and down steppers, and trailing actions, as over detail panes.

type Crumb = {
	label: string,
	icon: string?,
	id: string?,
	msg: any,
}

type Props = {
	crumbs: { Crumb },
	back: any,
	position: string?,
	up: any,
	down: any,
	actions: { any }?,
	event: string?,
	to: string?,
}

local function step(icon: string, msg: any, to: string?, tooltip: string)
	if msg == nil then
		return ui.row { class = "breadcrumbs-step breadcrumbs-step-off", ui.icon { name = icon :: any } }
	end
	return ui.row {
		class = "breadcrumbs-step",
		msg = msg,
		to = to,
		tooltip = tooltip,
		ui.icon { name = icon :: any },
	}
end

-- Lists in the preview mix optional fields; typed loosely for the checker.
local function loose(list: { any }): { any }
	return list
end

return component {
	title = "Breadcrumbs",
	category = "Navigation",
	icon = "chevron-right",
	description = "Crumbs with chevron separators, clickable earlier crumbs, an optional back arrow, \"2 of 3\" steppers and actions.",
	when = "Where a detail view sits in a hierarchy (repository › Pull requests › !2), with a way back up and to step through siblings. For a window's top row use `title-bar`; for switching views use `tabs`; for a pane's own header use `pane-frame`.",
	props = {
		["crumbs"] = "list of { label, icon?, id?, msg? }; all but the last are clickable",
		["back"] = "msg for a back arrow before the crumbs (omit for none)",
		["position"] = "text such as \"2 of 3\" before the steppers",
		["up"] = "msg for the previous sibling; nil greys the stepper out",
		["down"] = "msg for the next sibling; nil greys the stepper out",
		["actions"] = "elements on the right (open in a window, close)",
		["event"] = "key of a crumb's default msg; default `crumb`, sending `{ crumb = id or index }`",
		["to"] = "mod to send the msgs to, if not the drawing one",
	},
	styles = {
		[".breadcrumbs"] = { gap = 4, padding = { 4, 0 } },
		[".breadcrumbs-crumb"] = { gap = 5, radius = 6, padding = { 2, 6 }, color = "muted-foreground" },
		[".breadcrumbs-crumb:hover"] = { background = "list-hover", color = "foreground" },
		[".breadcrumbs-current"] = { color = "foreground", weight = "medium" },
		[".breadcrumbs-separator"] = { color = "muted-foreground/60", size = "small" },
		[".breadcrumbs-position"] = { color = "muted-foreground", size = "small", padding = { 0, 4 } },
		[".breadcrumbs-step"] = { radius = 6, padding = 5, color = "muted-foreground" },
		[".breadcrumbs-step:hover"] = { background = "list-hover", color = "foreground" },
		[".breadcrumbs-step-off"] = { opacity = 0.4 },
	},
	usage = [[local breadcrumbs = use "breadcrumbs"

breadcrumbs {
	back = "back",
	crumbs = {
		{ label = "group/sub/app", icon = "brand-gitlab", id = "repo" },
		{ label = "Merge requests", icon = "git-pull-request", id = "pulls" },
		{ label = "!2 Draft: cache MR pages" },
	},
	position = "2 of 3", up = "previous", down = "next",
	actions = { ui.row { msg = "close", ui.icon "close" } },
}
-- update: msg.crumb is the clicked crumb's id (or its index).]],
	example = {
		back = "back",
		crumbs = loose {
			{ label = "group/sub/app", icon = "brand-gitlab", id = "repo" },
			{ label = "Merge requests", icon = "git-pull-request", id = "pulls" },
			{ label = "!2 Draft: cache MR pages", icon = "git-pull-request-draft" },
		},
		position = "2 of 3",
		up = "previous",
		down = "next",
		actions = {
			ui.row { class = "breadcrumbs-step", msg = "pop-out", tooltip = "Open in a window", ui.icon "external-link" },
			ui.row { class = "breadcrumbs-step", msg = "close", tooltip = "Close", ui.icon "close" },
		},
	},

	build = function(props: Props)
		local children = {}
		if props.back ~= nil then
			children[#children + 1] = step("arrow-left", props.back, props.to, "Back")
		end
		local count = #props.crumbs
		for index, crumb in props.crumbs do
			if index > 1 then
				children[#children + 1] = ui.icon { name = "chevron-right", class = "breadcrumbs-separator" }
			end
			local parts: { any } = {
				crumb.icon ~= nil and ui.icon { name = crumb.icon :: any },
				ui.text { crumb.label, truncate = true },
			}
			if index == count then
				children[#children + 1] = ui.row {
					class = "breadcrumbs-crumb breadcrumbs-current",
					align = "center",
					grow = true,
					table.unpack(parts),
				}
			else
				children[#children + 1] = ui.row {
					class = "breadcrumbs-crumb",
					align = "center",
					shrink = false,
					msg = crumb.msg or { [props.event or "crumb"] = crumb.id or index },
					to = props.to,
					table.unpack(parts),
				}
			end
		end
		if props.position then
			children[#children + 1] = ui.text { props.position, class = "breadcrumbs-position" }
		end
		if props.position or props.up ~= nil or props.down ~= nil then
			children[#children + 1] = step("chevron-up", props.up, props.to, "Previous")
			children[#children + 1] = step("chevron-down", props.down, props.to, "Next")
		end
		for _, action in ipairs(props.actions or {}) do
			children[#children + 1] = action
		end
		return ui.row { class = "breadcrumbs", align = "center", table.unpack(children) }
	end,
}
