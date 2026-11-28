-- A whole conversation with a pretend agent. The chat's commands come to
-- this mod (`to = "parent"`) instead of the `chat` provider, and the mod
-- plays the agent: it answers a permission question, queues, unqueues and
-- steers prompts, runs a sub-agent that can be stopped, asks a question
-- with choices, hands the conversation to another agent, switches mode,
-- model and effort, and "Next step" moves the running turn on (a tool run,
-- then a streamed answer and a pull request). A real app passes only
-- `chat` (or nothing) and gets all this from `app.chat`.
local agent_chat = use "agent-chat"

local COMMANDS = {
	{ name = "review", description = "Review the current changes" },
	{ name = "compact", description = "Summarize the conversation to save context" },
	{ name = "init", description = "Write an AGENTS.md for this repository" },
}

local AGENTS = { claude = "Claude", codex = "Codex" }

local function picker(id: string, name: string, current: string, choices: { any }): any
	local current_name = current
	for _, choice in choices do
		if choice.id == current then
			current_name = choice.name
		end
	end
	return {
		id = id, name = name, send = if id == "mode" then "mode" else "setting", kind = "select",
		current = current, current_name = current_name, choices = choices,
	}
end

-- The rows a pretend session starts with.
local function opening(): { any }
	local rows: { any } = {
		{ type = "user", key = "e1", text = "Why does the parser test fail on Windows?", context = true },
		{
			type = "tools", key = "e2", title = "Read 2 files, ran 1 command", status = "done", live = false,
			count = 3, failed = 0, more = 0, files = {},
			items = {
				{ type = "thought", key = "e2", gist = "Line endings, most likely", text = "" },
				{ type = "tool", id = "a", kind = "read", title = "Read src/parse.rs", status = "done", steps = 0, files = {} },
				{ type = "tool", id = "b", kind = "read", title = "Read tests/parse.rs", status = "done", steps = 0, files = {} },
				{ type = "tool", id = "c", kind = "execute", title = "cargo test parse", status = "done", steps = 0, files = {}, detail = "test parse::crlf ... FAILED\ntest result: FAILED. 13 passed; 1 failed" },
			},
		},
		{ type = "answer", key = "e6", streaming = false, text = "The test reads a fixture with **`\\r\\n`** line endings, and `split_lines` only splits on `\\n`, so every line keeps a trailing `\\r`.\n\nI can fix `split_lines` to accept both endings and add a test for it." },
		{ type = "turn", key = "e7", ["end"] = "done", duration_ms = 41000, tokens = 18400, text = "Worked 41s · 18.4k tokens" },
	}
	return rows
end

local function view(state: any): any
	return {
		id = "demo",
		status = "live",
		view = {
			agent = AGENTS[state.agent] or state.agent, status = state.status, accepts = true, read_only = false,
			auto_accept = state.auto_accept, hidden = 0, truncated = false,
			rows = state.rows,
			permissions = state.permissions,
			queue = state.queue,
			plan = state.plan,
			mode = picker("mode", "Mode", state.mode, {
				{ id = "default", name = "Ask first" },
				{ id = "acceptEdits", name = "Accept edits" },
				{ id = "plan", name = "Plan" },
			}),
			model = picker("model", "Model", state.model, {
				{ id = "opus", name = "Opus 5.5", description = "Best for everyday, complex tasks" },
				{ id = "sonnet", name = "Sonnet 5.5", description = "Efficient for routine tasks" },
				{ id = "haiku", name = "Haiku 5.5", description = "Fastest for quick answers" },
			}),
			effort = picker("effort", "Effort", state.effort, {
				{ id = "low", name = "Low" },
				{ id = "medium", name = "Medium" },
				{ id = "high", name = "High" },
			}),
			settings = {},
			usage = { used = 45200, size = 200000, percent = 22, text = "45.2k / 200k", cost = "$0.42" },
			commands = COMMANDS,
			activities = state.activities,
			working_for = if state.status == "working" then `{8 + state.stage * 7}s` else nil,
			can_steer = true,
			agents = {
				{ id = "claude", name = "Claude", current = state.agent == "claude" },
				{ id = "codex", name = "Codex", current = state.agent == "codex" },
			},
			pull_requests = state.pulls,
			clients = 0,
		},
	}
end

-- The pretend agent's next move in a running turn.
local function step(state: any)
	local last = state.rows[#state.rows]
	if state.stage == 1 then
		table.insert(state.rows, {
			type = "tools", key = `t{state.serial}`, title = "Edit src/parse.rs", status = "running", live = true,
			count = 1, failed = 0, more = 0, current = "Edit src/parse.rs", files = {},
			items = { { type = "tool", id = "e", kind = "edit", title = "Edit src/parse.rs", status = "pending", steps = 0, files = {} } },
		})
		state.permissions = { {
			id = 1, title = "Allow editing src/parse.rs?", tool = "Edit src/parse.rs",
			options = {
				{ id = "allow", name = "Allow", kind = "allow_once", allows = true },
				{ id = "always", name = "Always allow edits", kind = "allow_always", allows = true },
				{ id = "reject", name = "Reject", kind = "reject_once", allows = false },
			},
		} }
		state.activities = { {
			call_id = "sub", kind = "subagent", label = "Look for other CRLF bugs", detail = "Reading src/lexer.rs",
			started_ms = 0, running_for = "6s", stoppable = true,
		} }
		state.plan = {
			done = 1, total = 3, current = "Split on both line endings",
			items = {
				{ text = "Find why the test fails", status = "done", priority = "high" },
				{ text = "Split on both line endings", status = "running", priority = "high" },
				{ text = "Add a CRLF test", status = "pending", priority = "medium" },
			},
		}
	elseif state.stage == 2 then
		local file = { path = "src/parse.rs", added = 4, removed = 1, new = false }
		last.status, last.live, last.current, last.title = "done", false, nil, "Edited 1 file, ran 1 command"
		last.count, last.files = 2, { file }
		last.items[1].status, last.items[1].files = "done", { file }
		table.insert(last.items, { type = "tool", id = "t", kind = "execute", title = "cargo test parse", status = "done", steps = 0, files = {}, detail = "test result: ok. 14 passed" })
		table.insert(state.rows, { type = "answer", key = `a{state.serial}`, streaming = true, text = "Fixed: `split_lines`" })
		state.plan.done, state.plan.items[2].status, state.plan.items[3].status = 2, "done", "running"
	elseif state.stage == 3 then
		last.text ..= " now splits on `\\r\\n` as well as `\\n`, and"
	else
		last.text ..= " the new `crlf` test passes on every platform."
		last.streaming = false
		table.insert(state.rows, { type = "turn", key = `s{state.serial}`, ["end"] = "done", duration_ms = 23000, tokens = 9100, text = "Worked 23s · 9.1k tokens" })
		state.status, state.plan, state.activities = "idle", nil, {}
		state.pulls = { {
			url = "https://github.com/moonframehq/moonframe/pull/42", repo = "moonframehq/moonframe", number = 42,
			actions = { "created", "pushed" }, state = "Pushed",
		} }
		-- A queued prompt goes next.
		local queued: any = table.remove(state.queue, 1)
		if queued then
			state.serial += 1
			table.insert(state.rows, { type = "user", key = `u{state.serial}`, text = queued.text, context = true })
			table.insert(state.rows, { type = "answer", key = `q{state.serial}`, streaming = false, text = "Done: that is all the turn changed." })
		end
		return
	end
	state.stage += 1
end

-- A question the agent asks with choices.
local function question(): any
	return {
		id = 2, title = "How should the fix treat a lone \\r?", options = {},
		question = {
			header = "Line endings", text = "How should the fix treat a lone \\r?", multiple = false,
			options = {
				{ label = "As a line end", description = "What files from old Macs expect" },
				{ label = "As part of the line", description = "Only \\n and \\r\\n end a line" },
			},
		},
	}
end

return mod {
	init = function(previous: any): any
		local state = previous or {
			rows = opening(), status = "idle", permissions = {}, queue = {}, plan = nil,
			mode = "default", model = "opus", effort = "medium", auto_accept = false,
			stage = 0, serial = 0,
		}
		-- Fields a state saved before they existed lacks.
		state.activities = state.activities or {}
		state.pulls = state.pulls or {}
		state.agent = state.agent or "claude"
		return state
	end,

	update = function(state: any, msg: any)
		if msg == "next" then
			if state.status == "working" and #state.permissions == 0 then
				step(state)
			end
		elseif msg == "ask" then
			state.permissions = { question() }
		elseif type(msg) ~= "table" then
			return
		elseif msg.prompt then
			state.serial += 1
			if state.status == "working" then
				table.insert(state.queue, {
					id = `q{state.serial}`, text = msg.prompt, context = msg.context ~= nil, files = {},
					held = false, steered = false,
				})
				return
			end
			table.insert(state.rows, { type = "user", key = `u{state.serial}`, text = msg.prompt, context = msg.context ~= nil })
			state.status, state.stage = "working", 1
			step(state)
		elseif msg.answer and msg.answers then
			state.permissions = {}
			state.serial += 1
			table.insert(state.rows, { type = "answer", key = `qa{state.serial}`, streaming = false, text = `Understood: **{msg.answers[1]}**.` })
		elseif msg.answer then
			state.permissions = {}
			if msg.option == "reject" then
				local last = state.rows[#state.rows]
				last.status, last.live, last.items[1].status = "failed", false, "failed"
				state.status, state.plan, state.activities = "idle", nil, {}
			end
		elseif msg.cancel then
			state.status, state.permissions, state.plan, state.activities = "idle", {}, nil, {}
			table.insert(state.rows, { type = "turn", key = `c{state.serial}`, ["end"] = "cancelled", duration_ms = 4000, text = "Stopped after 4s" })
			-- Queued prompts are held until sent now or taken back.
			for _, queued in state.queue do
				queued.held = true
			end
		elseif msg.unqueue then
			for index, queued in state.queue do
				if queued.id == msg.unqueue then
					table.remove(state.queue, index)
				end
			end
		elseif msg.send_now then
			for index, queued in state.queue do
				if queued.id == msg.send_now then
					if state.status == "working" then
						queued.steered = true
					else
						table.remove(state.queue, index)
						table.insert(state.rows, { type = "user", key = `u{queued.id}`, text = queued.text, context = true })
					end
				end
			end
		elseif msg.stop_activity then
			state.activities = {}
		elseif msg.agent then
			state.serial += 1
			table.insert(state.rows, {
				type = "handoff", key = `h{state.serial}`, from = state.agent, to = msg.agent, kept = #state.rows,
				omitted = 0, resumed = false,
				text = `Continued in {AGENTS[msg.agent] or msg.agent}: {#state.rows} entries handed over`,
			})
			state.agent = msg.agent
		elseif msg.mode then
			state.mode = msg.mode
		elseif msg.setting then
			state[msg.setting] = msg.value
		elseif msg.auto_accept ~= nil then
			state.auto_accept = msg.auto_accept
		end
	end,

	view = function(state: any)
		local working = state.status == "working"
		return ui.column {
			width = 640,
			height = 620,
			gap = "small",
			ui.row {
				gap = "small",
				ui.button { "Next step", icon = "play", small = true, msg = "next", disabled = not working or #state.permissions > 0 },
				ui.button { "Ask a question", icon = "circle-help", small = true, variant = "ghost", msg = "ask", disabled = #state.permissions > 0 },
				ui.text { if working then "The agent is working: step it on, or answer it." else "Send a prompt, or try / for commands.", size = "small", tone = "muted" },
			},
			ui.column {
				grow = true,
				border = true,
				agent_chat {
					id = "demo",
					chat = "demo",
					view = view(state),
					to = "parent",
					context = "The user is in src/parse.rs",
					placeholder = "Ask Claude about this repository…",
				},
			},
		}
	end,
}
