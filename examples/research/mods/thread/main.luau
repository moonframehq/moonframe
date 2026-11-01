-- The main thread: the ask, the agent's reply and its plan as a card that
-- advances step by step, then a box to add a note to the plan. The agent is
-- a SCRIPTED DEMO played by `providers/research` (app.research.run), not a
-- model; the stop button pauses it, and replays it once it is done.

local chat_message = use "chat-message"
local icon_button = use "icon-button"

type State = { notes: { string }, draft: string }

local CELLS = { "APY", "fee", "minimum", "rating" }

-- One app's agent and how many of its apps are written so far.
local function tally(apps: { any }, agent: string, done: (number) -> boolean): (number, number)
	local total, count = 0, 0
	for index, app in apps do
		if app.agent == agent then
			total += 1
			if done(index) then
				count += 1
			end
		end
	end
	return count, total
end

return mod {
	region = "thread",
	title = "Thread",

	styles = {
		[".thread-head"] = { padding = { 0, 12, 0, 16 }, gap = 8 },
		[".thread-body"] = { padding = { 10, 16, 16, 16 }, gap = 16 },
		[".thread-bot"] = { background = "accent", color = "#ffffff", radius = 6 },
		[".plan"] = { background = "background", border = true, border_color = "border", radius = 12, shadow = "0 1px 3px #0000000a" },
		[".plan-head"] = { padding = { 14, 14, 14, 16 }, gap = 10, border = "bottom", border_color = "border" },
		[".plan-bot"] = { background = "accent", color = "#ffffff", radius = 8 },
		[".plan-title"] = { size = "large", weight = "medium" },
		[".plan-clock"] = { border = true, border_color = "border", radius = 13, padding = { 3, 9 }, gap = 5, color = "muted-foreground" },
		[".plan-steps"] = { padding = { 14, 16, 4, 16 }, gap = 0 },
		[".plan-step-body"] = { padding = { 0, 0, 16, 0 }, gap = 10 },
		[".plan-done"] = { background = "success", color = "#ffffff", radius = 9 },
		[".plan-active"] = { color = "accent" },
		[".plan-pending"] = { color = "muted-foreground/50" },
		[".plan-line"] = { background = "success" },
		[".plan-step-pending"] = { color = "muted-foreground" },
		[".plan-count"] = { size = "small", color = "muted-foreground" },
		[".plan-count-active"] = { size = "small", color = "accent" },
		[".plan-box"] = { background = "secondary", radius = 10, padding = { 10, 14 }, gap = 0 },
		[".plan-agent"] = { gap = 10 },
		[".plan-agent-idle"] = { color = "muted-foreground" },
		[".plan-track"] = { background = "border", radius = 2 },
		[".plan-fill"] = { background = "accent", radius = 2 },
		[".plan-reading"] = { padding = { 10, 0, 0, 0 }, gap = 10, border = "top", border_color = "border" },
		[".plan-foot"] = { background = "secondary", padding = { 10, 16 }, gap = 6, radius = { 0, 0, 12, 12 }, border = "top", border_color = "border" },
		[".composer-box"] = { border = true, border_color = "border", radius = 14, padding = { 12, 12, 10, 14 }, gap = 10, shadow = "0 1px 3px #0000000a" },
		[".composer-tools"] = { color = "muted-foreground", gap = 2 },
		[".composer-model"] = { border = true, border_color = "border", radius = 15, padding = { 0, 10 }, gap = 5, color = "muted-foreground" },
		[".composer-stop"] = { background = "foreground", color = "background", radius = 16 },
	},

	init = function(previous: State?): State
		return { notes = previous and previous.notes or {}, draft = "" }
	end,

	update = function(state: State, msg)
		if msg == "toggle" or msg == "restart" then
			return state, { { to = "provider:research", msg = msg } }
		elseif type(msg) == "table" and msg.msg == "typing" then
			state.draft = msg.value
		elseif type(msg) == "table" and msg.msg == "note" and string.match(msg.value or "", "%S") then
			table.insert(state.notes, msg.value)
			state.draft = ""
		end
	end,

	view = function(state: State, app)
		local data = app.research
		local run = data and data.run

		local function step_mark(kind: string, last: boolean)
			local mark
			if kind == "done" then
				mark = ui.row { class = "plan-done", width = 18, height = 18, align = "center", justify = "center", ui.icon { name = "check", size = "small" } }
			elseif kind == "active" then
				mark = ui.icon { name = "circle-dot", class = "plan-active" }
			else
				mark = ui.icon { name = "circle", class = "plan-pending" }
			end
			return ui.column {
				width = 20,
				align = "center",
				gap = "none",
				ui.row { height = 22, align = "center", mark },
				kind == "done" and not last and ui.row { class = "plan-line", width = 2, grow = true },
			}
		end

		local function agents_box()
			local apps = data.apps
			local row = run.row
			local current = apps[row]
			local lines = {}
			for _, agent in data.agents do
				local done, total = tally(apps, agent.id, function(index)
					return index < row
				end)
				local working = current ~= nil and current.agent == agent.id
				local idle = done == 0 and not working
				local share = if total > 0 then done / total else 0
				table.insert(lines, ui.row {
					class = if idle then { "plan-agent", "plan-agent-idle" } else "plan-agent",
					height = 30,
					align = "center",
					ui.icon { name = "bot", size = "small", style = { color = agent.color } },
					ui.row { width = 84, ui.text { agent.label, truncate = true } },
					ui.row { class = "plan-track", width = 140, height = 4, ui.row { class = "plan-fill", width = 140 * share, height = 4 } },
					ui.spacer(),
					ui.row { width = 32, justify = "end", ui.text { if idle then "Next" else `{done}/{total}`, class = "plan-count" } },
				})
			end
			if current and run.cell > 0 then
				local agent = data.agents[1]
				for _, a in data.agents do
					if a.id == current.agent then
						agent = a
					end
				end
				table.insert(lines, ui.row {
					class = "plan-reading",
					align = "center",
					style = { margin = { 6, 0, 0, 0 } },
					ui.icon { name = "bot", size = "small", style = { color = agent.color } },
					ui.text { `{agent.label} is reading {current.source}`, truncate = true },
					ui.spacer(),
					ui.text { CELLS[run.cell], class = "plan-count" },
				})
			end
			return ui.column { class = "plan-box", table.unpack(lines) }
		end

		local function line_box(text: string, detail: string)
			return ui.row {
				class = "plan-box",
				height = 38,
				align = "center",
				gap = "normal",
				ui.icon { name = "loader-circle", size = "small", spin = run.playing, class = "plan-active" },
				ui.text { text, truncate = true },
				ui.spacer(),
				ui.text { detail, class = "plan-count" },
			}
		end

		local function plan()
			local n = #data.apps
			local s = run.step
			local rows_done = if s > 2 then n elseif s == 2 then math.max(0, run.row - 1) else 0
			local right = {
				{ active = `{run.found} found`, done = `{n} found` },
				{ active = `{rows_done} of {n}`, done = `{n} of {n}` },
				{ active = `{run.reviewed} of {n}`, done = `{n} reviewed` },
				{ active = "Writing", done = "Ready" },
			}
			local steps = {}
			for index, title in data.steps do
				local kind = if index < s then "done" elseif index == s then "active" else "pending"
				local extra = nil
				if kind == "active" then
					if s == 1 then
						local last = data.apps[run.found]
						extra = line_box("Searching app stores and bank lists", if last then last.source else "")
					elseif s == 2 and run.row > 0 then
						extra = agents_box()
					elseif s == 3 then
						local app_ = data.apps[math.min(n, run.reviewed + 1)]
						extra = line_box(`Reading reviews of {app_.name}`, "App Store · Play")
					elseif s == 4 then
						extra = line_box("Comparing APY, fees and ratings", "draft")
					end
				end
				table.insert(steps, ui.row {
					gap = "normal",
					step_mark(kind, index == #data.steps),
					ui.column {
						class = "plan-step-body",
						grow = true,
						ui.row {
							height = 22,
							align = "center",
							ui.text { title, class = if kind == "pending" then "plan-step-pending" else nil, truncate = true },
							ui.spacer(),
							kind ~= "pending" and ui.text {
								if kind == "done" then right[index].done else right[index].active,
								class = if kind == "active" then "plan-count-active" else "plan-count",
							},
						},
						extra,
					},
				})
			end

			local paused = not run.playing and not run.done
			return ui.column {
				class = "plan",
				gap = "none",
				shrink = false,
				ui.row {
					class = "plan-head",
					align = "center",
					ui.row { class = "plan-bot", width = 28, height = 28, align = "center", justify = "center", ui.icon { name = "bot" } },
					ui.column {
						gap = "none",
						grow = true,
						ui.text { if run.done then `Researched {n} savings apps` else `Researching {n} savings apps`, class = "plan-title", truncate = true },
						ui.text {
							if run.done then "4 of 4 steps · done"
								elseif paused then `Paused · step {s} of 4`
								else `Step {s} of 4 · {run.left}`,
							class = "small-muted",
						},
					},
					ui.row {
						class = "plan-clock",
						align = "center",
						ui.icon { name = if paused then "pause" else "clock", size = "small" },
						ui.row { width = 40, ui.text { run.elapsed, size = "small" } },
					},
				},
				ui.column { class = "plan-steps", table.unpack(steps) },
				ui.row {
					class = "plan-foot",
					align = "center",
					ui.icon { name = "activity", size = "small", class = "quiet" },
					ui.text { `{run.used} of ~{run.estimate} credits`, class = "small-muted" },
					ui.spacer(),
					ui.text { `Plan approved {data.approved}`, class = "small-muted" },
				},
			}
		end

		local body = {}
		if data and run then
			table.insert(body, chat_message { from = "user", text = data.prompt })
			table.insert(body, ui.column {
				gap = "normal",
				ui.row {
					align = "center",
					gap = "normal",
					ui.row { class = "thread-bot", width = 20, height = 20, align = "center", justify = "center", ui.icon { name = "bot", size = "small" } },
					ui.text { "Agent · demo", weight = "medium" },
					ui.text { "Thought for 8s", class = "small-muted" },
					ui.icon { name = "chevron-right", size = "small", class = "quiet" },
				},
				ui.text { data.intro, wrap = true },
			})
			table.insert(body, plan())
			if run.done then
				table.insert(body, ui.text {
					"Done. Kestrel leads on APY at 4.60% with no fee and a $1 minimum; Greenmark is the only one charging $5 a month. Sorrel's gap is the minimum deposit: four of the fourteen ask for none. Every number links to its source in the sheet.",
					wrap = true,
				})
			end
			for _, note in state.notes do
				table.insert(body, chat_message { from = "user", text = note })
				table.insert(body, ui.text { "Noted. I'll fold that into the summary (a scripted reply: this agent is a demo).", class = "small-muted", wrap = true })
			end
		else
			table.insert(body, ui.text { "Loading the research…", class = "small-muted" })
		end

		local playing = run ~= nil and run.playing and not run.done
		return ui.column {
			grow = true,
			gap = "none",
			ui.row {
				class = "thread-head",
				height = 46,
				shrink = false,
				align = "center",
				ui.icon { name = "message-square", size = "small", class = "quiet" },
				ui.text { "Main thread", weight = "medium" },
				ui.icon { name = "chevron-down", size = "small", class = "quiet" },
				ui.spacer(),
				icon_button { icon = "history", tooltip = "Run the demo again", msg = "restart", size = "small" },
				icon_button { icon = "maximize-2", tooltip = "Hide the sheet (not in this example)", msg = "toggle-thread", to = "shell", size = "small" },
			},
			ui.column {
				grow = true,
				scroll = true,
				gap = "none",
				ui.column { class = "thread-body", shrink = false, table.unpack(body) },
			},
			ui.column {
				class = "composer-box",
				shrink = false,
				style = { margin = { 0, 12, 12, 12 } },
				ui.input {
					id = "note",
					value = state.draft,
					placeholder = "Add a note to the plan…",
					change = "typing",
					submit = "note",
					class = "bare-input",
				},
				ui.row {
					align = "center",
					ui.row {
						class = "composer-tools",
						align = "center",
						icon_button { icon = "plus", tooltip = "Attach", msg = "none", size = "small" },
						icon_button { icon = "at-sign", tooltip = "Mention", msg = "none", size = "small" },
						icon_button { icon = "mic", tooltip = "Dictate", msg = "none", size = "small" },
					},
					ui.spacer(),
					ui.row { align = "center", gap = "small", ui.text { "Agent" }, ui.icon { name = "chevron-down", size = "small", class = "quiet" } },
					ui.row { class = "composer-model", height = 30, align = "center", ui.icon { name = "zap", size = "small" }, ui.text { "Max" } },
					ui.row {
						class = "composer-stop",
						width = 32,
						height = 32,
						align = "center",
						justify = "center",
						msg = "toggle",
						tooltip = if playing then "Pause the agent" elseif run and run.done then "Run the demo again" else "Resume the agent",
						ui.icon {
							name = if playing then "square" elseif run and run.done then "rotate-cw" else "play",
							size = "small",
						},
					},
				},
			},
		}
	end,
}
