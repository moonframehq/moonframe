-- An inline callout: a tinted card with a tone icon, a title, text, an
-- optional action and ×. Long text can fold behind "Show more", which the
-- mod flips (it owns `expanded`).

type Props = {
	id: any,
	tone: string?,
	icon: string?,
	title: string?,
	text: string?,
	action: any,
	dismiss: any,
	foldable: boolean?,
	expanded: boolean?,
	-- Sent by Show more / Show less; default `{ toggle = id }`.
	toggle: any,
	to: string?,
	[number]: any,
}

local ICONS = {
	info = "info",
	success = "circle-check",
	warning = "triangle-alert",
	danger = "circle-alert",
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
	title = "Notice",
	category = "Feedback",
	icon = "info",
	description = "An inline tinted callout with a tone icon, title, text, an optional action and ×; long text can fold behind Show more.",
	when = "A message that belongs in the page and stays until handled: a config error, an announcement, a permission problem. For transient notifications use `toast`/`toasts`; for a failed load with Retry use `error-state`; for an empty list use `empty-state`.",
	props = {
		["id"] = "sent back in the default dismiss and toggle msgs",
		["tone"] = "`info` (default), `success`, `warning` or `danger`",
		["icon"] = "icon instead of the tone's own",
		["title"] = "bold first line",
		["text"] = "the message",
		["action"] = "a button: { label, msg, variant }",
		["dismiss"] = "msg for a × in the corner (none when unset)",
		["foldable"] = "cut `text` to one line until `expanded`, with a Show more link",
		["expanded"] = "show all of a foldable text",
		["toggle"] = "msg sent by Show more / Show less; default `{ toggle = id }`",
		["to"] = "mod the buttons send to, if not the drawing one",
		["[children]"] = "extra content under the text (a code line, a list)",
	},
	styles = {
		[".notice"] = {
			background = "accent/8",
			border = "all",
			border_color = "accent/30",
			radius = 10,
			padding = { 10, 12 },
			gap = 10,
		},
		[".notice-success"] = { background = "success/8", border_color = "success/30" },
		[".notice-warning"] = { background = "warning/10", border_color = "warning/35" },
		[".notice-danger"] = { background = "danger/8", border_color = "danger/30" },
		[".notice-icon"] = { color = "accent", margin = { top = 2 } },
		[".notice-icon-success"] = { color = "success" },
		[".notice-icon-warning"] = { color = "warning" },
		[".notice-icon-danger"] = { color = "danger" },
		[".notice-content"] = { gap = 4 },
		[".notice-title"] = { weight = "medium" },
		[".notice-text"] = { size = "small", color = "muted-foreground" },
		[".notice-more"] = { size = "small", color = "accent" },
		[".notice-more:hover"] = { color = "foreground" },
		[".notice-actions"] = { gap = 8, margin = { top = 4 } },
		[".notice-close"] = { padding = 3, radius = 6, color = "muted-foreground", size = "small" },
		[".notice-close:hover"] = { background = "list-hover", color = "foreground" },
	},
	example = {
		id = "config",
		tone = "warning",
		title = "config.toml has 2 problems",
		text = "Line 14: unknown key `sidebar.density`; it is ignored. Line 31: `theme = \"nord-light\"` is not an installed theme, so the default theme is used until you fix it or install it.",
		foldable = true,
		expanded = false,
		action = { label = "Open config.toml", msg = "open-config" },
		dismiss = "hide-config-notice",
	},

	build = function(props: Props)
		local tone = props.tone or "info"
		local id = props.id or props.title
		local folded = props.foldable == true and props.expanded ~= true

		local more: any = false
		if props.foldable then
			more = ui.row {
				msg = props.toggle or { toggle = id },
				to = props.to,
				ui.text { props.expanded and "Show less" or "Show more", class = "notice-more" },
			}
		end

		local actions: any = false
		if props.action or more then
			actions = ui.row {
				class = "notice-actions",
				align = "center",
				props.action ~= nil and ui.button {
					props.action.label,
					small = true,
					variant = (props.action.variant or "outline") :: any,
					msg = props.action.msg,
					to = props.to,
				},
				more,
			}
		end

		local content: { any } = {
			props.title ~= nil and ui.text { props.title, class = "notice-title" } or false,
			props.text ~= nil and ui.text { props.text, class = "notice-text", truncate = folded } or false,
		}
		for _, child in positional(props) do
			content[#content + 1] = child
		end
		content[#content + 1] = actions or false

		return ui.row {
			class = "notice notice-" .. tone,
			align = "start",
			ui.icon { name = (props.icon or ICONS[tone] or "info") :: any, class = "notice-icon notice-icon-" .. tone },
			ui.column { class = "notice-content", grow = true, table.unpack(content) },
			props.dismiss ~= nil and ui.row {
				class = "notice-close",
				msg = props.dismiss,
				to = props.to,
				tooltip = "Dismiss",
				ui.icon "close",
			},
		}
	end,
}
