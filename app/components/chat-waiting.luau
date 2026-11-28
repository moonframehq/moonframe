-- What waits on the user above an agent's composer: the agent's plan
-- (folded to "Plan 2/5 · current item"), its permission questions with
-- the agent's options, questions with choices in full, guests' prompts to
-- allow or deny, and the prompts queued behind the running turn: each can
-- be taken back or sent now (steered into the turn when the agent can),
-- and those held after a Stop wait for one or the other.
--
-- It draws parts of `app.chat`'s view, and its buttons send their messages
-- to the module that draws it: `{ answer = permission, option }`,
-- `{ answer_question = permission, answers }` (from `chat-question`),
-- `{ unqueue = id }`, `{ send_now = id }` and `plan_toggle`.

type Props = {
	view: any,
	chat: string?,
	plan_open: boolean?,
	plan_toggle: any,
}

local chat_question = use "chat-question"

local function plan_strip(plan: any, props: Props): any
	if plan == nil or plan.total == 0 then
		return nil
	end
	local open = props.plan_open == true
	local items: { any } = {}
	if open then
		for _, item in plan.items do
			local icon = if item.status == "done" then "circle-check" elseif item.status == "running" then "circle-dot" else "circle"
			table.insert(items, ui.row {
				gap = "small",
				align = "center",
				ui.icon { name = icon :: any, size = "small", class = "chat-waiting-plan-" .. item.status },
				ui.text { item.text, size = "small", strike = item.status == "done", wrap = true },
			})
		end
	end
	return ui.column {
		class = "chat-waiting-plan",
		gap = "small",
		ui.row {
			gap = "small",
			align = "center",
			msg = props.plan_toggle,
			ui.icon { name = if open then "chevron-down" else "list-checks", size = "small" },
			ui.text { `Plan {plan.done}/{plan.total}`, size = "small", weight = "medium" },
			not open and plan.current and ui.text { plan.current, size = "small", tone = "muted", truncate = true },
		},
		table.unpack(items),
	}
end

local function permission_card(permission: any, props: Props): any
	if permission.question then
		return chat_question { id = `q-{props.chat or "chat"}-{permission.id}`, permission = permission }
	end
	local buttons: { any } = {}
	for _, option in permission.options or {} do
		table.insert(buttons, ui.button {
			option.name,
			small = true,
			variant = if option.kind == "allow_once" then "primary" elseif option.allows then "outline" else "ghost",
			msg = { answer = permission.id, option = option.id },
		})
	end
	local guest = permission.guest
	return ui.column {
		class = "chat-waiting-permission",
		gap = "small",
		ui.row {
			gap = "small",
			align = "center",
			ui.icon { name = if guest then "user" else "shield", size = "small", class = "chat-waiting-permission-icon" },
			ui.text {
				if guest then `{guest}, a guest, asks to send` else permission.title,
				weight = "medium",
				wrap = true,
			},
		},
		guest and permission.guest_text and ui.text { permission.guest_text, size = "small", wrap = true, class = "chat-waiting-guest" },
		not guest and permission.tool and permission.tool ~= permission.title
			and ui.text { permission.tool, size = "small", class = "chat-waiting-permission-tool", wrap = true },
		ui.row { gap = "small", wrap = true, table.unpack(buttons) },
	}
end

local function queued_row(queued: any, view: any): any
	local working = view.status == "working"
	local label: string? = nil
	if queued.steered then
		label = "Steering"
	elseif queued.held then
		label = "Held"
	end
	-- Into the running turn when the agent steers; else the turn stops
	-- and this goes in its place. Held prompts wait for it.
	local now = (working or queued.held) and not queued.steered and ui.button {
		icon = if view.can_steer and working then "arrow-right" else "arrow-up",
		small = true,
		variant = "ghost",
		tooltip = if view.can_steer and working then "Steer it into this turn" elseif working then "Stop the turn and send this now" else "Send it now",
		msg = { send_now = queued.id },
	}
	return ui.row {
		key = queued.id,
		class = "chat-waiting-queued",
		state = if queued.held then "held" else nil,
		gap = "small",
		align = "center",
		ui.icon { name = if queued.steered then "arrow-right" else "clock", size = "small" },
		label and ui.text { label, size = "small", weight = "medium" },
		queued.guest and ui.text { queued.guest, size = "small", tone = "muted" },
		ui.column { grow = true, ui.text { queued.text, size = "small", truncate = true } },
		now,
		not queued.steered and ui.button { icon = "close", small = true, variant = "ghost", tooltip = "Take it back", msg = { unqueue = queued.id } },
	}
end

return component {
	title = "Chat Waiting",
	category = "Data",
	icon = "inbox",
	description = "What waits on the user above an agent's composer: its plan, permission questions with the agent's options, questions with choices, guests' prompts to allow, and queued prompts to take back, steer into the turn or send now.",
	when = "Between an agent conversation from `app.chat`'s view and its composer; `agent-chat` draws it already. For one run's question use `agent-run`; for a plain confirmation use `confirm`.",
	props = {
		["view"] = "the chat's view: its `plan`, `permissions`, `queue`, `status` and `can_steer`",
		["chat"] = "the chat's id, which keeps each question's instance its own",
		["plan_open"] = "the plan is listed item by item rather than folded",
		["plan_toggle"] = "message sent when the plan's line is clicked",
	},
	styles = {
		[".chat-waiting"] = { gap = 8 },
		[".chat-waiting-permission"] = { border = true, border_color = "warning/50", background = "warning/8", radius = 12, padding = { 10, 12 } },
		[".chat-waiting-permission-icon"] = { color = "warning" },
		[".chat-waiting-permission-tool"] = { font = "mono", color = "muted-foreground" },
		[".chat-waiting-guest"] = { background = "background", radius = 8, padding = { 6, 10 } },
		[".chat-waiting-queued"] = { background = "muted", radius = 8, padding = { 2, 4, 2, 10 }, color = "muted-foreground" },
		[".chat-waiting-queued:held"] = { color = "warning" },
		[".chat-waiting-plan"] = { border = true, border_color = "border", radius = 10, padding = { 6, 10 } },
		[".chat-waiting-plan-done"] = { color = "success" },
		[".chat-waiting-plan-running"] = { color = "accent" },
	},
	example = {
		chat = "example",
		plan_open = false,
		view = {
			status = "working",
			can_steer = true,
			plan = { done = 1, total = 3, current = "Write the tests", items = {} },
			permissions = {
				{
					id = 7,
					title = "Run a command?",
					tool = "rm -rf build",
					options = {
						{ id = "yes", name = "Allow", kind = "allow_once", allows = true },
						{ id = "no", name = "Reject", kind = "reject_once", allows = false },
					},
				},
			},
			queue = {
				{ id = "q1", text = "Also update the README", held = false, steered = false },
				{ id = "q2", text = "Use tabs, not spaces", held = false, steered = true },
			},
		},
	},

	build = function(props: Props)
		local view = props.view
		if view == nil then
			return nil
		end
		local children: { any } = {}
		local plan = plan_strip(view.plan, props)
		if plan then
			table.insert(children, plan)
		end
		for _, permission in view.permissions or {} do
			table.insert(children, permission_card(permission, props))
		end
		for _, queued in view.queue or {} do
			table.insert(children, queued_row(queued, view))
		end
		if #children == 0 then
			return nil
		end
		return ui.column { class = "chat-waiting", table.unpack(children) }
	end,
}
