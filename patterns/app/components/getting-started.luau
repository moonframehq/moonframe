local progress = use "progress"
local task_check = use "task-check"
local function loose(items: { any }): { any } return items end

return component {
	title = "Getting started",
	category = "Onboarding",
	icon = "rocket",
	description = "Make the next useful step feel obvious.",
	when = "Offer one next action, keep completed steps visible, and let people return to setup without interrupting their work. Put in a sidebar or a popover.",
	props = {
		["title"] = "Checklist heading.",
		["steps"] = "List of { id, title, done?, detail?, content? }; content is an element for the expanded step.",
		["expanded"] = "Id of the expanded step.",
		["event"] = "Key sent when selecting a step; default step.",
		["to"] = "Message recipient, optional.",
	},
	styles = {
		[".getting-started"] = { background = "card", border = "all", border_color = "border", radius = 12, padding = 18, gap = 14 },
		[".getting-started-row"] = { padding = { 7, 0 }, gap = 10, radius = 6 },
		[".getting-started-row:hover"] = { background = "list-hover" },
		[".getting-started-content"] = { padding = { 4, 0, 10, 28 }, gap = 10 },
	},
	example = { title = "Make yourself at home", steps = loose { { id = "workspace", title = "Create a workspace", done = true }, { id = "source", title = "Connect your first source", detail = "Bring your tasks into one place." } }, expanded = "source" },
	build = function(props: any)
		local steps = props.steps or {}
		local done = 0
		local rows = {}
		for _, step in steps do
			if step.done then done += 1 end
			table.insert(rows, ui.column { gap = "none",
				ui.row { class = "getting-started-row", align = "center", msg = { [props.event or "step"] = step.id }, to = props.to,
					task_check { state = if step.done then "done" else "open" },
					ui.text { step.title, tone = if step.done then "muted" else "normal" },
					ui.spacer(), ui.icon { name = if props.expanded == step.id then "chevron-down" else "chevron-right", size = "small", tone = "muted" },
				},
				props.expanded == step.id and ui.column { class = "getting-started-content",
					step.detail and ui.text { step.detail, tone = "muted", size = "small" }, step.content,
				},
			})
		end
		return ui.column { class = "getting-started",
			ui.row { align = "center", gap = "small", ui.icon "rocket", ui.text { props.title or "Getting started", weight = "medium" }, ui.spacer(), ui.text { `{done} / {#steps}`, tone = "muted", size = "small" } },
			progress { value = if #steps > 0 then done / #steps else 0 },
			ui.column { gap = "small", table.unpack(rows) },
		}
	end,
}
