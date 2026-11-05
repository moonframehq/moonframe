-- A document's headings as a list to jump through, indented by level, with
-- the one being read marked. It takes the `headings` a `ui.markdown` sends to
-- its `outline` message, and the index its `section` message sends.

type Heading = { level: number, text: string, at: number?, slug: string? }

type Props = {
	headings: { Heading }?,
	current: number?,
	event: string?,
	indent: number?,
	depth: number?,
	empty: string?,
	to: string?,
}

return component {
	title = "Outline",
	category = "Navigation",
	icon = "list-tree",
	description = "A document's headings, indented by level, with the one being read marked; a click sends { heading = index }.",
	when = "Beside a long document: a README, a note, a report read in a `markdown`. For a file tree use `tree`; for an app's navigation use `side-nav`; for steps use `numbered-list`.",
	props = {
		["headings"] = "list of { level, text, at? }, as `ui.markdown`'s `outline` message sends them",
		["current"] = "index of the heading being read, as its `section` message sends",
		["event"] = "key of the message a click sends, default \"heading\": { heading = index }",
		["indent"] = "pixels per level (default 12)",
		["depth"] = "deepest level shown (default all)",
		["empty"] = "text when there are no headings (none when unset)",
		["to"] = "mod to send it to, if not the drawing one",
	},
	styles = {
		[".outline"] = { gap = 1 },
		[".outline-row"] = { padding = { 3, 8 }, radius = 6, color = "muted-foreground" },
		[".outline-row:hover"] = { background = "list-hover", color = "foreground" },
		[".outline-row:selected"] = { background = "accent/12", color = "foreground" },
		[".outline-top"] = { weight = "medium" },
		[".outline-empty"] = { size = "small", color = "muted-foreground", padding = { 3, 8 } },
	},
	example = {
		current = 3,
		headings = {
			{ level = 1, text = "Getting started" },
			{ level = 2, text = "Install" },
			{ level = 2, text = "First run" },
			{ level = 3, text = "Your first note" },
			{ level = 1, text = "Reference" },
		},
	},
	usage = [[local outline = use "outline"
local markdown = use "markdown"

outline { headings = state.headings, current = state.section }
markdown { text = state.text, grow = true, scroll = true, reveal = state.reveal,
	outline = "headings", section = "reading" }
-- update: msg.msg == "headings" keeps state.headings = msg.headings;
-- msg.msg == "reading" keeps state.section = msg.heading; msg.heading on its
-- own is a click: state.reveal = { at = state.headings[msg.heading].at,
-- seq = state.seq } after adding one to state.seq.]],

	build = function(props: Props)
		local headings: { Heading } = props.headings or {}
		local event = props.event or "heading"
		local indent = props.indent or 12
		local top = math.huge
		for _, heading in headings do
			top = math.min(top, heading.level)
		end
		local rows = {}
		for index, heading in headings do
			if props.depth and heading.level > props.depth then
				continue
			end
			local level = heading.level - top
			table.insert(rows, ui.row {
				class = if level == 0 then "outline-row outline-top" else "outline-row",
				selected = props.current == index,
				style = { margin = { left = level * indent } },
				msg = { [event] = index },
				to = props.to,
				ui.text { heading.text, truncate = true },
			})
		end
		if #rows == 0 and props.empty then
			return ui.text { props.empty, class = "outline-empty" }
		end
		return ui.column { class = "outline", table.unpack(rows) }
	end,
}
