local group_header = use "group-header"
local t3_thread = use "t3-thread"

type Props = {
	threads: index<index<App, "t3"> & {}, "threads">,
	folded: { [string]: boolean }?,
	selected: string?,
	to: string?,
}

local groups = {
	{ id = "waiting", title = "Waiting for you", status = "blocked" },
	{ id = "running", title = "Running", status = "working" },
	{ id = "active", title = "Other active sessions", status = "idle" },
	{ id = "settled", title = "Settled", status = "done" },
}

return component {
	title = "T3 Sessions",
	category = "Lists",
	icon = "list",
	description = "T3 sessions grouped by what needs attention, with counts and expandable rows.",
	when = "Use for an overview of app.t3.threads, or a subset filtered by project, issue or worktree. Handle fold and thread messages in the drawing mod.",
	props = {
		threads = "Threads in display order",
		folded = "Map from group id to collapsed state",
		selected = "Id of the expanded thread",
		to = "Mod receiving { fold = group } and { thread = id }",
	},
	example = { threads = {
		{ id = "docs", projectId = "docs", project = "Docs", title = "Check examples", status = "running",
			group = "running", updatedAt = "", issues = {}, pullRequests = {} },
	} },
	build = function(props: Props)
		local rows: { Element } = {}
		for _, group in groups do
			local items = {}
			for _, thread in props.threads do
				if thread.group == group.id then
					table.insert(items, thread)
				end
			end
			if #items > 0 then
				local folded = (props.folded or {})[group.id]
				table.insert(rows, group_header { title = group.title, count = #items, status = group.status,
					band = true, collapsed = folded, toggle = { fold = group.id }, to = props.to })
				if not folded then
					for _, thread in items do
						table.insert(rows, t3_thread { thread = thread, expanded = thread.id == props.selected,
							msg = { thread = thread.id }, to = props.to })
					end
				end
			end
		end
		return ui.column { gap = "none", table.unpack(rows) }
	end,
}
