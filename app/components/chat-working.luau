-- What an agent is doing now, under its conversation: a spinner with the
-- step it is on and how long the turn has run, "Waiting for you" while a
-- question waits, and the sub-agents and background shells still running,
-- each with its time and, when the agent can end it alone, a Stop button.
-- The list stays after the turn ends while anything still runs.
--
-- It draws parts of `app.chat`'s view; its buttons send their messages to
-- the module that draws it: `{ stop_activity = call_id }` and `toggle`.

type Props = {
	working: boolean?,
	waiting: boolean?,
	step: string?,
	working_for: string?,
	activities: { any }?,
	open: boolean?,
	toggle: any,
}

local KIND_ICONS: { [string]: string } = {
	subagent = "bot",
	shell = "terminal",
	monitor = "activity",
	other = "loader",
}

local function activity_row(activity: any): any
	return ui.row {
		key = activity.call_id,
		class = "chat-working-activity",
		gap = "small",
		align = "center",
		ui.icon { name = (KIND_ICONS[activity.kind] or "loader") :: any, size = "small" },
		ui.column {
			grow = true,
			ui.text { activity.label, size = "small", truncate = true },
			activity.detail and ui.text { activity.detail, size = "small", tone = "muted", truncate = true },
		},
		activity.running_for and ui.text { activity.running_for, size = "small", tone = "muted", class = "chat-working-time" },
		activity.stoppable and ui.button {
			icon = "circle-stop",
			small = true,
			variant = "ghost",
			tooltip = "Stop this one; the turn goes on",
			msg = { stop_activity = activity.call_id },
		},
	}
end

-- "2 sub-agents · 1 background shell".
local function summary(activities: { any }): string
	local counts: { [string]: number } = {}
	for _, activity in activities do
		counts[activity.kind] = (counts[activity.kind] or 0) + 1
	end
	local parts: { string } = {}
	local names = {
		{ "subagent", "sub-agent", "sub-agents" },
		{ "shell", "background shell", "background shells" },
		{ "monitor", "monitor", "monitors" },
		{ "other", "background task", "background tasks" },
	}
	for _, name in names do
		local count = counts[name[1]]
		if count then
			table.insert(parts, `{count} {if count == 1 then name[2] else name[3]}`)
		end
	end
	return table.concat(parts, " · ")
end

return component {
	title = "Chat Working",
	category = "Data",
	icon = "loader-circle",
	description = "What a coding agent is doing now: a spinner with its current step and how long the turn has run, or \"Waiting for you\", and its running sub-agents and background shells with their times and Stop buttons.",
	when = "Under an agent conversation drawn from `app.chat`'s view; `agent-chat` draws it already. For one run's status use `agent-run`; for a plain spinner use `loading`.",
	props = {
		["working"] = "a turn runs now",
		["waiting"] = "a question waits for the user, so the agent is paused",
		["step"] = "what the agent is doing, such as the running tool's title",
		["working_for"] = "how long the turn has run (`view.working_for`)",
		["activities"] = "`view.activities`: { call_id, kind, label, detail?, running_for?, stoppable }",
		["open"] = "the activities are listed one per line rather than counted",
		["toggle"] = "message sent when the activity count is clicked",
	},
	styles = {
		[".chat-working"] = { color = "muted-foreground", padding = { 0, 2 }, gap = 6 },
		[".chat-working-list"] = { border = true, border_color = "border", radius = 10, padding = { 4, 8 }, gap = 2 },
		[".chat-working-activity"] = { padding = { 2, 0 } },
		[".chat-working-time"] = { font = "mono" },
	},
	example = {
		working = true,
		step = "Running cargo test",
		working_for = "42s",
		open = true,
		activities = {
			{ call_id = "a", kind = "subagent", label = "Explore the chat crates", detail = "Reading lib.rs", running_for = "1m 05s", stoppable = true },
			{ call_id = "b", kind = "shell", label = "npm run dev", running_for = "3m 12s", stoppable = false },
		},
	},

	build = function(props: Props)
		local activities = props.activities or {}
		if not props.working and #activities == 0 then
			return nil
		end
		local label: string
		if props.waiting then
			label = "Waiting for you"
		elseif props.working then
			label = if props.step then `{props.step}…` else "Working…"
		else
			label = "Waiting for background work"
		end
		local list: { any } = {}
		if props.open then
			for _, activity in activities do
				table.insert(list, activity_row(activity))
			end
		end
		return ui.column {
			class = "chat-working",
			ui.row {
				gap = "small",
				align = "center",
				ui.icon {
					name = if props.waiting then "circle-pause" else "loader-circle",
					size = "small",
					spin = not props.waiting,
				},
				ui.column { grow = true, ui.text { label, size = "small", truncate = true } },
				props.working and props.working_for and ui.text { props.working_for, size = "small", class = "chat-working-time" },
				#activities > 0 and ui.button {
					summary(activities),
					icon = if props.open then "chevron-down" else "chevron-right",
					small = true,
					variant = "ghost",
					msg = props.toggle,
				},
			},
			#list > 0 and ui.column { class = "chat-working-list", table.unpack(list) },
		}
	end,
}
