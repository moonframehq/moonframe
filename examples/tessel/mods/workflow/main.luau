-- The workflow modal: a run's title and state over a chat on the left and
-- its workflow on the right, a column of step cards from Start, the failed
-- one outlined in red. The chat folds away, the modal fills the window, and
-- + and − zoom the cards. "Connect to Drive" (a failed step's fix) and
-- Re-run are shared with the hub on the `tessel/runs` topic.
--
-- Opened by `{ open = "<run id>" }`. The chat is a scripted demo, not a model.

local pixel_icon = use "pixel-icon"
local chat_message = use "chat-message"

type State = {
	run: string,
	chat: boolean,
	full: boolean,
	zoom: number,
	draft: string,
	messages: { any },
	runs: { [string]: string },
}

local ZOOMS = { 0.8, 0.9, 1, 1.1, 1.2 }

local STATUS: { [string]: { label: string, icon: string, color: string } } = {
	active = { label = "Active", icon = "play", color = "run-active" },
	completed = { label = "Completed", icon = "circle-check", color = "run-completed" },
	review = { label = "Needs review", icon = "eye", color = "run-review" },
	pending = { label = "Pending", icon = "pause", color = "run-pending" },
	failed = { label = "Failed", icon = "circle-x", color = "run-failed" },
}

local function find(list: { any }?, id: string): any?
	local items: { any } = list or {}
	for _, item in items do
		if item.id == id then
			return item
		end
	end
	return nil
end

return mod {
	title = "Workflow",

	subscribe = { "tessel/runs" },

	styles = {
		[".wf"] = { gap = 0, padding = { 0, 0, 16, 0 } },
		[".wf-head"] = { padding = { 0, 14, 0, 16 }, gap = 10 },
		[".wf-title"] = { weight = "medium" },
		[".wf-pill"] = { radius = 6, padding = { 1, 7 }, gap = 4, size = "small" },
		[".wf-rerun"] = { background = "secondary", radius = 7, padding = { 4, 10 }, gap = 6, size = "small", color = "muted-foreground" },
		[".wf-rerun:hover"] = { background = "#ebebea", color = "foreground" },
		[".wf-close"] = { radius = 7, padding = 5, color = "muted-foreground" },
		[".wf-close:hover"] = { background = "list-hover", color = "foreground" },
		[".wf-body"] = { padding = { 0, 16 }, gap = 16 },
		[".wf-chat"] = { gap = 10, padding = { 0, 8, 0, 8 } },
		[".wf-hint-icon"] = { color = "brand" },
		[".wf-hint-text"] = { color = "muted-foreground" },
		[".wf-composer"] = { background = "card", border = true, border_color = "border", radius = 12, padding = { 8, 10, 8, 10 }, gap = 6, shadow = "0 1px 2px #00000008" },
		[".wf-round"] = { radius = 7, padding = 4, color = "muted-foreground", border = true, border_color = "border" },
		[".wf-round:hover"] = { background = "list-hover", color = "foreground" },
		[".wf-canvas"] = { background = "band", border = true, border_color = "border", radius = 12, gap = 0 },
		[".wf-canvas-head"] = { padding = { 14, 16 }, gap = 10 },
		[".wf-tool"] = { background = "card", border = true, border_color = "border", radius = 8, padding = 6, color = "muted-foreground", shadow = "0 1px 2px #0000000a" },
		[".wf-tool:hover"] = { color = "foreground" },
		[".wf-toggle"] = { radius = 6, padding = 3, color = "muted-foreground" },
		[".wf-toggle:hover"] = { background = "list-hover", color = "foreground" },
		[".wf-start"] = { background = "secondary", border = true, border_color = "border", radius = 6, padding = { 2, 9 }, gap = 5, size = "small", font = "mono", color = "muted-foreground" },
		[".wf-card"] = { background = "card", border = true, border_color = "border", radius = 10, gap = 0, shadow = "0 1px 3px #0000000d" },
		[".wf-card-failed"] = { border_color = "run-failed/70", background = "#fffafa" },
		[".wf-card-head"] = { padding = { 10, 12 }, gap = 9 },
		[".wf-card-title"] = { weight = "medium" },
		[".wf-card-title-failed"] = { weight = "medium", color = "#b4232b" },
		[".wf-card-rule"] = { background = "border", margin = { 0, 10 } },
		[".wf-card-foot"] = { padding = { 8, 12, 10, 12 }, gap = 8 },
		[".wf-note"] = { size = "small", color = "muted-foreground", gap = 7 },
		[".wf-chip"] = { background = "card", border = true, border_color = "border", radius = 6, padding = { 2, 8 }, size = "small", gap = 5 },
		[".wf-chip:hover"] = { background = "band" },
		[".wf-action"] = { background = "secondary", radius = 6, padding = { 3, 8 }, size = "small", weight = "medium", gap = 6 },
		[".wf-action:hover"] = { background = "#e6e6e4" },
		[".wf-dot"] = { background = "run-failed", radius = 999 },
		[".wf-line"] = { background = "#d9d9d7" },
		[".wf-plus"] = { background = "card", border = true, border_color = "border", radius = 6, color = "muted-foreground" },
		[".wf-plus:hover"] = { color = "foreground" },
		[".wf-muted"] = { color = "muted-foreground" },
	},

	init = function(previous: any): State
		return {
			run = "",
			chat = true,
			full = false,
			zoom = 3,
			draft = "",
			messages = {},
			runs = {},
		}
	end,

	update = function(state: State, msg)
		if type(msg) == "string" then
			if msg == "close" then
				state.run = ""
				state.full = false
			elseif msg == "toggle-chat" then
				state.chat = not state.chat
			elseif msg == "toggle-full" then
				state.full = not state.full
			elseif msg == "zoom-in" then
				state.zoom = math.min(#ZOOMS, state.zoom + 1)
			elseif msg == "zoom-out" then
				state.zoom = math.max(1, state.zoom - 1)
			elseif msg == "rerun" and state.run ~= "" then
				state.runs[state.run] = "rerun"
				return state, { { to = "topic:tessel/runs", msg = state.runs } }
			elseif msg == "fix" and state.run ~= "" then
				state.runs[state.run] = "resolved"
				table.insert(state.messages, {
					from = "assistant",
					text = "Connected. I picked up the candidate sheet and the run carries on from the failed step.",
				})
				return state, { { to = "topic:tessel/runs", msg = state.runs } }
			end
			return state
		end
		if type(msg) ~= "table" then
			return state
		end
		if msg.topic == "tessel/runs" then
			state.runs = if type(msg.value) == "table" then msg.value else {}
		elseif msg.open then
			state.run = msg.open
			state.messages = {}
		elseif msg.msg == "draft" then
			state.draft = msg.value
		elseif msg.msg == "send" then
			local text = msg.value or state.draft
			if text ~= "" then
				table.insert(state.messages, { from = "user", text = text })
				local lower = string.lower(text)
				local reply
				if string.find(lower, "drive", 1, true) or string.find(lower, "connect", 1, true) then
					reply = "Use Connect to Drive on the red step and I'll read the candidate sheet from there."
				elseif string.find(lower, "why", 1, true) or string.find(lower, "fail", 1, true) then
					reply = "The candidate details step had nothing to read: no sheet was attached and no drive is connected. Give me either and I can retry."
				else
					reply = "Noted. I'll add that to the workflow's instructions; re-run when you're ready."
				end
				table.insert(state.messages, { from = "assistant", text = reply })
				state.draft = ""
			end
		end
		return state
	end,

	view = function(state: State, app)
		local data = app.tessel
		local run = if state.run ~= "" then find(data and data.runs, state.run) else nil
		local agent = run and find(data and data.agents, run.agent)
		local fix = run and state.runs[run.id]

		local content: any = ui.column {}
		if run then
			local status = if fix and run.status == "failed" then "active" else run.status
			local look = STATUS[status] or STATUS.pending
			local zoom = ZOOMS[state.zoom]
			local width = math.floor(352 * zoom)

			local function status_icon(s: string)
				if s == "done" then
					return ui.icon { name = "circle-check", size = "small", style = { color = "run-completed" } }
				elseif s == "failed" then
					return ui.icon { name = "circle-x", size = "small", style = { color = "run-failed" } }
				elseif s == "running" then
					return ui.icon { name = "loader-circle", size = "small", spin = true, tone = "muted" }
				elseif s == "review" then
					return ui.icon { name = "eye", size = "small", style = { color = "run-review" } }
				end
				return ui.icon { name = "circle-dashed", size = "small", tone = "muted" }
			end

			local function line(height: number)
				return ui.row { class = "wf-line", width = 1, height = math.floor(height * zoom), shrink = false }
			end

			-- The cards from Start down, with a fix or re-run applied.
			local flow: { any } = {
				ui.row { class = "wf-start", align = "center", hug = true, ui.icon { name = "key-round", size = "small" }, ui.text "Start" },
			}
			local after_fix = false
			for index, step in run.steps do
				local s = step.status
				if fix == "rerun" and s == "failed" then
					s = "running"
				elseif fix == "resolved" and s == "failed" then
					s = "done"
					after_fix = true
				elseif after_fix and s == "waiting" then
					s = "running"
					after_fix = false
				end
				local failed = s == "failed"
				local foot: { any } = {}
				if step.note and not (fix == "resolved" and step.status == "failed") then
					table.insert(foot, ui.row {
						class = "wf-note",
						align = "center",
						ui.icon { name = (step.note_icon or "info") :: any, size = "small" },
						ui.text { step.note, truncate = true },
					})
				elseif fix == "resolved" and step.status == "failed" then
					table.insert(foot, ui.row {
						class = "wf-note",
						align = "center",
						ui.icon { name = "cloud", size = "small" },
						ui.text "Drive connected · 7 candidates read.",
					})
				end
				if step.action and failed then
					table.insert(foot, ui.row {
						class = "wf-action",
						align = "center",
						hug = true,
						msg = "fix",
						ui.icon { name = "cloud", size = "small", style = { color = "#1a73e8" } },
						ui.text { step.action },
					})
				end
				if step.chips then
					local pills: { any } = { ui.icon { name = (step.chips_icon or "hash") :: any, size = "small", style = { color = "#c2410c" } } }
					for _, chip in step.chips do
						table.insert(pills, ui.row { class = "wf-chip", ui.text { chip } })
					end
					table.insert(foot, ui.row { align = "center", gap = "normal", table.unpack(pills) })
				end
				table.insert(flow, line(14))
				if index > 1 then
					table.remove(flow)
					local before = run.steps[index - 1]
					if before.status == "failed" and not fix then
						table.insert(flow, line(14))
						table.insert(flow, ui.row {
							class = "wf-plus",
							width = 20,
							height = 20,
							align = "center",
							justify = "center",
							tooltip = "Add a step",
							ui.icon { name = "plus", size = "small" },
						})
						table.insert(flow, line(14))
					else
						table.insert(flow, line(54))
					end
				end
				table.insert(flow, ui.column {
					class = if failed then "wf-card wf-card-failed" else "wf-card",
					width = width,
					shrink = false,
					ui.row {
						class = "wf-card-head",
						align = "center",
						status_icon(s),
						ui.text { step.title, class = if failed then "wf-card-title-failed" else "wf-card-title", truncate = true },
						ui.spacer(),
						ui.icon { name = "ellipsis", size = "small", tone = "muted" },
					},
					#foot > 0 and ui.row { class = "wf-card-rule", height = 1 },
					#foot > 0 and ui.column { class = "wf-card-foot", table.unpack(foot) },
					failed and ui.row { class = "wf-dot", width = 9, height = 9, float = { top = -5, right = -5 } },
				})
			end

			local chat: { any } = {}
			if #state.messages == 0 then
				chat = {
					ui.spacer(),
					ui.icon { name = "message-square", size = "large", class = "wf-hint-icon" },
					ui.text { "Not sure what to do?", weight = "bold" },
					ui.text {
						"Describe your goal and the agent will build the instructions. It can handle the workflow from your own words.",
						class = "wf-hint-text",
						wrap = true,
					},
					ui.spacer(),
				}
			else
				local turns = {}
				for _, message in state.messages do
					table.insert(turns, chat_message {
						from = message.from,
						text = message.text,
						author = if message.from == "assistant" then agent and agent.name or "Agent" else nil,
					})
				end
				chat = { ui.column { grow = true, scroll = true, gap = "large", padding = "small", table.unpack(turns) } }
			end

			content = ui.column {
				class = "wf",
				grow = true,
				ui.row {
					class = "wf-head",
					height = 52,
					shrink = false,
					align = "center",
					agent and pixel_icon { pattern = agent.icon.pattern, colors = agent.icon.colors, size = 18 },
					ui.text { agent and agent.name or run.agent, class = "wf-title" },
					ui.row {
						class = "wf-pill",
						align = "center",
						hug = true,
						style = { color = look.color, background = look.color .. "/10" },
						ui.icon { name = look.icon :: any, size = "small" },
						ui.text { look.label },
					},
					ui.spacer(),
					ui.row {
						class = "wf-rerun",
						align = "center",
						msg = "rerun",
						tooltip = "Run it again from the failed step",
						ui.icon { name = "refresh-cw", size = "small" },
						ui.text "Re-run",
					},
					ui.row { class = "wf-close", msg = "close", tooltip = "Close", ui.icon "close" },
				},
				ui.row {
					class = "wf-body",
					grow = true,
					state.chat and ui.column {
						width = 340,
						shrink = false,
						gap = "none",
						ui.column { class = "wf-chat", grow = true, table.unpack(chat) },
						ui.column {
							class = "wf-composer",
							shrink = false,
							ui.input {
								id = "wf-draft",
								value = state.draft,
								placeholder = "Type here…",
								change = "draft",
								submit = "send",
								multiline = true,
								rows = 3,
								class = "bare-input",
							},
							ui.row {
								align = "center",
								ui.row { class = "wf-round", tooltip = "Attach a file", ui.icon { name = "plus", size = "small" } },
								ui.spacer(),
								ui.row { class = "wf-round", msg = { msg = "send" }, tooltip = "Send (⌘↩)", ui.icon { name = "arrow-up", size = "small" } },
							},
						},
					},
					ui.column {
						class = "wf-canvas",
						grow = true,
						ui.row {
							class = "wf-canvas-head",
							align = "center",
							ui.row {
								class = "wf-toggle",
								msg = "toggle-chat",
								tooltip = if state.chat then "Hide the chat" else "Show the chat",
								ui.icon { name = if state.chat then "panel-left-close" else "panel-left-open" },
							},
							ui.text { "Workflow", weight = "medium" },
						},
						ui.column {
							grow = true,
							scroll = true,
							align = "center",
							gap = "none",
							ui.column { align = "center", gap = "none", shrink = false, padding = "large", table.unpack(flow) },
						},
						ui.column {
							float = { top = 12, right = 12 },
							gap = "small",
							ui.row { class = "wf-tool", msg = "toggle-full", tooltip = if state.full then "Shrink" else "Fill the window", ui.icon { name = if state.full then "minimize-2" else "maximize-2", size = "small" } },
							ui.row { class = "wf-tool", msg = "zoom-in", tooltip = "Zoom in", ui.icon { name = "plus", size = "small" } },
							ui.row { class = "wf-tool", msg = "zoom-out", tooltip = "Zoom out", ui.icon { name = "minus", size = "small" } },
						},
					},
				},
			}
		end

		return ui.column {
			gap = "none",
			height = 0,
			ui.overlay {
				open = run ~= nil,
				dismiss = "close",
				backdrop = true,
				place = "center",
				width = if state.full then 1420 else 1210,
				height = if state.full then 990 else 772,
				content = content,
			},
		}
	end,
}
