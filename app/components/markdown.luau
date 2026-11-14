-- A markdown document, drawn by the host's `ui.markdown`: headings,
-- emphasis, lists, task lists, tables, quotes, GitHub alerts, footnotes and
-- highlighted code with a copy button, and math where the app has it. A link
-- never opens by itself: with `link` set, a click sends it with the address
-- as `url`, for the mod to open as it sees fit, and a `#heading` link scrolls
-- there. Pictures show only with `images`, and only files a `ui.image` could.

type Props = {
	text: string?,
	link: any,
	to: string?,
	grow: boolean?,
	scroll: boolean?,
	max_width: number?,
	lines: number?,
	images: boolean?,
	image: any,
	outline: any,
	section: any,
	reveal: { at: number, seq: number? }?,
	find: string?,
	find_index: number?,
	found: any,
	stream: boolean?,
	scrolled: any,
	scroll_at: { fraction: number, seq: number? }?,
}

return component {
	title = "Markdown",
	category = "Content",
	icon = "book-open",
	description = "A markdown document: headings, emphasis, lists, task lists, tables, quotes, alerts, footnotes and highlighted code; links send a message instead of opening.",
	when = "Show markdown read-only: a README, notes, an agent's answer, release notes written in markdown. For editing a file use `markdown-editor`; for a chat answer streaming in, or one with source chips, use `chat-markdown`; for code alone use `code-view`; for one line of text use `ui.text`.",
	props = {
		["text"] = "the markdown, up to 1 MiB",
		["link"] = "message a link click sends, with `url` added (a table gets the field; anything else arrives as { msg = link, url })",
		["to"] = "mod to send its messages to, if not the drawing one",
		["grow"] = "fill the parent's free space",
		["scroll"] = "scroll its own content (with `grow` or a fixed height around it)",
		["max_width"] = "pixels: a readable measure for long prose",
		["lines"] = "show at most this many lines, for a preview (not with `scroll`)",
		["images"] = "show pictures, from files a `ui.image` could show; never from the web. Without it a picture on its own line shows its alt text",
		["image"] = "message a click on a picture on its own line sends, with `src` and `alt` added, to open it in a `lightbox`",
		["outline"] = "message sent as the headings change, with `headings`: { level, text, at, slug }; feed it to `outline`",
		["section"] = "message sent as a scrolling document passes a heading, with `heading`, its index",
		["reveal"] = "{ at = <a heading's `at`>, seq = <a number> }: scrolls there when it changes (with `scroll`); a new `seq` goes there again",
		["find"] = "text to mark wherever it shows",
		["find_index"] = "which match is current, from 1: marked strongly and scrolled to",
		["found"] = "message sent as the number of matches changes, with `count`",
		["stream"] = "fade in what is added to `text`, as an answer streams in",
		["scrolled"] = "message sent as the reader scrolls (with `scroll`), with `fraction`: 0 at the top, 1 at the end",
		["scroll_at"] = "{ fraction = <0 to 1>, seq = <a number> }: scrolls there when it changes (with `scroll`), without sending `scrolled`; a new `seq` goes there again",
	},
	styles = {
		[".markdown"] = { padding = { 12, 16 } },
	},
	example = {
		text = "# Release notes\n\nMarkdown is **drawn by the host**, with `inline code`, [links](https://luau.org), footnotes[^1] and lists:\n\n- [x] Headings and emphasis\n- [x] Tables, quotes and alerts\n\n> [!NOTE]\n> Links send a message; the mod decides.\n\n```lua\nlocal markdown = use \"markdown\"\n```\n\n[^1]: A click on the mark scrolls here.\n",
	},
	usage = [[local markdown = use "markdown"

markdown { text = state.notes, grow = true, scroll = true, link = "open-link" }
-- update: msg.msg == "open-link" carries the address in msg.url

-- A preview of a note, cut to four lines:
markdown { text = note.body, lines = 4 }

-- A document beside its `outline`, with a `find-bar` over it:
markdown {
	text = state.text, grow = true, scroll = true,
	outline = "headings", section = "reading", reveal = state.reveal,
	find = state.query, find_index = state.match, found = "found",
}]],

	build = function(props: Props)
		return ui.markdown {
			class = "markdown",
			text = props.text or "",
			link = props.link,
			to = props.to,
			grow = props.grow,
			scroll = props.scroll,
			max_width = props.max_width,
			lines = props.lines,
			images = props.images,
			image = props.image,
			outline = props.outline,
			section = props.section,
			reveal = props.reveal,
			find = props.find,
			find_index = props.find_index,
			found = props.found,
			stream = props.stream,
			scrolled = props.scrolled,
			scroll_at = props.scroll_at,
		}
	end,
}
