-- Where a message to an agent is written: a chip naming who it goes to, an
-- attach button, the field, and a round send button. The host keeps the
-- text; the mod gets it on each edit and on send.

type Props = {
	id: string?,
	value: string?,
	placeholder: string?,
	agent: string?,
	icon: string?,
	change: any,
	submit: any,
	attach: any,
	send: any,
	to: string?,
	focus: boolean?,
}

return component {
	title = "Composer",
	category = "Forms",
	icon = "send",
	description = "A message box for an agent: a chip naming it, an attach button, the field and a round send button.",
	when = "The bottom of a chat with an agent or a person. For a search box use `search-field`; for a longer form field use `ui.input` with `multiline`.",
	props = {
		["id"] = "keeps the text across renders (default \"composer\")",
		["value"] = "the text; \"\" after sending clears it",
		["placeholder"] = "grey text while empty",
		["agent"] = "who it goes to, in a chip above the field",
		["icon"] = "the chip's icon (default sparkles)",
		["change"] = "sent on each edit, with `value` set to the text",
		["submit"] = "sent on Enter and by the send button, with `value`",
		["attach"] = "message the attach button sends; no button without it",
		["send"] = "message the round button sends; a click cannot read the field, so the mod keeps the text from `change`",
		["to"] = "mod to send them to, if not the drawing one",
		["focus"] = "take focus when it appears",
	},
	styles = {
		[".composer"] = { background = "background", border = true, border_color = "border", radius = 14, padding = { 10, 12 }, gap = 8, shadow = "0 2px 8px #0000000d" },
		[".composer-chip"] = { background = "secondary", radius = 6, padding = { 2, 8 }, gap = 6 },
		[".composer-chip-icon"] = { color = "accent" },
		[".composer-field"] = { background = "#00000000", border = "none", shadow = "none", padding = { 4, 0 } },
		[".composer-send"] = { background = "foreground", color = "background", radius = 14 },
		[".composer-attach"] = { color = "muted-foreground" },
	},
	example = { agent = "Sales agent", placeholder = "Ask the sales agent anything…", attach = "attach", submit = "send", send = "send" },

	build = function(props: Props)
		local submit = props.submit
		return ui.column {
			class = "composer",
			props.agent and ui.row {
				class = "composer-chip",
				align = "center",
				hug = true,
				ui.icon { name = (props.icon or "sparkles") :: any, size = "small", class = "composer-chip-icon" },
				ui.text { props.agent, size = "small", weight = "medium" },
			},
			ui.row {
				align = "center",
				gap = "small",
				props.attach ~= nil and ui.button { icon = "link", variant = "ghost", small = true, msg = props.attach, to = props.to },
				ui.input {
					id = props.id or "composer",
					value = props.value,
					placeholder = props.placeholder or "Write a message…",
					change = props.change,
					submit = submit,
					to = props.to,
					focus = props.focus,
					grow = true,
					class = "composer-field",
				},
				ui.row {
					class = "composer-send",
					width = 28,
					height = 28,
					align = "center",
					justify = "center",
					shrink = false,
					tooltip = "Send (Enter)",
					msg = props.send,
					to = props.to,
					ui.icon { name = "arrow-up", size = "small" },
				},
			},
		}
	end,
}
