-- A "Send feedback" dialog: a text area for what should change, a note on
-- who acts on it, the mod's own controls (an agent picker), Cancel and Send,
-- and under them what came of earlier requests ("Feedback & fixes"): agent
-- runs from `app.acp`, each opening into `agent-run`. Stateless: the host
-- keeps the text, the mod keeps `open`, the text from `change` and which
-- run is open, and sends the request on.
local agent_run = use "agent-run"

type Props = {
	open: boolean?,
	dismiss: any,
	id: string?,
	value: string?,
	text: string?,
	placeholder: string?,
	note: string?,
	-- A problem with the last send, shown above the buttons.
	error: string?,
	change: any,
	send: any,
	-- Runs from `app.acp.runs`, newest first.
	runs: { any }?,
	-- The run shown open.
	expanded: string?,
	-- Elements on the footer's left (an agent picker).
	actions: { any }?,
	-- An Inspect button: the app's Inspect Element, for the mod to attach
	-- what the user picks (it hears it on `app/inspect`).
	inspect: boolean?,
	-- The element attached, { element, at }, shown as a chip.
	attached: { element: string, at: string? }?,
	-- Sent by the chip's ×.
	detach: any,
	to: string?,
}

local PLACEHOLDER = "What should change? For example:\n• Focus the prompt when I open a session\n• Make deploys faster"

local STATUS = {
	starting = { icon = "loader-circle", class = "feedback-running", label = "Starting", spin = true },
	running = { icon = "loader-circle", class = "feedback-running", label = "Working on it", spin = true },
	waiting = { icon = "circle-alert", class = "feedback-waiting", label = "Needs your answer" },
	done = { icon = "circle-check", class = "feedback-done", label = "Done" },
	failed = { icon = "circle-x", class = "feedback-failed", label = "Failed" },
	cancelled = { icon = "circle-x", class = "feedback-muted", label = "Stopped" },
	stopped = { icon = "circle-alert", class = "feedback-muted", label = "Stopped when the app quit" },
}

-- `mods/status/main.luau:33` from a whole path.
local function short_at(at: string): string
	return string.match(at, "([^/]+/[^/]+/[^/]+)$") or at
end

-- The first line of a request, for its row.
local function first_line(text: string): string
	return (string.match(text, "^%s*([^\n]*)") or text)
end

-- What a closed row says under the request: what the agent does now, or
-- how it ended.
local function detail(run: any): string
	local status = STATUS[run.status] or STATUS.stopped
	local tools = run.tools or {}
	local last = tools[#tools]
	if run.status == "running" and last then
		return last.title
	elseif run.status == "failed" and run.error then
		return run.error
	elseif run.status == "done" and run.message then
		return first_line(run.message)
	end
	return `{status.label} · {run.agent_title}`
end

local function row(run: any, open: boolean, to: string?): any
	local status = STATUS[run.status] or STATUS.stopped
	local going = run.status == "starting" or run.status == "running" or run.status == "waiting"
	return ui.column {
		key = "run-" .. run.id,
		class = "feedback-item",
		gap = "small",
		ui.row {
			class = "feedback-item-head",
			align = "start",
			msg = { expand = run.id },
			to = to,
			ui.icon { name = status.icon :: any, class = status.class, spin = status.spin },
			ui.column {
				grow = true,
				gap = "none",
				ui.text { first_line(run.text), class = "feedback-request", truncate = true, tooltip = run.text },
				not open and ui.text { detail(run), class = "feedback-detail", truncate = true },
			},
			not going and ui.row {
				class = "feedback-action",
				msg = { forget = run.id },
				to = "provider:acp",
				tooltip = "Remove",
				ui.icon "close",
			},
			ui.icon { name = if open then "chevron-down" else "chevron-right", class = "feedback-muted" },
		},
		open and ui.column { class = "feedback-run", agent_run { run = run, lines = 10, tools = 6 } },
	}
end

return component {
	title = "Send Feedback",
	category = "Agents",
	icon = "circle-help",
	description = "A dialog to ask an agent for a change to the app: a text area, a note on who acts on it, Cancel and Send, and the agent runs that came of earlier requests.",
	when = "Let the user ask for a change to the app itself, from inside it, and follow what the agent does (`app.acp`). For one run on its own use `agent-run`; for a plain form use `dialog`; for chatting with an agent use `composer`.",
	props = {
		["open"] = "show it",
		["dismiss"] = "msg sent by Cancel, Escape or a backdrop click",
		["id"] = "keeps the text across renders (default \"feedback\")",
		["value"] = "the text; set it to \"\" after sending to clear the field",
		["text"] = "the text the mod has from `change`; Send is greyed out while it is blank",
		["placeholder"] = "grey text while empty (default: two examples)",
		["note"] = "muted lines under the field: who acts on the feedback, and what goes with it",
		["error"] = "why the last send failed, shown above the buttons",
		["change"] = "sent on each edit, with `value` set to the text",
		["send"] = "sent by Send and ⌘↩, with `value` set to the text from ⌘↩",
		["runs"] = "runs from `app.acp.runs` the mod started, newest first; a row sends `{ expand = id }`, its × `{ forget = id }` to `provider:acp`",
		["expanded"] = "id of the run shown open, with `agent-run`; one waiting on an answer is always open",
		["actions"] = "elements on the footer's left, such as an agent `dropdown`",
		["inspect"] = "show an Inspect button that starts Inspect Element for the mod (`command = \"inspect\"`); the pick comes back on the topic `app/inspect`",
		["attached"] = "the element attached to the request, `{ element, at }`, shown as a chip",
		["detach"] = "msg the chip's × sends",
		["to"] = "mod its own messages go to, if not the drawing one",
	},
	styles = {
		[".feedback"] = { background = "background", border = "all", border_color = "border", radius = 12, padding = 0 },
		[".feedback-body"] = { padding = { 18, 20, 16, 20 }, gap = 12 },
		[".feedback-title"] = { weight = "bold", size = "large" },
		[".feedback-note"] = { size = "small", color = "muted-foreground" },
		[".feedback-error"] = { size = "small", color = "danger" },
		[".feedback-chip"] = {
			gap = 6,
			padding = { 4, 6, 4, 10 },
			radius = 8,
			background = "accent/8",
			border = true,
			border_color = "accent/25",
		},
		[".feedback-chip-element"] = { size = "small", font = "mono" },
		[".feedback-chip-at"] = { size = "small", color = "muted-foreground" },
		[".feedback-history"] = { border = "top", border_color = "border", padding = { 12, 20, 14, 20 }, gap = 8 },
		[".feedback-heading"] = { size = "small", weight = "medium", color = "muted-foreground", case = "upper" },
		[".feedback-item"] = { radius = 8, transition = "slide-fade fast ease-out" },
		[".feedback-item-head"] = { gap = 10, padding = { 6, 8 }, radius = 8 },
		[".feedback-item-head:hover"] = { background = "list-hover" },
		[".feedback-run"] = { padding = { 0, 8, 8, 34 } },
		[".feedback-request"] = { weight = "medium" },
		[".feedback-detail"] = { size = "small", color = "muted-foreground" },
		[".feedback-running"] = { color = "accent" },
		[".feedback-waiting"] = { color = "warning" },
		[".feedback-done"] = { color = "success" },
		[".feedback-failed"] = { color = "danger" },
		[".feedback-muted"] = { color = "muted-foreground" },
		[".feedback-action"] = { padding = 4, radius = 6, color = "muted-foreground" },
		[".feedback-action:hover"] = { background = "list-hover", color = "foreground" },
	},
	usage = [[local feedback = use "feedback"

feedback { open = state.open, dismiss = "close", value = state.text, text = state.text,
	change = "type", send = "send", runs = app.acp and app.acp.runs, expanded = state.expanded,
	note = "A background agent changes the app and reports back below." }]],
	example = {
		open = true,
		dismiss = "close",
		text = "",
		inspect = true,
		attached = { element = 'text.status-text "⎇ main +29"', at = "/code/moonframe/app/mods/status/main.luau:33" },
		note = "A background agent (Claude) in ~/code/moonframe implements it, asking you here before each change, then reports back under Feedback & fixes. It gets what you're looking at, your window and the build.",
		expanded = "2",
		runs = ({
			{
				id = "2",
				agent = "claude",
				agent_title = "Claude",
				text = "Make the status bar taller",
				status = "running",
				started = 0,
				tool_count = 2,
				tools = {
					{ id = "a", title = "Read app/main.luau", kind = "read", status = "completed" },
					{ id = "b", title = "Edit app/main.luau", kind = "edit", status = "in_progress" },
				},
			},
			{
				id = "1",
				agent = "claude",
				agent_title = "Claude",
				text = "Focus the prompt when I open a session",
				status = "done",
				started = 0,
				finished = 140,
				tool_count = 5,
				message = "New terminal tabs take focus now: app/mods/editor/main.luau passes focus to the terminal it opens.",
			},
		} :: { any }),
	},

	build = function(props: Props)
		local blank = string.match(props.text or "", "^%s*$") ~= nil
		local runs: { any } = props.runs or {}
		local rows = {}
		for _, run in runs do
			local open = run.id == props.expanded or run.status == "waiting"
			table.insert(rows, row(run, open, props.to))
		end
		-- The mod's controls on the left, Cancel and Send on the right.
		local footer: { [any]: any } = { gap = "normal", align = "center" }
		if props.inspect then
			table.insert(footer, ui.button {
				"Inspect",
				icon = "inspector",
				variant = "outline",
				command = "inspect",
				tooltip = "Pick an element in the window to attach",
			})
		end
		local actions: { any } = props.actions or {}
		for _, action in actions do
			table.insert(footer, action)
		end
		table.insert(footer, ui.spacer())
		table.insert(footer, ui.button { "Cancel", variant = "outline", msg = props.dismiss, to = props.to })
		table.insert(
			footer,
			ui.button { "Send", variant = "primary", msg = props.send, to = props.to, disabled = blank }
		)
		return ui.overlay {
			open = props.open,
			dismiss = props.dismiss,
			backdrop = true,
			width = 600,
			to = props.to,
			keys = { escape = props.dismiss },
			class = "feedback",
			content = ui.column {
				gap = "none",
				ui.column {
					class = "feedback-body",
					ui.text { "Send feedback", class = "feedback-title" },
					ui.input {
						id = props.id or "feedback",
						value = props.value,
						placeholder = props.placeholder or PLACEHOLDER,
						multiline = true,
						rows = 6,
						focus = props.open,
						change = props.change,
						submit = not blank and props.send or nil,
						to = props.to,
					},
					props.note ~= nil and ui.text { props.note, class = "feedback-note", wrap = true },
					props.error ~= nil and ui.text { props.error, class = "feedback-error", wrap = true },
					props.attached ~= nil and ui.row {
						class = "feedback-chip",
						align = "center",
						self_align = "start",
						ui.icon { name = "inspector", class = "feedback-running", size = "small" },
						ui.text { props.attached.element, class = "feedback-chip-element", truncate = true },
						props.attached.at ~= nil and ui.text {
							short_at(props.attached.at),
							class = "feedback-chip-at",
							truncate = true,
							tooltip = props.attached.at,
						},
						ui.row {
							class = "feedback-action",
							msg = props.detach,
							to = props.to,
							tooltip = "Don't attach it",
							ui.icon "close",
						},
					},
					ui.row(footer),
				},
				#rows > 0 and ui.column {
					class = "feedback-history",
					ui.text { "Feedback & fixes", class = "feedback-heading" },
					ui.column { gap = "none", max_height = 360, scroll = true, table.unpack(rows) },
				},
			},
		}
	end,
}
