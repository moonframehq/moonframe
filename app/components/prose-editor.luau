-- A writing surface: a borderless text area in a centred column of a
-- readable measure, with markdown styled as it is typed. `dim` keeps the
-- sentence or paragraph being written in full ink and fades the rest
-- (focus mode); `typewriter` keeps the line being written in the middle.
-- Edits send `change` with the text as `value`; the mod keeps it.

type Props = {
	id: string?,
	value: string?,
	placeholder: string?,
	change: any?,
	submit: any?,
	keys: { [string]: any }?,
	to: string?,
	dim: ("sentence" | "paragraph")?,
	typewriter: boolean?,
	markdown: boolean?,
	mono: boolean?,
	size: ("normal" | "large" | "title")?,
	measure: number?,
	focus: boolean?,
}

return component {
	title = "Prose Editor",
	category = "Content",
	icon = "pencil",
	description = "A distraction-free writing surface: a centred column of text with markdown styled as typed, focus mode that fades all but the current sentence or paragraph, and typewriter scrolling.",
	when = "Writing long text: a draft, an article, notes, a journal, where the words are the whole screen. For a markdown file with a preview beside it use `markdown-editor`; for a short field in a form use `ui.input` (with `multiline` for a text area); for a chat box use `composer`.",
	props = {
		["id"] = "keeps the text across renders; give each document its own (default \"prose\")",
		["value"] = "the text; applied when it changes, so opening another document replaces it",
		["placeholder"] = "grey text while empty (default \"Start writing…\")",
		["change"] = "sent on each edit, with `value` set to the text",
		["submit"] = "sent on ⌘↩, with `value`",
		["keys"] = "keys the field sends instead of editing while focused: { ['secondary-d'] = \"focus\" }",
		["to"] = "mod to send its messages to, if not the drawing one",
		["dim"] = "focus mode: \"sentence\" or \"paragraph\" stays in full ink, the rest fades; nil shows all",
		["typewriter"] = "keep the caret's line in the middle of the field",
		["markdown"] = "false writes plain text: no styling of markdown (default true)",
		["mono"] = "set the text in the monospace font",
		["size"] = "the text's size: normal, large (default) or title",
		["measure"] = "the column's widest width in pixels, a readable line (default 680)",
		["focus"] = "take keyboard focus when it appears, or when this turns true",
	},
	styles = {
		[".prose-editor"] = { padding = { 28, 32, 0, 32 } },
		[".prose-editor-field"] = {
			background = "background",
			border = "none",
			radius = 0,
			padding = 0,
			color = "foreground",
		},
		[".prose-editor-field:hover"] = { border = "none" },
		[".prose-editor-field:focus"] = { border = "none" },
		[".prose-editor-normal"] = { size = "normal" },
		[".prose-editor-large"] = { size = "large" },
		[".prose-editor-title"] = { size = "title" },
	},
	example = {
		value = "# A quiet page\n\nThe sentence being written stays in full ink. The rest of the page steps back, so the next word is all there is.\n\nMarkdown takes its shape as it is typed: *emphasis*, **strong**, `code`.",
		dim = "sentence",
		measure = 520,
	},

	build = function(props: Props)
		local size = props.size or "large"
		return ui.row {
			class = "prose-editor",
			grow = true,
			justify = "center",
			ui.column {
				grow = true,
				max_width = props.measure or 680,
				ui.input {
					id = props.id or "prose",
					class = `prose-editor-field prose-editor-{size}`,
					value = props.value,
					placeholder = props.placeholder or "Start writing…",
					change = props.change,
					submit = props.submit,
					keys = props.keys,
					to = props.to,
					multiline = true,
					grow = true,
					prose = props.markdown ~= false,
					dim = props.dim,
					typewriter = props.typewriter,
					mono = props.mono,
					focus = props.focus,
				},
			},
		}
	end,
}
