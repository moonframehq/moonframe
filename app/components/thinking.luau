-- How long an agent thought, as a quiet fold: a chevron and "Thought for 6
-- seconds"; open, its reasoning in muted text under it.

type Props = {
	seconds: number?,
	label: string?,
	open: boolean?,
	msg: any,
	to: string?,
	text: string?,
}

return component {
	title = "Thinking",
	category = "Feedback",
	icon = "sparkles",
	description = "A quiet fold saying how long an agent thought, showing its reasoning when open.",
	when = "Inside an agent's `chat-message`, before its answer. For a spinner while it is still thinking use `loading`; for a general fold use `collapsible`.",
	props = {
		["seconds"] = "how long it thought",
		["label"] = "the text instead of \"Thought for N seconds\"",
		["open"] = "show the reasoning",
		["msg"] = "message a click sends (the mod flips `open`)",
		["to"] = "mod to send it to, if not the drawing one",
		["text"] = "the reasoning shown when open",
	},
	styles = {
		[".thinking"] = { gap = 6 },
		[".thinking-head"] = { gap = 4, color = "muted-foreground" },
		[".thinking-head:hover"] = { color = "foreground" },
		[".thinking-text"] = { size = "small", color = "muted-foreground", padding = { 0, 0, 0, 18 } },
	},
	example = { seconds = 6, open = true, text = "Three late deals have had no contact in ten days." },

	build = function(props: Props)
		local seconds = props.seconds or 0
		local label = props.label or `Thought for {seconds} {if seconds == 1 then "second" else "seconds"}`
		return ui.column {
			class = "thinking",
			ui.row {
				class = "thinking-head",
				align = "center",
				msg = props.msg,
				to = props.to,
				ui.icon { name = if props.open then "chevron-down" else "chevron-right", size = "small" },
				ui.text { label, size = "small" },
			},
			props.open and props.text and ui.text { props.text, class = "thinking-text", wrap = true },
		}
	end,
}
