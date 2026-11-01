-- A markdown document, drawn by the host's `ui.markdown`: headings,
-- emphasis, lists, task lists, tables, quotes and highlighted code. A link
-- never opens by itself; with `link` set, a click sends it with the address
-- as `url`, for the mod to open as it sees fit. Pictures are not loaded.

type Props = {
	text: string?,
	link: any,
	to: string?,
	grow: boolean?,
	scroll: boolean?,
	max_width: number?,
}

return component {
	title = "Markdown",
	category = "Content",
	icon = "book-open",
	description = "A markdown document: headings, emphasis, lists, task lists, tables, quotes and highlighted code; links send a message instead of opening.",
	when = "Show markdown read-only: a README, notes, an agent's answer, release notes written in markdown. For editing a file use `markdown-editor`; for code alone use `code-view`; for one line of text use `ui.text`.",
	props = {
		["text"] = "the markdown, up to 1 MiB",
		["link"] = "message a link click sends, with `url` added (a table gets the field; anything else arrives as { msg = link, url })",
		["to"] = "mod to send `link` to, if not the drawing one",
		["grow"] = "fill the parent's free space",
		["scroll"] = "scroll its own content (with `grow` or a fixed height around it)",
		["max_width"] = "pixels: a readable measure for long prose",
	},
	styles = {
		[".markdown"] = { padding = { 12, 16 } },
	},
	example = {
		text = "# Release notes\n\nMarkdown is **drawn by the host**, with `inline code`, [links](https://luau.org) and lists:\n\n- [x] Headings and emphasis\n- [x] Tables and quotes\n- [ ] Pictures (not loaded)\n\n> Links send a message; the mod decides.\n\n```lua\nlocal markdown = use \"markdown\"\n```\n",
	},
	usage = [[local markdown = use "markdown"

markdown { text = state.notes, grow = true, scroll = true, link = "open-link" }
-- update: msg.msg == "open-link" carries the address in msg.url]],

	build = function(props: Props)
		return ui.markdown {
			class = "markdown",
			text = props.text or "",
			link = props.link,
			to = props.to,
			grow = props.grow,
			scroll = props.scroll,
			max_width = props.max_width,
		}
	end,
}
