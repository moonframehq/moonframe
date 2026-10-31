-- Monospace text in a bordered box with a Copy button: a command to run,
-- a pairing code, a path. Clicking the button puts the text on the
-- clipboard; the mod can flip `copied` to show it worked.

type Props = {
	text: string,
	label: string?,
	copied: boolean?,
	msg: any,
	button: string?,
	to: string?,
}

return component {
	title = "Copy Field",
	category = "Controls",
	icon = "copy",
	description = "Monospace text in a bordered box with a Copy button that puts it on the clipboard, and a \"Copied\" state.",
	when = "Text the user will paste elsewhere: an install command, a token, a URL, a path. For read-only label/value pairs use `key-values`; for an editable field wait for `ui.input`.",
	props = {
		["text"] = "the text shown and copied",
		["label"] = "small muted heading above the box",
		["copied"] = "show \"Copied\" with a check in the success colour (the mod sets it after `msg`, then clears it)",
		["msg"] = "message sent on the click as well, e.g. to set `copied`",
		["button"] = "button text (default \"Copy\")",
		["to"] = "mod to send `msg` to, if not the drawing one",
	},
	styles = {
		[".copy-field"] = { gap = 4 },
		[".copy-field-label"] = { color = "muted-foreground", size = "small", weight = "medium" },
		[".copy-field-box"] = {
			gap = 8,
			padding = { 4, 4, 4, 10 },
			radius = 8,
			border = "all",
			border_color = "border",
			background = "muted/50",
		},
		[".copy-field-text"] = { font = "mono", size = "small" },
		[".copy-field-button"] = {
			gap = 5,
			padding = { 3, 8 },
			radius = 6,
			border = "all",
			border_color = "border",
			background = "background",
			color = "muted-foreground",
			size = "small",
		},
		[".copy-field-button:hover"] = { color = "foreground", background = "list-hover" },
		[".copy-field-button:selected"] = { color = "success", border_color = "success/40", background = "success/10" },
	},
	example = { label = "Install", text = "curl -fsSL https://moonframe.dev/install.sh | sh", copied = false },
	usage = [[local copy_field = use "copy-field"

copy_field { label = "Install", text = "curl -fsSL https://moonframe.dev/install.sh | sh",
	copied = state.copied, msg = "copied" }]],

	build = function(props: Props)
		local copied = props.copied == true
		return ui.column {
			class = "copy-field",
			props.label and ui.text { props.label, class = "copy-field-label" },
			ui.row {
				class = "copy-field-box",
				align = "center",
				ui.row { grow = true, ui.text { props.text, class = "copy-field-text", truncate = true } },
				ui.row {
					class = "copy-field-button",
					align = "center",
					selected = copied,
					copy = props.text,
					msg = props.msg,
					to = props.to,
					tooltip = if copied then nil else "Copy to clipboard",
					ui.icon { name = if copied then "check" else "copy" },
					ui.text { if copied then "Copied" else (props.button or "Copy") },
				},
			},
		}
	end,
}
