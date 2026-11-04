-- A "Make it yours" dialog: a text area for what should change, a note on
-- who acts on it, the mod's own controls (an agent picker), Cancel and Send,
-- and under them what came of earlier requests ("Feedback & fixes"): agent
-- runs from `app.acp`, each request with its follow-ups (runs whose `thread`
-- is its id) as one conversation that opens into `agent-run`s and a box to
-- reply in. Stateless: the host keeps the text, the mod keeps `open`, the
-- text from `change`, the reply and which conversation is open, and sends
-- the requests on.
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
	-- The conversation shown open: its first run's id.
	expanded: string?,
	-- A box under an open conversation to reply in: sends `{ follow = id }`
	-- with the first run's id (and `value` from ⌘↩), and `{ follow_type =
	-- id, value }` on each edit; `reply` is the text the mod has.
	follow: boolean?,
	reply: string?,
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

-- Runs (newest first) as conversations, newest first: each request with
-- its follow-ups after it, oldest first. A follow-up whose request was
-- forgotten stands on its own.
local function conversations(runs: { any }): { { any } }
	local by: { [string]: { any } } = {}
	local order: { { any } } = {}
	for _, run in runs do
		if run.thread == nil then
			by[run.id] = { run }
			table.insert(order, by[run.id])
		end
	end
	for index = #runs, 1, -1 do
		local run = runs[index]
		if run.thread ~= nil and by[run.thread] then
			table.insert(by[run.thread], run)
		end
	end
	for _, run in runs do
		if run.thread ~= nil and not by[run.thread] then
			table.insert(order, { run })
		end
	end
	return order
end

-- One conversation: the request's row, opening into each run (the user's
-- words above every follow-up) and a box to reply in.
local function conversation(turns: { any }, open: boolean, props: Props): any
	local first, run = turns[1], turns[#turns]
	local to = props.to
	local status = STATUS[run.status] or STATUS.stopped
	local going = run.status == "starting" or run.status == "running" or run.status == "waiting"
	local body = {}
	if open then
		for index, turn in turns do
			if index > 1 then
				table.insert(body, ui.text { turn.text, class = "feedback-said", wrap = true })
			end
			local latest = turn == run
			table.insert(body, agent_run { run = turn, lines = if latest then 10 else 4, tools = if latest then 6 else 3 })
			if turn.warning then
				table.insert(body, ui.text { turn.warning, class = "feedback-error", wrap = true })
			end
		end
		local blank = string.match(props.reply or "", "^%s*$") ~= nil
		if props.follow and not going then
			table.insert(body, ui.row {
				class = "feedback-reply",
				align = "end",
				ui.input {
					id = "feedback-reply-" .. first.id,
					value = props.reply,
					placeholder = "Not quite? Tell the agent what is still wrong…",
					multiline = true,
					rows = 2,
					grow = true,
					change = { follow_type = first.id },
					submit = not blank and { follow = first.id } or nil,
					to = to,
				},
				ui.button { "Reply", variant = "primary", msg = { follow = first.id }, to = to, disabled = blank },
			})
		end
	end
	return ui.column {
		key = "run-" .. first.id,
		class = "feedback-item",
		gap = "small",
		ui.row {
			class = "feedback-item-head",
			align = "start",
			msg = { expand = first.id },
			to = to,
			ui.icon { name = status.icon :: any, class = status.class, spin = status.spin },
			ui.column {
				grow = true,
				gap = "none",
				ui.text { first_line(first.text), class = "feedback-request", truncate = true, tooltip = first.text },
				not open and ui.text {
					if #turns > 1 then `{#turns} turns · {detail(run)}` else detail(run),
					class = "feedback-detail",
					truncate = true,
				},
			},
			not going and ui.row {
				class = "feedback-action",
				msg = { forget_thread = first.id },
				to = to,
				tooltip = "Remove",
				ui.icon "close",
			},
			ui.icon { name = if open then "chevron-down" else "chevron-right", class = "feedback-muted" },
		},
		open and ui.column { class = "feedback-run", gap = "normal", table.unpack(body) },
	}
end

return component {
	title = "Send Feedback",
	category = "Agents",
	icon = "sparkles",
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
		["runs"] = "runs from `app.acp.runs` the mod started, newest first; runs with a `thread` show under that run as one conversation. A row sends `{ expand = id }`, its × `{ forget_thread = id }` (the mod forgets each run of it)",
		["expanded"] = "id of the conversation shown open (its first run), each run with `agent-run`; one waiting on an answer is always open",
		["follow"] = "show a box to reply in under the open conversation: Reply sends `{ follow = id }` (⌘↩ with `value`), each edit `{ follow_type = id, value }`; start the follow-up with `thread = id`",
		["reply"] = "the reply the mod has from `follow_type`; Reply is greyed out while it is blank",
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
		[".feedback-said"] = { padding = { 6, 10 }, radius = 8, background = "accent/8", size = "small" },
		[".feedback-reply"] = { gap = 8 },
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
		expanded = "1",
		follow = true,
		runs = ({
			{
				id = "2",
				thread = "1",
				agent = "claude",
				agent_title = "Claude",
				text = "It is still the same height in my window",
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
				text = "Make the status bar taller",
				status = "done",
				started = 0,
				finished = 140,
				tool_count = 5,
				message = "The status bar is 28px tall now, in app/main.luau.",
			},
		} :: { any }),
	},

	build = function(props: Props)
		local blank = string.match(props.text or "", "^%s*$") ~= nil
		local runs: { any } = props.runs or {}
		local rows = {}
		for _, turns in conversations(runs) do
			local open = turns[1].id == props.expanded or turns[#turns].status == "waiting"
			table.insert(rows, conversation(turns, open, props))
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
					ui.text { "Make it yours", class = "feedback-title" },
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
