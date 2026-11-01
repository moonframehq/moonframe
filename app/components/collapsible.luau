-- A disclosure: a clickable header (chevron, title, optional summary on the
-- right) over a body that shows only while `open`. Stateless: the mod keeps
-- `open` and flips it when the header sends `toggle`.

type Props = {
	title: string,
	open: boolean?,
	summary: string?,
	icon: string?,
	-- Sent when the header is clicked; `{ toggle = id }` by default.
	toggle: any,
	id: string?,
	to: string?,
	-- Draw it as a bordered card (the default) or flat, with no frame.
	flat: boolean?,
	[number]: any,
}

-- The children in order. `ipairs` and `#` stop at the first `nil`, which a
-- child such as `cond and ui.text "x"` leaves, and drop what follows it.
local function positional(props: any): { any }
	local keys = {}
	for key, child in props do
		if type(key) == "number" and child then
			table.insert(keys, key)
		end
	end
	table.sort(keys)
	local list = table.create(#keys)
	for _, key in keys do
		table.insert(list, props[key])
	end
	return list
end

return component {
	title = "Collapsible",
	category = "Containers",
	icon = "chevron-right",
	description = "A disclosure header with a chevron, title and summary over a body shown while open.",
	when = "One foldable block of content, such as \"No agent yet\" or advanced options. For several of them stacked use `accordion`; for a titled sidebar group with a count and actions use `sidebar-section`; for a plain heading use `section`.",
	props = {
		["title"] = "header text",
		["open"] = "show the body",
		["summary"] = "muted text on the header's right (a count, a status)",
		["icon"] = "icon before the title",
		["toggle"] = "msg sent when the header is clicked; default `{ toggle = id }`",
		["id"] = "id for the default toggle msg (default the title)",
		["to"] = "mod to send the msg to, if not the drawing one",
		["flat"] = "no card frame: just the header row and the body",
		["[children]"] = "the body",
	},
	styles = {
		[".collapsible"] = { background = "background", border = "all", border_color = "border", radius = 10 },
		[".collapsible-flat"] = { background = "#00000000", border = "none", radius = 0 },
		[".collapsible-header"] = { padding = { 10, 12 }, radius = 10, gap = 8 },
		[".collapsible-header-flat"] = { padding = { 8, 10 }, radius = 6 },
		[".collapsible-header:hover"] = { background = "list-hover" },
		[".collapsible-chevron"] = { color = "muted-foreground", size = "small" },
		[".collapsible-title"] = { weight = "medium" },
		[".collapsible-summary"] = { color = "muted-foreground", size = "small" },
		[".collapsible-body"] = { padding = { top = 0, right = 12, bottom = 12, left = 34 }, gap = 6 },
	},
	usage = [[local collapsible = use "collapsible"

collapsible { title = "No agent yet", summary = "dispatch one", open = state.agent_open,
	toggle = "toggle-agent",
	ui.text { "Dispatch an agent to work on this merge request in a new worktree.", tone = "muted" },
}]],
	example = {
		title = "No agent yet",
		summary = "dispatch one",
		open = true,
		toggle = "toggle-agent",
		ui.text { "Dispatch an agent to work on this merge request in a new worktree.", tone = "muted", size = "small" },
		ui.row { gap = "small", ui.button { "Dispatch agent", small = true, variant = "primary", msg = "dispatch" } },
	},

	build = function(props: Props)
		local open = props.open == true
		local header = ui.row {
			class = props.flat and "collapsible-header collapsible-header-flat" or "collapsible-header",
			align = "center",
			msg = props.toggle or { toggle = props.id or props.title },
			to = props.to,
			ui.icon { name = (open and "chevron-down" or "chevron-right") :: any, class = "collapsible-chevron" },
			props.icon ~= nil and ui.icon { name = props.icon :: any, class = "collapsible-icon" },
			ui.text { props.title, class = "collapsible-title", truncate = true },
			ui.spacer(),
			props.summary ~= nil and ui.text { props.summary, class = "collapsible-summary" },
		}
		local body = nil
		local children = positional(props)
		if open and #children > 0 then
			body = ui.column { class = "collapsible-body", table.unpack(children) }
		end
		return ui.column {
			class = props.flat and "collapsible collapsible-flat" or "collapsible",
			gap = "none",
			header,
			body or false,
		}
	end,
}
