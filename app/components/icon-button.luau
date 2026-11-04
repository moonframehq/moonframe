-- A square, icon-only button with a tooltip, in three looks and two sizes.
-- `active` shows it pressed (a toggled sidebar, a chosen tool); `badge`
-- adds a small count beside the icon.

type Props = {
	icon: string,
	tooltip: string?,
	msg: any,
	command: any,
	variant: string?,
	size: string?,
	active: boolean?,
	badge: any,
	disabled: boolean?,
	tone: string?,
	to: string?,
}

local VARIANTS = { ghost = true, outline = true, solid = true }

return component {
	title = "Icon Button",
	category = "Controls",
	icon = "settings",
	description = "A square icon-only button with a tooltip: ghost, outline or solid, small or normal, optionally pressed or with a count.",
	when = "An action or toggle shown only as an icon: in a title bar, a panel header, a toolbar. Always give a `tooltip`. For buttons with words use `ui.button` or `toolbar`; for back/forward use `nav-buttons`; for joined icon choices use `button-group`.",
	props = {
		["icon"] = "icon name",
		["tooltip"] = "text shown under the pointer (also its name for the user)",
		["msg"] = "message a click sends",
		["command"] = "an app command to run instead of `msg`",
		["variant"] = "\"ghost\" (default), \"outline\" or \"solid\"",
		["size"] = "\"normal\" (32px, default) or \"small\" (26px)",
		["active"] = "show it pressed, for a toggle that is on",
		["badge"] = "a small count or dot text beside the icon",
		["disabled"] = "greys it out and ignores clicks",
		["tone"] = "\"danger\" for a destructive action",
		["to"] = "mod to send `msg` to, if not the drawing one",
	},
	styles = {
		[".icon-button"] = { radius = 8, gap = 4, color = "muted-foreground" },
		[".icon-button:hover"] = { background = "list-hover", color = "foreground" },
		[".icon-button:selected"] = { background = "accent/15", color = "accent" },
		[".icon-button-outline"] = { border = "all", border_color = "border", background = "background" },
		[".icon-button-outline:selected"] = { border_color = "accent/50" },
		[".icon-button-solid"] = { background = "secondary", color = "foreground" },
		[".icon-button-solid:hover"] = { background = "muted-foreground/25" },
		[".icon-button-solid:selected"] = { background = "accent", color = "accent-foreground" },
		[".icon-button-small"] = { radius = 6, size = "small" },
		[".icon-button-danger"] = { color = "danger" },
		[".icon-button-danger:hover"] = { background = "danger/12", color = "danger" },
		[".icon-button-disabled"] = { opacity = 0.4 },
		[".icon-button-badge"] = {
			padding = { 0, 4 },
			radius = 999,
			background = "accent",
			color = "accent-foreground",
			size = "small",
			weight = "medium",
		},
	},
	example = { icon = "bell", tooltip = "Notifications", msg = "notifications", variant = "outline", badge = 3 },
	usage = [[local icon_button = use "icon-button"

icon_button { icon = "panel-left", tooltip = "Toggle sidebar", msg = "sidebar", active = state.sidebar }
icon_button { icon = "bell", tooltip = "Notifications", msg = "notifications", variant = "outline", badge = 3 }
icon_button { icon = "delete", tooltip = "Delete", msg = "delete", size = "small", tone = "danger" }]],

	build = function(props: Props)
		local small = props.size == "small"
		local side = if small then 26 else 32
		local class = { "icon-button" }
		local variant = if props.variant and VARIANTS[props.variant] then props.variant else "ghost"
		if variant ~= "ghost" then
			table.insert(class, "icon-button-" .. variant)
		end
		if small then
			table.insert(class, "icon-button-small")
		end
		if props.tone == "danger" then
			table.insert(class, "icon-button-danger")
		end
		if props.disabled then
			table.insert(class, "icon-button-disabled")
		end
		local badge = props.badge ~= nil
		-- Wrapped so a stretching parent does not widen the button.
		return ui.row { gap = "none", ui.row {
			class = class,
			width = if badge then nil else side,
			height = side,
			style = if badge then { padding = { x = if small then 6 else 8 } } else nil,
			align = "center",
			justify = "center",
			selected = props.active == true,
			tooltip = props.tooltip,
			msg = if props.disabled then nil else props.msg,
			command = if props.disabled then nil else props.command,
			to = if props.disabled then nil else props.to,
			ui.icon { name = props.icon :: any },
			badge and ui.row { class = "icon-button-badge", ui.text { tostring(props.badge) } },
		} }
	end,
}
