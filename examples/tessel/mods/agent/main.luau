-- An agent's own page: a bar with the way back and how it is shared, a
-- chat with the agent on the left (a scripted demo), and on the right its
-- card: the pixel icon, who manages it, Manage workflow, then Details
-- (instructions, guardrails, tone, knowledge and tools, each foldable, with
-- switches) or Activity (its runs).

local pixel_icon = use "pixel-icon"
local switch = use "switch"
local chat_message = use "chat-message"

type State = {
	detail: string,
	tab: string,
	folded: { [string]: any },
	toggles: { [string]: boolean },
	tone: { [string]: string },
	draft: string,
	mode: string,
	messages: { [string]: { any } },
}

local KNOWLEDGE = {
	{ id = "peoplebase", label = "Peoplebase", icon = "user", color = "#4f46e5", on = true },
	{ id = "drive", label = "Google Drive", icon = "cloud", color = "#1a73e8", on = false, connect = true },
	{ id = "slack", label = "Slack", icon = "hash", color = "#c2410c", on = true },
}

local TOOLS = {
	{ id = "code", label = "Code Interpreter", icon = "code", on = true },
	{ id = "images", label = "Image Generation", icon = "palette", on = false },
	{ id = "canvas", label = "Canvas", icon = "file-text", on = true },
}

local STATUS: { [string]: { label: string, icon: string, color: string } } = {
	active = { label = "Active", icon = "play", color = "run-active" },
	completed = { label = "Completed", icon = "circle-check", color = "run-completed" },
	review = { label = "Needs review", icon = "eye", color = "run-review" },
	pending = { label = "Pending", icon = "pause", color = "run-pending" },
	failed = { label = "Failed", icon = "circle-x", color = "run-failed" },
}

return mod {
	title = "Agent",

	subscribe = { "tessel/nav" },

	styles = {
		[".bar"] = { padding = { 0, 14, 0, 18 }, gap = 10, border = "bottom", border_color = "border" },
		[".bar-button"] = { radius = 7, padding = 5, color = "muted-foreground" },
		[".bar-button:hover"] = { background = "list-hover", color = "foreground" },
		[".bar-share"] = { gap = 6, weight = "medium" },
		[".chat"] = { padding = { 0, 60, 16, 60 }, gap = 0 },
		[".hello"] = { gap = 6, padding = { 0, 0, 70, 0 } },
		[".hello-icon"] = { color = "brand" },
		[".hello-text"] = { color = "muted-foreground" },
		[".ask"] = { background = "card", border = true, border_color = "#f2542d55", radius = 14, padding = { 10, 12 }, gap = 10, shadow = "0 0 0 3px #f2542d14" },
		[".ask-chip"] = { border = true, border_color = "border", radius = 8, padding = { 4, 9 }, gap = 6, size = "small" },
		[".ask-chip:hover"] = { background = "band" },
		[".ask-chip:selected"] = { background = "secondary", border_color = "#cfcfcd" },
		[".ask-mic"] = { background = "foreground", color = "background", radius = 9 },
		[".panel"] = { border = "left", border_color = "border", background = "background" },
		[".panel-top"] = { padding = { 38, 38, 0, 38 }, gap = 6 },
		[".panel-name"] = { size = "title", weight = "medium", padding = { 12, 0, 0, 0 } },
		[".panel-desc"] = { color = "muted-foreground" },
		[".panel-owner"] = { gap = 6, padding = { 8, 0, 14, 0 } },
		[".tag"] = { background = "secondary", radius = 5, padding = { 1, 6 }, size = "small", color = "muted-foreground" },
		[".more"] = { background = "secondary", radius = 8, padding = { 7, 9 } },
		[".more:hover"] = { background = "#ebebea" },
		[".panel-tabs"] = { padding = { 20, 38, 0, 38 }, border = "bottom", border_color = "border" },
		[".panel-tab"] = { padding = { 0, 0, 10, 0 }, border = "bottom", border_color = "#00000000", color = "muted-foreground", size = "large" },
		[".panel-tab:hover"] = { color = "foreground" },
		[".panel-tab:selected"] = { border_color = "foreground", color = "foreground" },
		[".block"] = { padding = { 16, 38 }, gap = 10, border = "bottom", border_color = "border" },
		[".block-head"] = { gap = 8, color = "muted-foreground" },
		[".block-title"] = { weight = "medium", color = "foreground" },
		[".block-text"] = { color = "foreground" },
		[".block-muted"] = { color = "muted-foreground" },
		[".toggle-row"] = { gap = 10 },
		[".connect"] = { color = "brand", weight = "medium", size = "small" },
		[".add-link"] = { gap = 6, size = "small", weight = "medium" },
		[".run-row"] = { padding = { 10, 38 }, gap = 10, border = "bottom", border_color = "border" },
		[".run-row:hover"] = { background = "band" },
		[".pill"] = { radius = 6, padding = { 1, 6 }, gap = 4, size = "small" },
	},

	init = function(previous: any): State
		local p = previous or {}
		return {
			detail = "",
			tab = "details",
			folded = p.folded or {},
			toggles = p.toggles or {},
			tone = p.tone or {},
			draft = "",
			mode = "",
			messages = {},
		}
	end,

	update = function(state: State, msg)
		if type(msg) ~= "table" then
			return state
		end
		if msg.topic == "tessel/nav" and type(msg.value) == "table" then
			if msg.value.detail ~= state.detail then
				state.tab = "details"
			end
			state.detail = msg.value.detail or ""
		elseif msg.back then
			return state, { { to = "topic:tessel/nav", msg = { detail = "" } } }
		elseif msg.select then
			state.tab = msg.select
		elseif msg.fold then
			state.folded[msg.fold] = if state.folded[msg.fold] then nil else true
		elseif msg.toggle then
			local key = state.detail .. ":" .. msg.toggle
			local current = state.toggles[key]
			if current == nil then
				current = msg.default == true
			end
			state.toggles[key] = not current
		elseif msg.mode then
			state.mode = if state.mode == msg.mode then "" else msg.mode
		elseif msg.manage then
			return state, { { to = "workflow", msg = { open = msg.manage } } }
		elseif msg.msg == "tone" then
			state.tone[state.detail] = msg.value
		elseif msg.msg == "draft" then
			state.draft = msg.value
		elseif msg.msg == "send" then
			local text = msg.value or state.draft
			if text ~= "" then
				local list = state.messages[state.detail] or {}
				table.insert(list, { from = "user", text = text })
				local reply = if state.mode == "research"
					then "I'll look through the connected sources first, then come back with what I found and where."
					elseif state.mode == "think"
					then "Let me think it through step by step before I touch the workflow."
					else "On it. I'll follow my instructions and post the result here when the run finishes."
				table.insert(list, { from = "assistant", text = reply })
				state.messages[state.detail] = list
				state.draft = ""
			end
		end
		return state
	end,

	view = function(state: State, app)
		local data = app.tessel
		local agent: any = nil
		if data and data.agents then
			for _, a in data.agents do
				if a.id == state.detail then
					agent = a
				end
			end
		end
		if not agent then
			return ui.column {
				grow = true,
				align = "center",
				justify = "center",
				ui.text { "This agent is not in the sample data.", tone = "muted" },
				ui.button { "Back to Agents", msg = { back = true } },
			}
		end
		local person = (data.people or {})[agent.owner] or { name = agent.owner, color = "#888888" }

		-- The agent's runs, its failed one first for Manage workflow.
		local runs: { any } = {}
		local manage: string? = nil
		local all_runs: { any } = data.runs or {}
		for _, run in all_runs do
			if run.agent == agent.id then
				table.insert(runs, run)
				if run.status == "failed" or manage == nil then
					manage = run.id
				end
			end
		end

		local function switched(id: string, default: boolean): boolean
			local value = state.toggles[agent.id .. ":" .. id]
			if value == nil then
				return default
			end
			return value
		end

		local function block(id: string, title: string, children: { any })
			local folded = state.folded[id] == true
			return ui.column {
				class = "block",
				ui.row {
					class = "block-head",
					align = "center",
					msg = { fold = id },
					ui.text { title, class = "block-title" },
					ui.spacer(),
					ui.icon { name = "info", size = "small", tooltip = `What {string.lower(title)} do` },
					ui.icon { name = if folded then "chevron-down" else "chevron-up", size = "small" },
				},
				not folded and ui.column { gap = "normal", table.unpack(children) },
			}
		end

		local knowledge: { any } = {}
		for _, item in KNOWLEDGE do
			local on = switched(item.id, item.on)
			table.insert(knowledge, ui.row {
				class = "toggle-row",
				height = 26,
				align = "center",
				ui.icon { name = item.icon :: any, size = "small", style = { color = item.color } },
				ui.text { item.label, tone = if on then "normal" else "muted" },
				item.connect and not on and ui.text { "·", tone = "muted" },
				item.connect and not on and ui.row { msg = { toggle = item.id, default = item.on }, ui.text { "Connect Account", class = "connect" } },
				ui.spacer(),
				switch { on = on, msg = { toggle = item.id, default = item.on } },
			})
		end
		table.insert(knowledge, ui.row { class = "add-link", align = "center", ui.icon { name = "plus", size = "small" }, ui.text "Add connection" })

		local tools: { any } = {}
		for _, item in TOOLS do
			local on = switched(item.id, item.on)
			table.insert(tools, ui.row {
				class = "toggle-row",
				height = 26,
				align = "center",
				ui.icon { name = item.icon :: any, size = "small", tone = "muted" },
				ui.text { item.label },
				ui.spacer(),
				switch { on = on, msg = { toggle = item.id, default = item.on } },
			})
		end

		local panel_body: { any } = {}
		if state.tab == "details" then
			panel_body = {
				block("instructions", "Instructions", {
					ui.text { "# " .. agent.name, class = "block-muted" },
					ui.text {
						`{agent.description}. Work through the workflow's steps in order, keep a note of every file you read, and ask before sending anything outside the team.`,
						class = "block-text",
						wrap = true,
					},
				}),
				block("guardrails", "Guardrails", {
					ui.text { `Only discuss topics related to {agent.tag} work. Never share candidate or salary details in public channels.`, wrap = true },
				}),
				block("tone", "Tone", {
					ui.input {
						id = "tone-" .. agent.id,
						value = state.tone[agent.id] or "",
						placeholder = "Type e.g. Casual, Detailed, Humorous, Objective…",
						change = "tone",
						class = "bare-input",
					},
				}),
				block("knowledge", "Knowledge", knowledge),
				block("tools", "Tools", tools),
			}
		else
			for _, run in runs do
				local look = STATUS[run.status] or STATUS.pending
				table.insert(panel_body, ui.row {
					class = "run-row",
					align = "center",
					msg = { manage = run.id },
					ui.column {
						grow = true,
						gap = "none",
						ui.text { run.summary, truncate = true },
						ui.text { `{#run.steps} steps · {run.age} ago`, size = "small", tone = "muted" },
					},
					ui.row {
						class = "pill",
						align = "center",
						hug = true,
						style = { color = look.color, background = look.color .. "/9" },
						ui.icon { name = look.icon :: any, size = "small" },
						ui.text { look.label },
					},
				})
			end
			if #runs == 0 then
				table.insert(panel_body, ui.row { class = "run-row", ui.text { "No runs yet.", tone = "muted" } })
			end
		end

		local messages: { any } = state.messages[agent.id] or {}
		local chat: any
		if #messages == 0 then
			chat = ui.column {
				grow = true,
				justify = "end",
				ui.column {
					class = "hello",
					max_width = 360,
					ui.icon { name = "message-square", size = "large", class = "hello-icon" },
					ui.text { "Let's start chatting with your agent", weight = "bold" },
					ui.text {
						"Describe your goal and the agent builds the instructions. It can handle records, routing, sequences and more.",
						class = "hello-text",
						wrap = true,
					},
				},
			}
		else
			local turns = {}
			for _, message in messages do
				table.insert(turns, chat_message {
					from = message.from,
					text = message.text,
					author = if message.from == "assistant" then agent.name else nil,
				})
			end
			chat = ui.column { grow = true, scroll = true, gap = "large", padding = { 24, 0 } :: any, table.unpack(turns) }
		end

		local function chip(id: string, icon: string, label: string)
			return ui.row {
				class = "ask-chip",
				align = "center",
				selected = state.mode == id,
				msg = { mode = id },
				ui.icon { name = icon :: any, size = "small" },
				ui.text { label },
			}
		end

		return ui.column {
			grow = true,
			gap = "none",
			ui.row {
				class = "bar",
				drag = true,
				height = 54,
				shrink = false,
				align = "center",
				ui.row { class = "bar-button", msg = { back = true }, tooltip = "Back to Agents", ui.icon "arrow-left" },
				pixel_icon { pattern = agent.icon.pattern, colors = agent.icon.colors, size = 20 },
				ui.text { agent.name, weight = "medium" },
				ui.spacer(),
				ui.row { class = "bar-share", align = "center", ui.icon { name = "lock", size = "small" }, ui.text "Public to everyone" },
				ui.row { class = "soft-button", ui.text { "Share", weight = "medium" } },
				ui.row { class = "bar-button", tooltip = "More", ui.icon "ellipsis" },
			},
			ui.row {
				grow = true,
				gap = "none",
				ui.column {
					class = "chat",
					grow = true,
					chat,
					ui.column {
						class = "ask",
						shrink = false,
						ui.input {
							id = "ask",
							value = state.draft,
							placeholder = `Ask {agent.name}…`,
							change = "draft",
							submit = "send",
							multiline = true,
							rows = 2,
							class = "bare-input",
						},
						ui.row {
							align = "center",
							gap = "small",
							ui.row { class = "ask-chip", tooltip = "Attach", ui.icon { name = "plus", size = "small" } },
							chip("research", "search", "Research"),
							chip("think", "sparkles", "Think"),
							chip("tools", "settings-2", "Tools"),
							ui.spacer(),
							ui.row {
								class = "ask-mic",
								width = 34,
								height = 34,
								align = "center",
								justify = "center",
								msg = { msg = "send" },
								tooltip = "Send (⌘↩)",
								ui.icon { name = if state.draft ~= "" then "arrow-up" else "mic" },
							},
						},
					},
				},
				ui.column {
					class = "panel",
					width = 415,
					shrink = false,
					scroll = true,
					gap = "none",
					ui.column {
						class = "panel-top",
						shrink = false,
						pixel_icon { pattern = agent.icon.pattern, colors = agent.icon.colors, size = 64 },
						ui.text { agent.name, class = "panel-name" },
						ui.text { agent.description, class = "panel-desc" },
						ui.row {
							class = "panel-owner",
							align = "center",
							ui.row {
								width = 18,
								height = 18,
								align = "center",
								justify = "center",
								style = { background = person.color, radius = 999, color = "#ffffff" },
								ui.text { string.upper(string.sub(person.name, 1, 1)), size = "small", weight = "bold" },
							},
							ui.text { if agent.owner == "me" then "Managed by you" else `Managed by {person.name}`, weight = "medium" },
							ui.row { class = "tag", ui.text { "# " .. agent.tag } },
						},
						ui.row {
							gap = "normal",
							ui.row {
								class = "dark-button",
								align = "center",
								msg = if manage then { manage = manage } else nil,
								tooltip = if manage then nil else "Nothing has run yet",
								ui.icon { name = "workflow", size = "small" },
								ui.text { "Manage workflow", weight = "medium" },
							},
							ui.row { class = "more", ui.icon { name = "ellipsis", size = "small" } },
						},
					},
					ui.column {
						class = "panel-tabs",
						shrink = false,
						ui.row {
							gap = "large",
							ui.row { class = "panel-tab", selected = state.tab == "details", msg = { select = "details" }, ui.text "Details" },
							ui.row {
								class = "panel-tab",
								selected = state.tab == "activity",
								msg = { select = "activity" },
								gap = "small",
								ui.text "Activity",
								#runs > 0 and ui.text { tostring(#runs), size = "small", tone = "muted" },
							},
						},
					},
					ui.column { gap = "none", shrink = false, table.unpack(panel_body) },
				},
			},
		}
	end,
}
