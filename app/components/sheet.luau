-- A sheet: a panel that slides in along one edge of the window (right by
-- default) over a dimmed window, with a header (icon, title, subtitle,
-- actions, ×) over a scrolling body and an optional footer.

type Props = {
	open: boolean?,
	dismiss: any,
	side: string?,
	title: string?,
	subtitle: string?,
	icon: string?,
	width: number?,
	height: number?,
	backdrop: boolean?,
	actions: { any }?,
	footer: any,
	keys: { [string]: any }?,
	to: string?,
	[number]: any,
}

return component {
	title = "Sheet",
	category = "Overlays",
	icon = "panel-right-open",
	description = "A panel sliding in from the left, right or bottom edge, with a header (title, actions, ×), a scrolling body and a footer.",
	when = "Side content that needs room but should keep the window in view: details, an inspector, a settings pane, logs. For a centred modal use `dialog`; for a permanent side panel use a `ui.pane` in a split; for a small floating card by a control use `ui.popover`.",
	props = {
		["open"] = "show it",
		["dismiss"] = "msg sent by Escape, a backdrop click or the ×",
		["side"] = "`right` (default), `left` or `bottom`",
		["title"] = "header title",
		["subtitle"] = "muted line under the title",
		["icon"] = "icon before the title",
		["width"] = "pixels for a left or right sheet; default 380",
		["height"] = "pixels for a bottom sheet; default 320",
		["backdrop"] = "dim the window behind; default true",
		["actions"] = "elements in the header before the ×",
		["footer"] = "an element pinned under the body (buttons, a status)",
		["keys"] = "more keys for the overlay",
		["to"] = "mod the × sends to, if not the drawing one",
		["[children]"] = "the body, which scrolls",
	},
	styles = {
		[".sheet"] = { background = "background", border = "all", border_color = "border", radius = 0, padding = 0 },
		[".sheet-header"] = { padding = { 12, 14 }, gap = 10, border = "bottom", border_color = "border" },
		[".sheet-icon"] = { color = "muted-foreground" },
		[".sheet-title"] = { weight = "bold" },
		[".sheet-subtitle"] = { size = "small", color = "muted-foreground" },
		[".sheet-close"] = { padding = 4, radius = 6, color = "muted-foreground" },
		[".sheet-close:hover"] = { background = "list-hover", color = "foreground" },
		[".sheet-body"] = { padding = 14, gap = 10 },
		[".sheet-footer"] = { padding = { 10, 14 }, gap = 8, border = "top", border_color = "border" },
	},
	usage = [[local sheet = use "sheet"

sheet { open = state.details, dismiss = "close-details", title = "Pull request #482",
	subtitle = "feature/inspect → main", icon = "git-pull-request",
	ui.text "Body content scrolls here.",
}]],
	example = {
		open = true,
		dismiss = "close",
		side = "right",
		title = "Pull request #482",
		subtitle = "feature/inspect → main",
		icon = "git-pull-request",
		actions = { ui.button { "Open", small = true, variant = "ghost", msg = "open" } },
		footer = ui.row {
			gap = "small",
			ui.spacer(),
			ui.button { "Request changes", small = true, variant = "outline", msg = "changes" },
			ui.button { "Approve", small = true, variant = "primary", msg = "approve" },
		},
		ui.text { "Add an element inspector to the shell", weight = "medium" },
		ui.text { "Picks any element under the pointer and shows its mod, classes and resolved style.", size = "small", tone = "muted" },
		ui.row {
			gap = "small",
			ui.badge { "checks passing", tone = "success" },
			ui.badge { "2 reviews", tone = "accent" },
			ui.badge { "+412 −38", tone = "muted" },
		},
	},

	build = function(props: Props)
		local side = props.side or "right"
		local header: any = false
		if props.title or props.icon or props.dismiss ~= nil then
			local children: { any } = {}
			if props.icon then
				children[#children + 1] = ui.icon { name = props.icon :: any, class = "sheet-icon" }
			end
			children[#children + 1] = ui.column {
				grow = true,
				gap = "none",
				props.title ~= nil and ui.text { props.title, class = "sheet-title", truncate = true },
				props.subtitle ~= nil and ui.text { props.subtitle, class = "sheet-subtitle", truncate = true },
			}
			for _, action in ipairs(props.actions or {}) do
				children[#children + 1] = action
			end
			if props.dismiss ~= nil then
				children[#children + 1] = ui.row {
					class = "sheet-close",
					msg = props.dismiss,
					to = props.to,
					tooltip = "Close",
					ui.icon "close",
				}
			end
			header = ui.row { class = "sheet-header", shrink = false, align = "center", table.unpack(children) }
		end

		local vertical = side == "bottom" or side == "top"
		return ui.overlay {
			open = props.open,
			dismiss = props.dismiss,
			place = side :: any,
			backdrop = props.backdrop ~= false,
			width = if vertical then nil else (props.width or 380),
			height = if vertical then (props.height or 320) else nil,
			keys = props.keys,
			to = props.to,
			class = "sheet sheet-" .. side,
			content = ui.column {
				gap = "none",
				grow = true,
				header,
				ui.column { class = "sheet-body", grow = true, scroll = true, table.unpack(props) },
				props.footer ~= nil and ui.column { class = "sheet-footer", shrink = false, props.footer },
			},
		}
	end,
}
