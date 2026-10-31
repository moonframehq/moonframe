-- Back and forward chevrons joined into one capsule, as in a title bar or
-- a browser. Either side greys out when there is nowhere to go.

type Props = {
	can_back: boolean?,
	can_forward: boolean?,
	back: any,
	forward: any,
	small: boolean?,
	to: string?,
}

return component {
	title = "Nav Buttons",
	category = "Navigation",
	icon = "chevron-left",
	description = "Back and forward chevrons as one capsule; each side disables when it cannot go. Sends `back` and `forward`.",
	when = "History navigation in a title bar, a browser tab or a pane that keeps a back stack. For onboarding steps use `pager`; for a single icon action use `icon-button`.",
	props = {
		["can_back"] = "false greys out the back chevron (default true)",
		["can_forward"] = "false greys out the forward chevron (default true)",
		["back"] = "message the back chevron sends (default \"back\")",
		["forward"] = "message the forward chevron sends (default \"forward\")",
		["small"] = "a smaller capsule",
		["to"] = "mod to send messages to, if not the drawing one",
	},
	styles = {
		[".nav-buttons"] = {
			gap = 0,
			radius = 8,
			border = "all",
			border_color = "border",
			background = "background",
		},
		[".nav-buttons-button"] = { radius = 7, color = "muted-foreground" },
		[".nav-buttons-button:hover"] = { background = "list-hover", color = "foreground" },
		[".nav-buttons-disabled"] = { opacity = 0.35 },
		[".nav-buttons-separator"] = { background = "border" },
		[".nav-buttons-small"] = { size = "small" },
	},
	example = { can_back = true, can_forward = false, back = "back", forward = "forward" },

	build = function(props: Props)
		local side = if props.small then 22 else 26
		local function button(icon: string, enabled: boolean, msg: any, tooltip: string)
			local class = { "nav-buttons-button" }
			if not enabled then
				table.insert(class, "nav-buttons-disabled")
			end
			if props.small then
				table.insert(class, "nav-buttons-small")
			end
			return ui.row {
				class = class,
				width = side + 4,
				height = side,
				align = "center",
				justify = "center",
				tooltip = tooltip,
				msg = if enabled then msg else nil,
				to = props.to,
				ui.icon { name = icon :: any },
			}
		end
		-- Wrapped so a stretching parent does not widen the capsule.
		return ui.row { gap = "none", ui.row {
			class = "nav-buttons",
			align = "center",
			button("chevron-left", props.can_back ~= false, props.back or "back", "Back"),
			ui.row { class = "nav-buttons-separator", width = 1, height = side - 10 },
			button("chevron-right", props.can_forward ~= false, props.forward or "forward", "Forward"),
		} }
	end,
}
