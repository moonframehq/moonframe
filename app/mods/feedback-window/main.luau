-- The "Make it yours" window, which the sparkles (mods/feedback) open: a
-- window of its own rather than a dialog over the app, so the app stays
-- usable while an agent works on it. The feedback mod keeps what it shows
-- and talks to the agents; this draws it from the topic `feedback/dialog`
-- and `app.acp`, and its controls send their messages there. Its mic and
-- Inspect answer the mod that drew them, this one, which passes their
-- words and picks on. No region: nothing mounts it.
local feedback = use "feedback"
local dropdown = use "dropdown"
local dictation = use "dictation"

local FEEDBACK = "feedback"

-- Where the agent works, with the home folder as ~.
local function short(path: string): string
	local home = string.match(path, "^(/Users/[^/]+)") or string.match(path, "^(/home/[^/]+)")
	if home then
		return "~" .. string.sub(path, #home + 1)
	end
	return path
end

-- The feedback mod's runs, newest first.
local function ours(acp: any): { any }
	local runs = {}
	for _, run in (acp and acp.runs) or {} do
		if run.owner == FEEDBACK then
			table.insert(runs, run)
		end
	end
	return runs
end

return mod {
	title = "Make it yours",
	-- Sized for the request and its buttons; runs below scroll.
	window = { size = { width = 760, height = 460 }, min_size = { width = 520, height = 320 } },
	subscribe = { "feedback/dialog", "app/inspect" },

	init = function(previous)
		-- Only what the feedback mod shares; a pick or a draft from before
		-- comes again from it.
		return { dialog = previous and previous.dialog or {} }
	end,

	update = function(state, msg)
		if type(msg) == "table" and msg.topic == "feedback/dialog" then
			state.dialog = msg.value or {}
			return state
		elseif type(msg) == "table" and msg.topic == "app/inspect" then
			-- Picks started by this window's Inspect button.
			return state, { { to = FEEDBACK, msg = { inspected = msg.value } } }
		elseif msg == "close-agents" then
			-- The agent picker's popover dismisses to the mod drawing it.
			return state, { { to = FEEDBACK, msg = msg } }
		elseif msg == "stop-listening" then
			return state, { { to = "provider:speech", msg = { cancel = true } } }
		elseif type(msg) == "table" and (msg.msg == "dictated" or msg.msg == "listening" or msg.msg == "dictation-failed") then
			-- The mic drawn here answers here; its words go to the draft.
			return state, { { to = FEEDBACK, msg = msg } }
		end
		return nil
	end,

	view = function(state, app)
		local dialog: any = state.dialog or {}
		local acp = app.acp
		local git = app.git

		-- What goes with the request: what the user is looking at.
		local context = {
			main_tab = dialog.tab,
			window = app.window.platform,
			branch = git and git.available and git.branch or nil,
			changed_files = git and git.available and git.changed or nil,
			build = git and git.available and git.head and `{git.head.short} {git.head.subject}` or nil,
		}

		-- The agents this machine can run, for the picker.
		local options = {}
		local chosen = dialog.agent or (acp and acp.agent) or "claude"
		local title = chosen
		local agents: { any } = (acp and acp.agents) or {}
		for _, agent in agents do
			if agent.installed or agent.fetched then
				table.insert(options, {
					id = agent.name,
					label = agent.title,
					detail = if agent.installed then nil else "via npx",
				})
			end
			if agent.name == chosen then
				title = agent.title
			end
		end

		local note
		if acp == nil then
			note = "Starting…"
		elseif #options == 0 then
			note = "No coding agent is installed here. Install one with an ACP mode (claude-agent-acp, codex-acp, gemini, opencode or copilot), or Node for npx."
		else
			local writable = acp.writable or acp.workdir
			-- What it does without asking, as providers/acp's `allow` says.
			local level = acp.allow and acp.allow.feedback
			local asks = if level == "all"
				then "without asking you"
				elseif level == "app" then "on its own, asking you here before anything outside it"
				elseif level == "read" then "asking you here before each change"
				else "asking you here before each step"
			note = `A background agent ({title}) changes {short(writable)} {asks}; the app reloads with the change, a change that breaks it is undone, and it reports back under Feedback & fixes. Keep using the app meanwhile. It gets what you're looking at, the window and the build.`
		end

		local text = dialog.text or ""
		return ui.column {
			grow = true,
			gap = "none",
			class = "feedback-window",
			feedback {
				inline = true,
				open = true,
				to = FEEDBACK,
				dismiss = "close",
				value = text,
				text = if #options > 0 then text else "",
				change = "type",
				send = { send = true, context = context },
				note = note,
				error = dialog.error,
				runs = ours(acp),
				expanded = dialog.expanded,
				follow = true,
				undo = true,
				reply = dialog.reply,
				actions = {
					-- Drawn here, so it answers here and `update` passes it on.
					dictation { id = "prompt", text = "dictated", failed = "dictation-failed", listening = "listening" },
					(#options > 0 and dropdown {
						label = "Agent",
						open = dialog.picking == true,
						toggle = "pick-agent",
						dismiss = "close-agents",
						selected = chosen,
						value = title,
						options = options,
						event = "agent_pick",
						side = "top",
						to = FEEDBACK,
					}) :: any,
				},
				inspect = true,
				attached = dialog.attached,
				detach = "detach",
				copy = "copied",
				copied = dialog.copied == true,
			},
		}
	end,

	styles = {
		[".feedback-window"] = { background = "background" },
	},
}
