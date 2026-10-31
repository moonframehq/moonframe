-- Buttons joined into one bordered group, with thin lines between them:
-- "Uncommitted | Branch", "Unified | Split". Any of them may be marked
-- selected, so it works for actions and for a choice.

type Button = {
	id: string?,
	label: string?,
	icon: string?,
	tooltip: string?,
	selected: boolean?,
	disabled: boolean?,
	msg: any,
}

type Props = {
	buttons: { Button },
	selected: string?,
	event: string?,
	small: boolean?,
	to: string?,
}

-- The preview's buttons, typed loosely: they mix kinds of entries.
local EXAMPLE: { any } = {
	{ id = "unified", label = "Unified", icon = "rows-2" },
	{ id = "split", label = "Split", icon = "columns-2" },
	{ id = "refresh", icon = "refresh-cw", tooltip = "Refresh", msg = "refresh" },
}

return component {
	title = "Button Group",
	category = "Controls",
	icon = "columns-2",
	description = "Buttons joined into one bordered group with dividers, text or icons, optionally one selected; sends { select = id }.",
	when = "Related actions or view modes side by side in a header: Unified | Split, Uncommitted | Branch, zoom − | +. For a settings choice in a track use `segmented`; for separate quiet buttons use `toolbar`; for back/forward use `nav-buttons`.",
	props = {
		["buttons"] = "list of { id, label?, icon?, tooltip?, selected?, disabled?, msg? (sent instead of the default) }",
		["selected"] = "id of the selected button (or mark buttons `selected`)",
		["event"] = "key of the message a click sends, default \"select\": { select = id }",
		["small"] = "smaller buttons",
		["to"] = "mod to send messages to, if not the drawing one",
	},
	styles = {
		[".button-group"] = {
			gap = 0,
			radius = 8,
			border = "all",
			border_color = "border",
			background = "background",
			padding = 1,
		},
		[".button-group-button"] = { padding = { 4, 10 }, gap = 6, radius = 6, color = "muted-foreground" },
		[".button-group-button:hover"] = { background = "list-hover", color = "foreground" },
		[".button-group-button:selected"] = { background = "secondary", color = "foreground" },
		[".button-group-small"] = { padding = { 2, 7 }, size = "small", radius = 5 },
		[".button-group-disabled"] = { opacity = 0.4 },
		[".button-group-separator"] = { background = "border", margin = { 0, 1 } },
	},
	example = {
		selected = "unified",
		buttons = EXAMPLE,
	},

	build = function(props: Props)
		local event = props.event or "select"
		local cells = {}
		for index, button in props.buttons do
			local id = button.id or button.label
			if index > 1 then
				cells[#cells + 1] = ui.row { class = "button-group-separator", width = 1, height = if props.small then 14 else 18 }
			end
			local class = { "button-group-button" }
			if props.small then
				table.insert(class, "button-group-small")
			end
			if button.disabled then
				table.insert(class, "button-group-disabled")
			end
			cells[#cells + 1] = ui.row {
				class = class,
				align = "center",
				justify = "center",
				selected = button.selected == true or (id ~= nil and id == props.selected),
				tooltip = button.tooltip,
				msg = if button.disabled then nil else (button.msg or { [event] = id }),
				to = props.to,
				button.icon and ui.icon { name = button.icon :: any },
				button.label and ui.text { button.label },
			}
		end
		-- Wrapped so a stretching parent does not widen the group.
		return ui.row { gap = "none", ui.row { class = "button-group", align = "center", table.unpack(cells) } }
	end,
}
