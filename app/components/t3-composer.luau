local dropdown = use "dropdown"

type Props = {
	data: index<App, "t3"> & {},
	action: "start" | "send",
	id: string,
	environmentId: string,
	target: string?,
	title: string?,
	text: string,
	picker: boolean?,
}

return component {
	title = "T3 Composer",
	category = "Controls",
	icon = "messages-square",
	description = "Start a local T3 Code chat or send a follow-up to an existing thread.",
	when = "Draw inside the mod enabled as the T3 writer. Handle project, draft, picker and cancel messages locally and observe app.t3.results[id] for delivery.",
	props = {
		data = "The current app.t3 snapshot",
		action = "start or send",
		id = "Unique request id for this draft revision",
		environmentId = "Environment in which the draft was opened",
		target = "Project id for start, thread id for send",
		title = "Thread title when sending a follow-up",
		text = "The current draft",
		picker = "Whether the project picker is open",
	},
	example = {
		data = { available = true, environmentId = "preview", ticket = "preview", projects = {
			{ id = "mf", label = "Moonframe" },
		}, results = {}, threads = {}, total = 0 },
		action = "start", id = "example", environmentId = "preview", target = "mf", text = "",
	},
	build = function(props: Props)
		local data = props.data
		local result = data.results[props.id]
		local sending = result ~= nil and result.status == "sending"
		local start = props.action == "start"
		local targetAvailable = false
		if start then
			for _, project in data.projects do
				if project.id == props.target then targetAvailable = true end
			end
		else
			for _, thread in data.threads do
				if thread.id == props.target then targetAvailable = true end
			end
		end
		return ui.column {
			ui.row { align = "center",
				ui.text { if start then "New chat" else `Reply to {props.title or props.target}`, weight = "semibold" },
				start and dropdown { options = data.projects, selected = props.target, open = props.picker,
					toggle = "t3-picker", dismiss = "t3-dismiss", event = "project", disabled = sending,
					placeholder = if #data.projects == 0 then "No active projects" else "Choose project" },
			},
			ui.input { id = "t3-prompt", multiline = true, rows = 2, value = props.text, change = "draft",
				focus = true, disabled = sending, placeholder = if start then "What should the agent work on?" else "Message this thread…" },
			ui.row { justify = "end",
				ui.button { label = "Cancel", variant = "ghost", msg = "t3-cancel", disabled = sending },
				ui.button { label = if sending then "Sending…" elseif start then "Start chat" else "Send message",
					variant = "primary", to = "provider:t3",
					disabled = sending or not targetAvailable or not data.available or data.ticket == nil
						or props.environmentId ~= data.environmentId or #props.text > 65536 or string.find(props.text, "%S") == nil,
					msg = { action = props.action, target = props.target, id = props.id, ticket = data.ticket,
						environmentId = props.environmentId, text = props.text } },
			},
		}
	end,
}
