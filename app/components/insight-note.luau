-- A short note an assistant leaves under a chart or list: a sparkle and a
-- small accent label ("AI insight"), then the finding, optionally with an
-- underlined action in the middle of the sentence, in a soft blue box.

type Props = {
	label: string?,
	icon: string?,
	text: string,
	link: string?,
	after: string?,
	msg: any,
	to: string?,
}

return component {
	title = "Insight Note",
	category = "Feedback",
	icon = "sparkles",
	description = "A soft tinted box with a sparkle label and a finding, with an optional underlined action inside the sentence.",
	when = "An assistant's finding beside the data it explains (under a chart, over a table, in a checklist card). For a warning that must be dealt with use `notice`; for a conversation use `chat-message`; for a transient message use `toast`.",
	props = {
		["label"] = "the small heading (default \"AI insight\")",
		["icon"] = "icon before the label (default sparkles)",
		["text"] = "the finding",
		["link"] = "underlined action text after `text`; it sends `msg`",
		["after"] = "text after the link, ending the sentence",
		["msg"] = "message the link sends",
		["to"] = "mod to send `msg` to, if not the drawing one",
	},
	styles = {
		[".insight-note"] = { background = "accent/8", border = true, border_color = "accent/22", radius = 10, padding = { 10, 14 }, gap = 4 },
		[".insight-note-label"] = { color = "accent", size = "small", weight = "medium" },
		[".insight-note-text"] = { size = "small", color = "foreground/80" },
		[".insight-note-link"] = { size = "small", color = "foreground", weight = "medium" },
		[".insight-note-link:hover"] = { color = "accent" },
		[".insight-note-rule"] = { background = "foreground/70" },
	},
	example = {
		text = "Exceptions are running 29% above forecast since the last ledger sync. Recommend:",
		link = "read the dates from invoice scans",
		after = "and review the 9 unclear disputes.",
		msg = "read-scans",
	},

	build = function(props: Props)
		local body: any
		if props.link then
			body = ui.column {
				gap = "none",
				ui.text { props.text, class = "insight-note-text" },
				ui.row {
					wrap = true,
					gap = "none",
					ui.column {
						gap = "none",
						hug = true,
						msg = props.msg,
						to = props.to,
						ui.text { props.link, class = "insight-note-link", wrap = false },
						ui.row { class = "insight-note-rule", height = 1 },
					},
					props.after and ui.text { " " .. props.after, class = "insight-note-text" },
				},
			}
		else
			body = ui.text { props.text, class = "insight-note-text" }
		end
		return ui.column {
			class = "insight-note",
			ui.row {
				gap = "small",
				align = "center",
				ui.icon { name = (props.icon or "sparkles") :: any, size = "small", class = "insight-note-label" },
				ui.text { props.label or "AI insight", class = "insight-note-label" },
			},
			body,
		}
	end,
}
