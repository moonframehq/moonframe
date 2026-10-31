-- A vertical rail of dots, one line per entry: its time, a mono title (a
-- checkpoint, a commit), a detail and its diff counts, with day headings
-- where the day changes. Clicking an entry sends `{ select = id }`.

type Entry = {
	id: any,
	day: string?,
	time: string?,
	title: string,
	detail: string?,
	added: number?,
	removed: number?,
	tone: string?,
	icon: string?,
	msg: any,
}

type Props = {
	entries: { Entry },
	selected: any,
	event: string?,
	to: string?,
	mono: boolean?,
	empty: string?,
}

local diff_counts = use "diff-counts"

local EXAMPLE_ENTRIES: { Entry } = {
	{ id = "c5", day = "Today", time = "14:32", title = "checkpoint-5", detail = "Balance per currency", added = 38, removed = 16, tone = "accent" },
	{ id = "c4", day = "Today", time = "13:05", title = "checkpoint-4", detail = "Posting struct replaces tuples", added = 12, removed = 4, tone = "accent" },
	{ id = "c3", day = "Today", time = "11:47", title = "checkpoint-3", detail = "Tests for rounding", added = 64 },
	{ id = "c2", day = "Yesterday", time = "18:20", title = "checkpoint-2", detail = "Agent stopped: needs input", tone = "warning" },
	{ id = "c1", day = "Yesterday", time = "17:02", title = "checkpoint-1", detail = "Initial worktree", added = 3, removed = 1 },
}

return component {
	title = "Timeline",
	category = "Data",
	icon = "git-commit-horizontal",
	description = "A vertical rail of dots with one entry per line (time, mono title, detail, diff counts) and day headings; entries are selectable.",
	when = "Events in order: checkpoints, commits, deploys, an agent's steps. For a flat selectable list use `list`; for comments use `comment-card`; for a table of runs use `table`.",
	props = {
		["entries"] = "list of { id, day (a heading shown when it changes), time, title, detail, added, removed, tone (accent|success|warning|danger|muted dot), icon (instead of the dot), msg }",
		["selected"] = "id of the selected entry",
		["event"] = "message key instead of \"select\": sends { [event] = id }",
		["to"] = "the mod to send it to",
		["mono"] = "false sets titles in the sans font",
		["empty"] = "text when there are no entries",
	},
	styles = {
		[".timeline"] = { gap = 0 },
		[".timeline-day"] = { padding = { top = 10, bottom = 4, x = 6 } },
		[".timeline-day-text"] = { size = "small", weight = "bold", color = "muted-foreground" },
		[".timeline-entry"] = { gap = 10, padding = { 0, 6 }, radius = 6 },
		[".timeline-entry:hover"] = { background = "list-hover" },
		[".timeline-entry:selected"] = { background = "list-active" },
		[".timeline-rail"] = { gap = 0 },
		[".timeline-line"] = { background = "border" },
		[".timeline-dot"] = { radius = 999, background = "muted-foreground/60" },
		[".timeline-dot-accent"] = { background = "accent" },
		[".timeline-dot-success"] = { background = "success" },
		[".timeline-dot-warning"] = { background = "warning" },
		[".timeline-dot-danger"] = { background = "danger" },
		[".timeline-icon"] = { size = "small", color = "muted-foreground" },
		[".timeline-body"] = { gap = 2, padding = { 6, 0 } },
		[".timeline-time"] = { size = "small", color = "muted-foreground", font = "mono" },
		[".timeline-title"] = { size = "small", weight = "medium" },
		[".timeline-title-mono"] = { font = "mono" },
		[".timeline-detail"] = { size = "small", color = "muted-foreground" },
		[".timeline-empty"] = { size = "small", color = "muted-foreground" },
	},
	usage = [[local timeline = use "timeline"

	timeline {
		selected = state.checkpoint,
		entries = {
			{ id = "c5", day = "Today", time = "14:32", title = "checkpoint-5",
				detail = "Balance per currency", added = 38, removed = 16, tone = "accent" },
			{ id = "c2", day = "Yesterday", time = "18:20", title = "checkpoint-2",
				detail = "Agent stopped: needs input", tone = "warning" },
		},
	} -- sends { select = id }]],
	example = {
		selected = "c4",
		entries = EXAMPLE_ENTRIES,
	},

	build = function(props: Props)
		local children = {}
		local day = nil
		for index, entry in props.entries do
			if entry.day and entry.day ~= day then
				day = entry.day
				table.insert(children, ui.row {
					class = "timeline-day",
					shrink = false,
					ui.text { entry.day :: string, class = "timeline-day-text" },
				})
			end
			local nextEntry = props.entries[index + 1]
			-- The rail runs on to the next dot unless the day breaks or it ends.
			local continues = nextEntry ~= nil and (nextEntry.day == nil or nextEntry.day == day)
			local first = index == 1 or (entry.day ~= nil and props.entries[index - 1].day ~= entry.day)
			local mark = if entry.icon
				then ui.icon { name = entry.icon :: any, class = "timeline-icon" }
				else ui.row {
					class = if entry.tone then { "timeline-dot", `timeline-dot-{entry.tone}` } else "timeline-dot",
					width = 9,
					height = 9,
				}
			local rail = ui.column {
				class = "timeline-rail",
				width = 14,
				align = "center",
				ui.row { class = if first then nil else "timeline-line", width = 2, height = 10 },
				mark,
				ui.row { class = if continues then "timeline-line" else nil, width = 2, grow = true },
			}
			local titleClasses = { "timeline-title" }
			if props.mono ~= false then
				table.insert(titleClasses, "timeline-title-mono")
			end
			local id = if entry.id == nil then index else entry.id
			table.insert(children, ui.row {
				class = "timeline-entry",
				shrink = false,
				align = "stretch",
				selected = id == props.selected,
				msg = entry.msg or { [props.event or "select"] = id },
				to = props.to,
				rail,
				ui.column {
					class = "timeline-body",
					grow = true,
					ui.row {
						align = "center",
						gap = "small",
						ui.text { entry.title, class = titleClasses, truncate = true },
						ui.spacer(),
						(entry.added or entry.removed) and diff_counts { added = entry.added, removed = entry.removed },
						entry.time and ui.text { entry.time, class = "timeline-time" },
					},
					entry.detail and ui.text { entry.detail, class = "timeline-detail", truncate = true },
				},
			})
		end
		if #children == 0 then
			children[1] = ui.text { props.empty or "Nothing yet", class = "timeline-empty" }
		end
		return ui.column { class = "timeline", table.unpack(children) }
	end,
}
