local pill = use "pill"

type Props = {
	thread: index<index<index<App, "t3"> & {}, "threads">, number>,
	expanded: boolean?,
	msg: any,
	to: string?,
}

return component {
	title = "T3 Thread",
	category = "Lists",
	icon = "messages-square",
	description = "A T3 session with its project, branch and linked work.",
	when = "Use for one thread from app.t3 in a project sidebar or session overview. Set expanded to show its worktree, provider and update time.",
	props = {
		thread = "A thread from app.t3.threads",
		expanded = "Show session details",
		msg = "Optional message sent when the row is clicked",
		to = "Mod receiving the click",
	},
	styles = {
		[".t3-thread"] = { padding = { 9, 12 }, gap = 5 },
		[".t3-thread-meta"] = { color = "muted-foreground", size = "small" },
	},
	example = { thread = {
		id = "review", title = "Review pagination", projectId = "mf", project = "Moonframe", branch = "fix/paging",
		group = "waiting", status = "waiting", updatedAt = "2026-11-20T12:00:00Z",
		pullRequests = { { label = "#42", title = "Moonframe", url = "https://example.com/pr/42" } }, issues = {},
	} },
	build = function(props: Props)
		local thread = props.thread
		local links: { Element } = {}
		for _, link in thread.pullRequests do
			table.insert(links, pill { label = link.label, tooltip = link.title, msg = link.url, to = "app:open", icon = "git-pull-request" })
		end
		for _, link in thread.issues do
			table.insert(links, pill { label = link.label, tooltip = link.title, msg = link.url, to = "app:open", icon = "circle-dot" })
		end
		return ui.column {
			class = "t3-thread", msg = props.msg, to = props.to,
			ui.row {
				align = "center", ui.text { thread.title, truncate = true }, ui.spacer(),
				ui.text { thread.status, class = "t3-thread-meta" },
			},
			ui.row { align = "center",
				ui.text { thread.project .. (if thread.branch then " · " .. thread.branch else ""),
					class = "t3-thread-meta", truncate = true },
				table.unpack(links),
			},
			props.expanded and ui.text {
				table.concat({ thread.provider or "T3 Code", thread.updatedAt, thread.worktree or thread.id }, " · "),
				class = "t3-thread-meta",
			},
		}
	end,
}
