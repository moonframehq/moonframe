-- A "Send feedback" dialog: a text area for what should change, a note on
-- who acts on it, Cancel and Send, and under them what came of earlier
-- requests ("Feedback & fixes"). Stateless: the host keeps the text, the
-- mod keeps `open` and the text from `change`, and sends the request on.

type Item = {
	id: string,
	text: string,
	-- "running", "done", "failed", "cancelled" or "lost".
	status: string,
	started: number?,
	finished: number?,
	result: string?,
}

type Props = {
	open: boolean?,
	dismiss: any,
	id: string?,
	value: string?,
	text: string?,
	placeholder: string?,
	note: string?,
	change: any,
	send: any,
	items: { Item }?,
	to: string?,
}

local PLACEHOLDER = "What should change? For example:\n• Focus the prompt when I open a session\n• Make deploys faster"

local STATUS = {
	running = { icon = "loader-circle", class = "feedback-running", label = "Working on it" },
	done = { icon = "circle-check", class = "feedback-done", label = "Done" },
	failed = { icon = "circle-x", class = "feedback-failed", label = "Failed" },
	cancelled = { icon = "circle-x", class = "feedback-muted", label = "Stopped" },
	lost = { icon = "circle-alert", class = "feedback-failed", label = "Stopped without an answer" },
}

-- "took 3 min", from when a request started and finished.
local function took(item: Item): string?
	if not item.started or not item.finished then
		return nil
	end
	local seconds = math.max(0, item.finished - item.started)
	if seconds < 60 then
		return `took {seconds}s`
	end
	return `took {math.floor(seconds / 60 + 0.5)} min`
end

-- The first line of a request, for its row.
local function first_line(text: string): string
	return (string.match(text, "^%s*([^\n]*)") or text)
end

local function row(item: Item, to: string?): any
	local status = STATUS[item.status] or STATUS.lost
	local running = item.status == "running"
	local detail = status.label
	local time = took(item)
	if time then
		detail ..= " · " .. time
	end
	return ui.row {
		key = item.id,
		class = "feedback-item",
		align = "start",
		ui.icon { name = status.icon :: any, class = status.class, spin = running },
		ui.column {
			grow = true,
			gap = "none",
			ui.text { first_line(item.text), class = "feedback-request", truncate = true, tooltip = item.text },
			ui.text { detail, class = "feedback-detail" },
			item.result ~= nil and ui.text { item.result, class = "feedback-result", lines = 4, tooltip = item.result },
		},
		if running
			then ui.row { class = "feedback-action", msg = { cancel = item.id }, to = to, tooltip = "Stop the agent", ui.icon "square" }
			else ui.row { class = "feedback-action", msg = { forget = item.id }, to = to, tooltip = "Remove", ui.icon "close" },
	}
end

return component {
	title = "Send Feedback",
	category = "Overlays",
	icon = "circle-help",
	description = "A dialog to ask an agent for a change to the app: a text area, a note on who acts on it, Cancel and Send, and what came of earlier requests.",
	when = "Let the user ask for a change to the app itself, from inside it, and see what came of it. For a plain form use `dialog`; for chatting with an agent use `composer`.",
	props = {
		["open"] = "show it",
		["dismiss"] = "msg sent by Cancel, Escape or a backdrop click",
		["id"] = "keeps the text across renders (default \"feedback\")",
		["value"] = "the text; set it to \"\" after sending to clear the field",
		["text"] = "the text the mod has from `change`; Send is greyed out while it is blank",
		["placeholder"] = "grey text while empty (default: two examples)",
		["note"] = "muted lines under the field: who acts on the feedback, and what goes with it",
		["change"] = "sent on each edit, with `value` set to the text",
		["send"] = "sent by Send and ⌘↩, with `value` set to the text from ⌘↩",
		["items"] = "earlier requests { { id, text, status, started?, finished?, result? } }, newest first; each row sends `{ cancel = id }` while running, else `{ forget = id }`",
		["to"] = "mod its messages go to, if not the drawing one",
	},
	styles = {
		[".feedback"] = { background = "background", border = "all", border_color = "border", radius = 12, padding = 0 },
		[".feedback-body"] = { padding = { 18, 20, 16, 20 }, gap = 12 },
		[".feedback-title"] = { weight = "bold", size = "large" },
		[".feedback-note"] = { size = "small", color = "muted-foreground" },
		[".feedback-history"] = { border = "top", border_color = "border", padding = { 12, 20, 14, 20 }, gap = 8 },
		[".feedback-heading"] = { size = "small", weight = "medium", color = "muted-foreground", case = "upper" },
		[".feedback-item"] = { gap = 10, padding = { 6, 8 }, radius = 8, transition = "slide-fade fast ease-out" },
		[".feedback-item:hover"] = { background = "list-hover" },
		[".feedback-request"] = { weight = "medium" },
		[".feedback-detail"] = { size = "small", color = "muted-foreground" },
		[".feedback-result"] = { size = "small", padding = { top = 4 } },
		[".feedback-running"] = { color = "accent" },
		[".feedback-done"] = { color = "success" },
		[".feedback-failed"] = { color = "danger" },
		[".feedback-muted"] = { color = "muted-foreground" },
		[".feedback-action"] = { padding = 4, radius = 6, color = "muted-foreground" },
		[".feedback-action:hover"] = { background = "list-hover", color = "foreground" },
	},
	usage = [[local feedback = use "feedback"

feedback { open = state.open, dismiss = "close", value = state.text, text = state.text,
	change = "type", send = "send", items = app.feedback and app.feedback.items,
	note = "A background agent changes the app and reports back below." }]],
	example = {
		open = true,
		dismiss = "close",
		text = "",
		note = "A background agent in ~/code/moonframe implements it and checks it, then reports back under Feedback & fixes. It gets what you're looking at, your window and the build.",
		items = {
			{ id = "2", text = "Make the status bar taller", status = "running", started = 0 },
			{
				id = "1",
				text = "Focus the prompt when I open a session",
				status = "done",
				started = 0,
				finished = 140,
				result = "The terminal in a new tab now takes focus: app/mods/editor/main.luau passes focus to it.",
			},
		},
	},

	build = function(props: Props)
		local blank = string.match(props.text or "", "^%s*$") ~= nil
		local items: { Item } = props.items or {}
		local rows = {}
		for _, item in items do
			table.insert(rows, row(item, props.to))
		end
		return ui.overlay {
			open = props.open,
			dismiss = props.dismiss,
			backdrop = true,
			width = 560,
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
					ui.row {
						gap = "normal",
						ui.spacer(),
						ui.button { "Cancel", variant = "outline", msg = props.dismiss, to = props.to },
						ui.button { "Send", variant = "primary", msg = props.send, to = props.to, disabled = blank },
					},
				},
				#rows > 0 and ui.column {
					class = "feedback-history",
					ui.text { "Feedback & fixes", class = "feedback-heading" },
					ui.column { gap = "small", max_height = 260, scroll = true, table.unpack(rows) },
				},
			},
		}
	end,
}
