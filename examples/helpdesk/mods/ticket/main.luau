-- The open ticket: its header (ID, link, Assign), title and chips, who asked
-- and on what device, the assistant's summary, the SLA's two clocks, the
-- assistant's check on the device with its suggestion, the chat thread the
-- ticket came from, and a reply box (a reply or an internal note).
-- Follows `desk/selected`; tells the provider which ticket is on screen so a
-- check still running can finish.

local avatar = use "avatar"

type State = {
	selected: string?,
	tab: string,
	draft: string,
	earlier: boolean,
	assigned: { [string]: boolean },
}

local SOURCES = {
	slack = { icon = "hash", color = "#e2457a", name = "Slack" },
	teams = { icon = "message-square", color = "#6264a7", name = "Teams" },
	web = { icon = "globe", color = "#7d7d8a", name = "the web" },
}

local LABEL = 168
local BAR = 228

local function initials(name: string): string
	local first, last = string.match(name, "^(%S)%S*%s+(%S)")
	return if first and last then first .. last else string.sub(name, 1, 1)
end

local function first_name(name: string): string
	return string.match(name, "^(%S+)") or name
end

-- The assistant's mark: a small violet tile with sparkles.
local function mark(side: number)
	return ui.row {
		class = "mark",
		width = side,
		height = side,
		align = "center",
		justify = "center",
		shrink = false,
		ui.icon { name = "sparkles", size = if side < 20 then "small" else nil },
	}
end

local function find(desk, id: string?)
	for _, ticket in desk.tickets do
		if ticket.id == id then
			return ticket
		end
	end
	return nil
end

return mod {
	region = "detail",
	title = "Ticket",
	subscribe = { "desk/selected" },

	styles = {
		[".head"] = { padding = { 0, 30 }, height = 66, gap = 10, border = "bottom", border_color = "border" },
		[".head-link"] = { color = "muted-foreground", padding = 4, radius = 6 },
		[".head-link:hover"] = { background = "list-hover" },
		[".outline"] = {
			border = true,
			border_color = "border",
			radius = 9,
			background = "background",
			padding = { 0, 12 },
			height = 34,
			gap = 8,
		},
		[".outline:hover"] = { background = "secondary" },
		[".body"] = { padding = { 28, 30, 24, 30 }, gap = 0 },
		[".title"] = { size = "title", weight = "medium" },
		[".chips"] = { gap = 8, margin = { top = 14, bottom = 26 } },
		[".chip"] = { border = true, border_color = "border", radius = 8, padding = { 4, 10 }, gap = 7 },
		[".chip-dot"] = { radius = 3, background = "low" },
		[".chip-bar"] = { radius = 2 },
		[".bar-high"] = { background = "high" },
		[".bar-medium"] = { background = "medium" },
		[".bar-low"] = { background = "low" },
		[".muted"] = { color = "muted-foreground" },
		[".small-muted"] = { color = "muted-foreground", size = "small" },
		[".face"] = { radius = 12, border = true, border_color = "background" },
		[".field"] = { gap = 0, padding = { 7, 0 } },
		[".field-label"] = { color = "muted-foreground", gap = 6 },
		[".mark"] = { background = "accent", color = "#ffffff", radius = 6 },
		[".mark-soft"] = { background = "accent/15", color = "accent", radius = 4 },
		[".sla"] = { gap = 30, padding = { 10, 0, 0, 0 } },
		[".sla-clock"] = { gap = 3 },
		[".sla-left"] = { size = "large", weight = "medium" },
		[".sla-urgent"] = { color = "danger" },
		[".sla-track"] = { background = "secondary", radius = 2, margin = { top = 10 } },
		[".sla-fill"] = { background = "ink", radius = 2 },
		[".sla-fill-urgent"] = { background = "high" },
		[".check"] = {
			background = "tint",
			border = true,
			border_color = "tint-border",
			radius = 14,
			padding = { 18, 20, 20, 20 },
			gap = 0,
			margin = { top = 28 },
		},
		[".check-head"] = { gap = 12, margin = { bottom = 14 } },
		[".check-title"] = { weight = "medium" },
		[".step"] = { gap = 14, height = 35 },
		[".step-done"] = { background = "accent", color = "#ffffff", radius = 9 },
		[".step-wait"] = { color = "muted-foreground" },
		[".step-running"] = { color = "accent" },
		[".check-rule"] = { background = "tint-border", margin = { top = 12, bottom = 18, left = 40 } },
		[".suggest"] = { gap = 4, padding = { left = 40 } },
		[".suggest-label"] = { color = "accent", size = "small" },
		[".suggest-text"] = { size = "large", weight = "medium" },
		[".buttons"] = { gap = 10, margin = { top = 14 } },
		[".primary"] = { background = "accent", color = "#ffffff", radius = 9, padding = { 0, 18 }, height = 38, gap = 8 },
		[".primary:hover"] = { background = "accent/90" },
		[".primary-done"] = { background = "accent/15", color = "accent" },
		[".secondary"] = {
			background = "background",
			border = true,
			border_color = "border",
			radius = 9,
			padding = { 0, 16 },
			height = 38,
		},
		[".secondary:hover"] = { background = "secondary" },
		[".thread-head"] = { gap = 8, margin = { top = 30, bottom = 12 } },
		[".thread-more"] = { color = "muted-foreground", padding = { 2, 4 }, radius = 6 },
		[".thread-more:hover"] = { color = "foreground" },
		[".message"] = { padding = { 14, 16 }, gap = 12, radius = 12, margin = { bottom = 8 } },
		[".message-assistant"] = { background = "tint" },
		[".message-note"] = { background = "#fff6dc" },
		[".message-name"] = { weight = "medium" },
		[".message-assistant-name"] = { color = "accent" },
		[".message-text"] = { gap = 3 },
		[".composer-wrap"] = { padding = { 4, 30, 24, 30 } },
		[".composer"] = {
			border = true,
			border_color = "border",
			radius = 12,
			padding = { 8, 10, 10, 12 },
			gap = 6,
			background = "background",
		},
		[".composer-tab"] = { padding = { 4, 10 }, radius = 7, color = "muted-foreground" },
		[".composer-tab:hover"] = { color = "foreground" },
		[".composer-tab:selected"] = { background = "secondary", color = "foreground" },
		[".bare-input"] = { background = "#00000000", border = "none", shadow = "none", padding = { 6, 0 } },
		[".send"] = { background = "ink", color = "background", radius = 8 },
		[".send:hover"] = { background = "ink/85" },
		[".empty"] = { color = "muted-foreground" },
	},

	init = function(previous: State?): State
		local state: State = previous or { selected = nil, tab = "reply", draft = "", earlier = false, assigned = {} }
		state.assigned = state.assigned or {}
		return state
	end,

	update = function(state: State, msg)
		if type(msg) ~= "table" then
			if msg == "toggle-earlier" then
				state.earlier = not state.earlier
			elseif msg == "act" and state.selected then
				return state, { { to = "provider:desk", msg = { act = state.selected } } }
			elseif msg == "assign" and state.selected then
				state.assigned[state.selected] = not state.assigned[state.selected]
			elseif msg == "send" then
				msg = { msg = "submit", value = state.draft }
			else
				return state
			end
		end
		if msg.topic == "desk/selected" then
			local id = if type(msg.value) == "string" and msg.value ~= "" then msg.value else nil
			if id ~= state.selected then
				state.earlier = false
				state.draft = ""
			end
			state.selected = id
			if id then
				return state, { { to = "provider:desk", msg = { watch = id } } }
			end
		elseif msg.tab then
			state.tab = msg.tab
		elseif msg.msg == "typing" then
			state.draft = msg.value
		elseif msg.msg == "submit" and state.selected and string.match(msg.value or "", "%S") then
			state.draft = ""
			return state, {
				{ to = "provider:desk", msg = { reply = { id = state.selected, text = msg.value, internal = state.tab == "note" } } },
			}
		end
		return state
	end,

	view = function(state: State, app)
		local desk = app.desk
		local ticket = if desk then find(desk, state.selected) else nil
		if not desk or not ticket then
			return ui.column {
				grow = true,
				align = "center",
				justify = "center",
				ui.row { height = 66, drag = true },
				ui.text { if desk then "Pick a ticket to see it here." else "Loading…", class = "empty" },
			}
		end
		local assistant = desk.assistant
		local source = SOURCES[ticket.source] or SOURCES.web
		local requester = ticket.requester
		local who = first_name(requester.name)

		-- Header.
		local assigned = state.assigned[ticket.id]
		local head = ui.row {
			class = "head",
			align = "center",
			shrink = false,
			drag = true,
			ui.text { ticket.id, weight = "medium" },
			ui.row { class = "head-link", copy = `https://desk.corvo.co/t/{ticket.id}`, tooltip = "Copy link", ui.icon "link" },
			ui.spacer(),
			ui.row {
				class = "outline",
				align = "center",
				msg = "assign",
				ui.icon { name = if assigned then "check" else "user" },
				ui.text { if assigned then "Assigned to you" else "Assign" },
			},
			ui.row {
				class = "outline",
				width = 34,
				justify = "center",
				align = "center",
				tooltip = "More",
				style = { padding = 0 },
				ui.icon "ellipsis",
			},
		}

		-- Chips and watchers.
		local faces = {}
		for index, name in ticket.watchers do
			table.insert(faces, ui.row {
				class = "face",
				float = { top = 0, left = (index - 1) * 16 },
				avatar { name = name, shape = "circle", size = "normal" },
			})
		end
		local chips = ui.row {
			class = "chips",
			align = "center",
			ui.row {
				class = "chip",
				align = "center",
				ui.row { class = "chip-dot", width = 6, height = 6 },
				ui.text { ticket.status_label },
			},
			ui.row { class = "chip", align = "center", ui.text { ticket.kind } },
			ui.row {
				class = "chip",
				align = "center",
				ui.row { class = { "chip-bar", `bar-{string.lower(ticket.priority)}` }, width = 3, height = 13 },
				ui.text { ticket.priority },
			},
			ui.text { `Created {ticket.created} via`, class = "muted", style = { margin = { left = 8 } } },
			ui.icon { name = source.icon :: any, size = "small", style = { color = source.color } },
			ui.text { source.name },
			ui.spacer(),
			ui.icon { name = "eye", class = "muted", tooltip = `{#ticket.watchers} watching` },
			ui.row { width = 24 + (#faces - 1) * 16 + 2, height = 26, shrink = false, table.unpack(faces) },
		}

		local function field(label, value)
			return ui.row {
				class = "field",
				align = "start",
				ui.row { class = "field-label", width = LABEL, shrink = false, align = "center", label },
				ui.row { grow = true, align = "center", gap = "normal", value },
			}
		end

		-- SLA clocks.
		local clocks = {}
		for _, clock in ticket.sla do
			local met = clock.left == "Met"
			local used = math.clamp(clock.used or 0, 0, 1)
			table.insert(clocks, ui.column {
				class = "sla-clock",
				width = BAR,
				ui.row {
					align = "end",
					gap = "small",
					ui.text { clock.left, class = if clock.urgent then { "sla-left", "sla-urgent" } else "sla-left" },
					not met and ui.text { "left", class = if clock.urgent then "sla-urgent" else "muted" },
				},
				ui.text { if met then `{clock.label} met · due {clock.due}` else `{clock.label} due {clock.due}`, class = "small-muted" },
				ui.row {
					class = "sla-track",
					width = BAR,
					height = 3,
					ui.row { class = if clock.urgent then { "sla-fill", "sla-fill-urgent" } else "sla-fill", width = BAR * used, height = 3 },
				},
			})
		end

		local details = ui.column {
			gap = "none",
			field(ui.text "Requester", ui.row {
				align = "center",
				gap = "normal",
				avatar { name = requester.name, initial = initials(requester.name), shape = "circle", size = "normal" },
				ui.text { requester.name },
				ui.text { "·", class = "muted" },
				ui.text { requester.team, class = "muted" },
			}),
			field(ui.text "Device", ui.row {
				align = "center",
				gap = "normal",
				ui.row { width = 24, justify = "center", ui.icon "laptop" },
				ui.text { ticket.device.name },
				ticket.device.detail ~= "" and ui.text { "·", class = "muted" },
				ticket.device.detail ~= "" and ui.text { ticket.device.detail, class = "muted" },
			}),
			ui.row {
				class = "field",
				align = "start",
				ui.row {
					class = "field-label",
					width = LABEL,
					shrink = false,
					align = "center",
					ui.row {
						class = "mark-soft",
						width = 16,
						height = 16,
						align = "center",
						justify = "center",
						ui.icon { name = "sparkles", size = "small" },
					},
					ui.text { `Summary by {assistant}` },
				},
				ui.column { grow = true, ui.text { ticket.summary, wrap = true } },
			},
			ui.row {
				class = "field",
				align = "start",
				ui.row { class = "field-label", width = LABEL, shrink = false, style = { padding = { top = 10 } }, ui.text "SLA" },
				ui.row { class = "sla", table.unpack(clocks) },
			},
		}

		-- The assistant's check.
		local check = ticket.check
		local card = nil
		if check then
			local steps = {}
			for _, step in check.steps do
				local icon, note, tone
				if step.state == "done" then
					icon = ui.row {
						class = "step-done",
						width = 18,
						height = 18,
						align = "center",
						justify = "center",
						ui.icon { name = "check", size = "small" },
					}
					note, tone = step.note or "Done", "muted"
				elseif step.state == "running" then
					icon = ui.icon { name = "loader-circle", spin = true, class = "step-running" }
					note, tone = step.note or "Running now", "step-running"
				else
					icon = ui.icon { name = "clock", class = "step-wait" }
					note, tone = step.note or "Waiting", "muted"
				end
				table.insert(steps, ui.row {
					class = "step",
					align = "center",
					ui.row { width = 26, justify = "center", shrink = false, icon },
					ui.text { step.label },
					ui.spacer(),
					ui.text { note, class = tone },
				})
			end
			local primary
			if check.acting then
				primary = ui.row {
					class = { "primary", "primary-done" },
					align = "center",
					ui.icon { name = "loader-circle", spin = true },
					ui.text { "Running…", weight = "medium" },
				}
			elseif check.acted then
				primary = ui.row {
					class = { "primary", "primary-done" },
					align = "center",
					ui.icon "check",
					ui.text { "Done", weight = "medium" },
				}
			else
				primary = ui.row {
					class = "primary",
					align = "center",
					msg = "act",
					ui.text { check.primary, weight = "medium" },
				}
			end
			card = ui.column {
				class = "check",
				ui.row {
					class = "check-head",
					align = "center",
					mark(26),
					ui.text { check.title, class = "check-title" },
					ui.spacer(),
					ui.text { "via", class = "small-muted" },
					avatar { name = check.via, shape = "circle", size = "small" },
					ui.text { check.via },
				},
				ui.column { gap = "none", table.unpack(steps) },
				ui.row { class = "check-rule", height = 1 },
				ui.column {
					class = "suggest",
					ui.text { if check.acted then `{assistant} did` else `{assistant} suggests`, class = "suggest-label" },
					ui.text { check.suggestion, class = "suggest-text", wrap = true },
					ui.row {
						class = "buttons",
						align = "center",
						primary,
						ui.row {
							class = "secondary",
							align = "center",
							msg = { tab = "reply" },
							ui.text { check.secondary, weight = "medium" },
						},
					},
				},
			}
		end

		-- The thread.
		local function message(entry)
			local by_assistant = entry.from == "assistant"
			local name = if by_assistant then assistant else entry.name or ""
			return ui.row {
				class = if by_assistant
					then { "message", "message-assistant" }
					elseif entry.from == "note" then { "message", "message-note" }
					else "message",
				align = "start",
				if by_assistant then mark(28) else avatar { name = name, initial = initials(name), shape = "circle", size = "large" },
				ui.column {
					class = "message-text",
					grow = true,
					ui.row {
						align = "center",
						gap = "normal",
						ui.text { name, class = if by_assistant then { "message-name", "message-assistant-name" } else "message-name" },
						ui.text { if entry.from == "note" then `internal note · {entry.age}` else entry.age, class = "small-muted" },
					},
					ui.text { entry.text, wrap = true },
				},
			}
		end
		local messages = {}
		local earlier = ticket.earlier or {}
		if state.earlier then
			for _, entry in earlier do
				table.insert(messages, message(entry))
			end
		end
		for _, entry in ticket.thread do
			table.insert(messages, message(entry))
		end
		local more = #earlier
		local thread = ui.column {
			gap = "none",
			ui.row {
				class = "thread-head",
				align = "center",
				ui.icon { name = source.icon :: any, size = "small", style = { color = source.color } },
				ui.text { `{source.name} thread`, class = "muted" },
				ui.spacer(),
				more > 0 and ui.row {
					class = "thread-more",
					msg = "toggle-earlier",
					ui.text {
						if state.earlier
							then "Hide earlier messages"
							else `Show {more} earlier message{if more == 1 then "" else "s"}`,
					},
				},
			},
			#messages == 0 and ui.text { "No messages yet.", class = "muted" },
			table.unpack(messages),
		}

		-- The reply box.
		local note = state.tab == "note"
		local composer = ui.column {
			class = "composer-wrap",
			shrink = false,
			ui.column {
				class = "composer",
				ui.row {
					gap = "small",
					ui.row { class = "composer-tab", selected = not note, msg = { tab = "reply" }, ui.text { `Reply to {who}` } },
					ui.row { class = "composer-tab", selected = note, msg = { tab = "note" }, ui.text "Internal note" },
				},
				ui.row {
					align = "center",
					gap = "normal",
					ui.input {
						id = "reply",
						class = "bare-input",
						grow = true,
						value = state.draft,
						placeholder = if note then "Add a note only the IT team sees" else `Write a reply to {who}`,
						change = "typing",
						submit = "submit",
					},
					ui.row {
						class = "send",
						width = 32,
						height = 32,
						align = "center",
						justify = "center",
						msg = "send",
						tooltip = if note then "Add note" else "Send",
						ui.icon "arrow-up",
					},
				},
			},
		}

		return ui.column {
			grow = true,
			gap = "none",
			head,
			ui.column {
				grow = true,
				scroll = true,
				gap = "none",
				ui.column {
					class = "body",
					shrink = false,
					ui.text { ticket.title, class = "title", wrap = true },
					chips,
					details,
					card or false,
					thread,
				},
			},
			composer,
		}
	end,
}
