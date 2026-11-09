-- One agent run from `app.acp` (providers/acp): its status, the plan,
-- its latest tool calls, a permission question with the agent's options,
-- and its latest message, or why it failed. Its buttons talk to the `acp`
-- provider directly ({ cancel }, { answer, option }), which only obeys the
-- mod that started the run, so draw it from that mod.

type Tool = { id: string, title: string, kind: string, status: string }
type Option = { id: string, name: string, kind: string }

type Run = {
	id: string,
	agent: string,
	agent_title: string,
	text: string,
	-- "starting", "running", "waiting", "done", "failed", "cancelled" or "stopped".
	status: string,
	started: number?,
	finished: number?,
	error: string?,
	message: string?,
	tools: { Tool }?,
	tool_count: number?,
	plan: { { content: string, status: string } }?,
	question: { title: string, options: { Option } }?,
}

type Props = {
	run: Run,
	-- Lines of the agent's message shown; 0 hides it.
	lines: number?,
	-- Tool calls listed, newest last.
	tools: number?,
	-- Show the user's words above.
	request: boolean?,
	to: string?,
}

local STATUS = {
	starting = { icon = "loader-circle", tone = "agent-run-accent", label = "Starting", spin = true },
	running = { icon = "loader-circle", tone = "agent-run-accent", label = "Working", spin = true },
	waiting = { icon = "circle-alert", tone = "agent-run-warning", label = "Needs your answer" },
	done = { icon = "circle-check", tone = "agent-run-success", label = "Done" },
	failed = { icon = "circle-x", tone = "agent-run-danger", label = "Failed" },
	cancelled = { icon = "circle-x", tone = "agent-run-muted", label = "Stopped" },
	stopped = { icon = "circle-alert", tone = "agent-run-muted", label = "Stopped when the app quit" },
}

local TOOL_STATUS = {
	pending = { icon = "circle-dashed", tone = "agent-run-muted" },
	in_progress = { icon = "loader-circle", tone = "agent-run-accent", spin = true },
	completed = { icon = "circle-check", tone = "agent-run-success" },
	failed = { icon = "circle-x", tone = "agent-run-danger" },
}

local PLAN_STATUS = {
	pending = "circle",
	in_progress = "circle-dot",
	completed = "circle-check",
}

local AGENT_ICONS = {
	claude = "agent-claude",
	codex = "agent-codex",
	gemini = "agent-gemini",
	copilot = "agent-copilot",
	opencode = "agent-opencode",
}

-- "2 min", from the run's times.
local function took(run: Run): string?
	if not run.started or not run.finished then
		return nil
	end
	local seconds = math.max(0, run.finished - run.started)
	if seconds < 60 then
		return `{seconds}s`
	end
	return `{math.floor(seconds / 60 + 0.5)} min`
end

local function variant(kind: string): any
	if kind == "allow_always" or kind == "allow_once" then
		return if kind == "allow_once" then "primary" else "outline"
	end
	return "ghost"
end

return component {
	title = "Agent Run",
	category = "Agents",
	icon = "bot",
	description = "One coding agent's run: status, plan, its latest tool calls, a permission question with the agent's options, and its answer.",
	when = "Show what an agent started through `app.acp` is doing or did, with Stop and the answers to its permission questions. For the Send feedback dialog, which lists runs with this, use `feedback`; for a chat transcript use `chat-message`.",
	props = {
		["run"] = "a run from `app.acp.runs` { id, agent, agent_title, text, status, started, finished, error, message, tools, tool_count, plan, question }",
		["lines"] = "lines of the agent's message shown (default 6; 0 hides it)",
		["tools"] = "tool calls listed, newest last (default 4)",
		["request"] = "show the user's words above",
		["to"] = "where its buttons send, default `provider:acp`",
	},
	styles = {
		[".agent-run"] = { gap = 8 },
		[".agent-run-head"] = { gap = 6 },
		[".agent-run-status"] = { size = "small", weight = "medium" },
		[".agent-run-meta"] = { size = "small", color = "muted-foreground" },
		[".agent-run-request"] = { weight = "medium" },
		[".agent-run-tool"] = { gap = 6 },
		[".agent-run-tool-title"] = { size = "small", font = "mono", color = "muted-foreground" },
		[".agent-run-plan"] = { gap = 2, padding = { 6, 8 }, radius = 6, background = "muted/40" },
		[".agent-run-step"] = { size = "small" },
		[".agent-run-question"] = {
			gap = 8,
			padding = 10,
			radius = 8,
			border = true,
			border_color = "warning/50",
			background = "warning/8",
		},
		[".agent-run-message"] = { size = "small" },
		[".agent-run-error"] = { size = "small", color = "danger" },
		[".agent-run-accent"] = { color = "accent" },
		[".agent-run-success"] = { color = "success" },
		[".agent-run-warning"] = { color = "warning" },
		[".agent-run-danger"] = { color = "danger" },
		[".agent-run-muted"] = { color = "muted-foreground" },
	},
	example = {
		request = true,
		run = {
			id = "1",
			agent = "claude",
			agent_title = "Claude",
			text = "Make the status bar taller",
			status = "waiting",
			started = 0,
			tool_count = 3,
			tools = {
				{ id = "a", title = "Read app/main.luau", kind = "read", status = "completed" },
				{ id = "b", title = "Edit app/main.luau", kind = "edit", status = "pending" },
			},
			plan = {
				{ content = "Find the status bar's row", status = "completed" },
				{ content = "Make it 36px tall", status = "in_progress" },
				{ content = "Run just check", status = "pending" },
			},
			question = {
				title = "Edit app/main.luau",
				options = {
					{ id = "allow", name = "Allow", kind = "allow_once" },
					{ id = "always", name = "Always allow", kind = "allow_always" },
					{ id = "reject", name = "Reject", kind = "reject_once" },
				},
			},
			message = "The status bar is the row at the bottom of app/main.luau; I'll raise its height.",
		},
	},

	build = function(props: Props)
		local run = props.run
		local to = props.to or "provider:acp"
		local status = STATUS[run.status] or STATUS.stopped
		local going = run.status == "starting" or run.status == "running" or run.status == "waiting"

		local meta = run.agent_title
		local count = run.tool_count or 0
		if count > 0 then
			meta ..= ` · {count} {if count == 1 then "step" else "steps"}`
		end
		local time = took(run)
		if time then
			meta ..= " · " .. time
		end

		local plan: { any } = {}
		local steps: { { content: string, status: string } } = run.plan or {}
		for _, step in steps do
			table.insert(
				plan,
				ui.row {
					gap = "small",
					align = "center",
					ui.icon { name = (PLAN_STATUS[step.status] or "circle") :: any, class = "agent-run-muted", size = "small" },
					ui.text {
						step.content,
						class = "agent-run-step",
						truncate = true,
						strike = step.status == "completed",
					},
				}
			)
		end

		local tools: { any } = {}
		local list: { Tool } = run.tools or {}
		local first = math.max(1, #list - (props.tools or 4) + 1)
		for i = first, #list do
			local tool = list[i]
			local look = TOOL_STATUS[tool.status] or TOOL_STATUS.pending
			table.insert(
				tools,
				ui.row {
					class = "agent-run-tool",
					align = "center",
					ui.icon { name = look.icon :: any, class = look.tone, size = "small", spin = look.spin },
					-- One line: a command's title can hold a whole script,
					-- whose other lines would spill over the rows under it.
					ui.text {
						string.match(tool.title, "^%s*([^\n]*)") or tool.title,
						class = "agent-run-tool-title",
						truncate = true,
						tooltip = tool.title,
					},
				}
			)
		end

		local question: any = false
		if run.question then
			local buttons = {}
			for _, option in run.question.options do
				table.insert(
					buttons,
					ui.button {
						option.name,
						small = true,
						variant = variant(option.kind),
						msg = { answer = run.id, option = option.id },
						to = to,
					}
				)
			end
			question = ui.column {
				class = "agent-run-question",
				ui.row {
					gap = "small",
					align = "center",
					ui.icon { name = "circle-alert", class = "agent-run-warning" },
					ui.text { `{run.agent_title} asks to: {run.question.title}`, weight = "medium", wrap = true },
				},
				ui.row { gap = "small", wrap = true, table.unpack(buttons) },
			}
		end

		local lines = props.lines or 6
		return ui.column {
			class = "agent-run",
			props.request and ui.text { run.text, class = "agent-run-request", lines = 3, tooltip = run.text },
			ui.row {
				class = "agent-run-head",
				align = "center",
				ui.icon { name = (AGENT_ICONS[run.agent] or "bot") :: any, size = "small" },
				ui.icon { name = status.icon :: any, class = status.tone, size = "small", spin = status.spin },
				ui.text { status.label, class = "agent-run-status " .. status.tone },
				ui.text { meta, class = "agent-run-meta", truncate = true },
				ui.spacer(),
				going and ui.button {
					"Stop",
					icon = "square",
					small = true,
					variant = "ghost",
					msg = { cancel = run.id },
					to = to,
					tooltip = "Stop the agent",
				},
			},
			#plan > 0 and ui.column { class = "agent-run-plan", table.unpack(plan) },
			#tools > 0 and ui.column { gap = "small", table.unpack(tools) },
			question,
			run.error ~= nil and ui.text { run.error, class = "agent-run-error", wrap = true, lines = 4, tooltip = run.error },
			lines > 0 and run.message ~= nil and ui.text {
				run.message,
				class = "agent-run-message",
				wrap = true,
				lines = lines,
				tooltip = run.message,
			},
		}
	end,
}
