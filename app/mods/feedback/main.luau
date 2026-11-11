-- Make it yours: accent sparkles in the title bar open a dialog where the
-- user writes what should change in the app. The `acp` provider hands it
-- to a coding agent (Claude by default, over the Agent Client Protocol)
-- working on the app's own files, so the window reloads with the change.
-- The provider tells the agent about whichever app runs this (its folders,
-- its check), and undoes a change that breaks it. The dialog follows each
-- run under "Feedback & fixes", where the agent asks before what
-- providers/acp's `allow` does not cover: by default, anything but
-- reading and editing the app's own folder and running its check. The icon
-- spins while an agent works, and shows a dot when one finished since the
-- dialog was last open. Inspect in the dialog picks an element in the
-- window to attach, or to copy for an agent of the user's own; the app's
-- quick inspect shortcut does the same with the dialog closed, and using
-- the pick opens it. The mic beside it dictates the request instead (see
-- providers/speech). The shell mounts it in its title bar.
local feedback = use "feedback"
local dropdown = use "dropdown"
local dictation = use "dictation"

-- The dialog going away takes the mic button with it, so the take is
-- cancelled here or the microphone would stay on until the limit.
local STOP_LISTENING = { to = "provider:speech", msg = { cancel = true } }

type State = {
	open: boolean,
	text: string,
	agent: string?,
	picking: boolean,
	-- The element picked with Inspect to go with the request:
	-- { element, mod, at, report } from `app/inspect`. Dropped when the
	-- dialog closes and not kept across launches.
	attached: any?,
	-- The attached element's Copy button was pressed.
	copied: boolean,
	-- Picking for the dialog: it hides until the pick comes back.
	inspecting: boolean,
	-- The dialog was open when picking started.
	reopen: boolean,
	-- The mic button is listening or transcribing.
	listening: boolean?,
	expanded: string?,
	error: string?,
	-- The newest time one of our runs finished, and the newest the user has
	-- seen in the dialog.
	latest: number,
	seen: number,
	-- The main tab's title, from `tabs/main`: what they are looking at.
	tab: string?,
	-- The reply being written under the open conversation, and the
	-- conversation a follow-up being started goes in.
	reply: string,
	following: string?,
	-- The last request came from the Developer tools (`quiet`): a failure
	-- shows there, not by opening the dialog.
	quiet: boolean?,
	-- From `app.acp`: our runs (newest first), and this window's process,
	-- to screenshot.
	runs: { any },
	pid: number?,
}

-- How the agent looks at the user's window (the process `pid`), when the
-- change calls for it: a color or a word that passes the check does not.
local function look(pid: number?): string
	local window = if pid then `the user's window (process {pid})` else "the user's window"
	return `Look at {window} only for a change of layout or size, or when you cannot tell from the code that it shows: on macOS, \`screencapture -x -o -l<window number> <file>\`, with the number from CGWindowListCopyWindowInfo for that process. A change of color, text or style that passes the check needs no screenshot.`
end

-- How the agent should work on the app. The provider adds what it needs
-- to know about this app (`app = true`): its folders, where to write, the
-- command that checks it, a short guide and an index of its files, from
-- the app that runs it, so nothing here names this repository's tools.
local function instructions(pid: number?, follow: boolean): string
	return [[
The user of this Moonframe app wrote this in the app's "Make it yours" dialog. Change the app so it does what they ask.

How to work:
- Follow "This app" below: change files only where it says, and run its check command after your edits until it reports no problems. Go straight to the file: the context's Inspect pick and the file index say where things are.
- Keep the change to what they ask for, as small as it can be. When it is unclear which part they mean and nothing was picked with Inspect, change the likeliest one and say which: do not hunt for it.
- ]] .. look(pid) .. [[

- Do not commit, push, build, or stop the user's running app: it reloads by itself.
- End with two or three plain sentences for the user: what you changed and where; or why you did not.]]
		.. if follow
			then [[


This continues a conversation: `conversation` in the context has the user's earlier requests and the answers given, oldest first, and their new words say what is still wrong. Do not trust the earlier answers: read the files as they are now before changing anything.]]
			else ""
end

-- Starts the agent ahead of the request, while the user writes it, so the
-- run that follows takes it without waiting for it to start.
local function warm(agent: string?): any
	return { to = "provider:acp", msg = { warm = true, agent = agent } }
end

-- Where the agent works, with the home folder as ~.
local function short(path: string): string
	local home = string.match(path, "^(/Users/[^/]+)") or string.match(path, "^(/home/[^/]+)")
	if home then
		return "~" .. string.sub(path, #home + 1)
	end
	return path
end

-- Our runs, newest first.
local function ours(acp: any): { any }
	local runs = {}
	for _, run in (acp and acp.runs) or {} do
		if run.owner == "feedback" then
			table.insert(runs, run)
		end
	end
	return runs
end

-- The newest time one of `runs` finished.
local function newest(runs: { any }): number
	local latest = 0
	for _, run in runs do
		if run.finished and run.finished > latest then
			latest = run.finished
		end
	end
	return latest
end

local function going(run: any): boolean
	return run.status == "starting" or run.status == "running" or run.status == "waiting"
end

return mod {
	title = "Feedback",
	subscribe = { "tabs/main", "provider/acp", "app/inspect" },

	init = function(previous: State?): State
		local seen = previous and previous.seen or 0
		return {
			open = false,
			text = "",
			agent = previous and previous.agent,
			picking = false,
			-- Not kept: a pick belongs to the request being written.
			attached = nil,
			copied = false,
			inspecting = false,
			reopen = false,
			expanded = nil,
			error = nil,
			latest = seen,
			seen = seen,
			tab = nil,
			reply = "",
			following = nil,
			runs = {},
			pid = nil,
		}
	end,

	update = function(state: State, msg)
		if type(msg) ~= "table" then
			if msg == "open" or msg == "close" then
				-- What the dialog showed counts as seen.
				state.open = msg == "open"
				state.picking = false
				state.seen = state.latest
				if state.open then
					state.error = nil
					return state, { warm(state.agent) }
				else
					-- Closing drops the pick; Inspect hides the dialog
					-- without `close`, so it survives picking.
					state.attached = nil
					state.copied = false
					if state.listening then
						state.listening = false
						return state, { STOP_LISTENING }
					end
				end
			elseif msg == "pick-agent" then
				state.picking = not state.picking
			elseif msg == "close-agents" then
				state.picking = false
			elseif msg == "detach" then
				state.attached = nil
				state.copied = false
			elseif msg == "copied" then
				state.copied = true
			end
			return nil
		end
		if msg.topic == "app/inspect" then
			-- Inspect Element, started from the dialog's Inspect button.
			local pick = msg.value
			if pick.asker ~= "feedback" then
				return nil
			elseif pick.picking then
				-- Escape goes back to how it was; a pick always opens the dialog,
				-- as the app's quick inspect shortcut starts from no dialog.
				if not state.inspecting then
					state.reopen = state.open
				end
				state.inspecting = true
				state.open = false
				if state.listening then
					state.listening = false
					return state, { STOP_LISTENING }
				end
			elseif state.inspecting then
				state.inspecting = false
				state.open = pick.picked or state.reopen
				if state.open then
					state.seen = state.latest
				end
				if pick.picked then
					state.attached = { element = pick.element, mod = pick.mod, at = pick.at, report = pick.report }
					state.copied = false
				end
			end
		elseif msg.topic == "tabs/main" then
			state.tab = msg.value and msg.value.title
		elseif msg.topic == "provider/acp" then
			state.runs = ours(msg.value)
			state.pid = msg.value and msg.value.pid
			state.latest = newest(state.runs)
			if state.open then
				state.seen = state.latest
			end
		elseif msg.msg == "type" then
			state.text = msg.value
		elseif msg.msg == "dictated" then
			local blank = string.match(state.text or "", "^%s*$") ~= nil
			state.text = if blank then msg.value else `{state.text} {msg.value}`
		elseif msg.msg == "listening" then
			state.listening = msg.value and state.open
			if msg.value and not state.open then
				return state, { STOP_LISTENING }
			end
		elseif msg.msg == "dictation-failed" then
			state.error = msg.value
		elseif msg.agent_pick then
			state.agent = msg.agent_pick
			state.picking = false
			return state, { warm(state.agent) }
		elseif msg.expand then
			state.expanded = if state.expanded == msg.expand then nil else msg.expand
			state.reply = ""
		elseif msg.follow_type then
			state.reply = msg.value or ""
		elseif msg.forget_thread then
			-- Each run of the conversation.
			local sends = {}
			for _, run in state.runs do
				if run.id == msg.forget_thread or run.thread == msg.forget_thread then
					table.insert(sends, { to = "provider:acp", msg = { forget = run.id } })
				end
			end
			return state, sends
		elseif msg.undo_thread then
			-- Newest first, so the app ends up as the conversation found it.
			local sends = {}
			for _, run in state.runs do
				if (run.id == msg.undo_thread or run.thread == msg.undo_thread) and run.changed and not run.undone then
					table.insert(sends, { to = "provider:acp", msg = { undo = run.id } })
				end
			end
			return state, sends
		elseif msg.follow then
			-- A follow-up in the conversation `msg.follow`, with what was said.
			local text = msg.value or state.reply
			if string.match(text or "", "^%s*$") then
				return nil
			end
			local said = {}
			for index = #state.runs, 1, -1 do
				local run = state.runs[index]
				if run.id == msg.follow or run.thread == msg.follow then
					table.insert(said, { request = run.text, answer = run.message or run.error or run.status })
				end
			end
			state.reply = ""
			state.error = nil
			state.following = msg.follow
			state.quiet = msg.quiet
			return state,
				{
					{
						to = "provider:acp",
						msg = {
							start = text,
							id = "feedback",
							thread = msg.follow,
							agent = state.agent,
							app = true,
							instructions = instructions(state.pid, true),
							context = { main_tab = state.tab, conversation = said },
						},
					},
				}
		elseif msg.send then
			-- The Send button can't read the field, so it carries no `value`;
			-- ⌘↩ does.
			local text = msg.value or state.text
			if string.match(text or "", "^%s*$") then
				return nil
			end
			local context = msg.context or {}
			if state.attached then
				context.inspect_element_selection = state.attached.report
			end
			-- The dialog stays open on the new run, which opens under the
			-- field as it starts, so the user follows it and replies there.
			state.text = ""
			state.error = nil
			state.attached = nil
			state.copied = false
			state.quiet = msg.quiet
			local sends: { any } = {
				{
					to = "provider:acp",
					msg = {
						start = text,
						id = "feedback",
						agent = state.agent,
						app = true,
						instructions = instructions(state.pid, false),
						context = context,
					},
				},
			}
			if state.listening then
				state.listening = false
				table.insert(sends, STOP_LISTENING)
			end
			return state, sends
		elseif msg.reply == "feedback" and msg.action == "finished" then
			-- Ready for a follow-up while the dialog is still open.
			if state.open then
				return state, { warm(state.agent) }
			end
		elseif msg.reply == "feedback" and msg.action == "start" then
			local following = state.following
			state.following = nil
			local quiet = state.quiet
			state.quiet = nil
			if msg.ok then
				state.expanded = following or msg.run
			else
				-- In the dialog, with why; the Developer tools show it themselves.
				state.open = state.open or not quiet
				state.error = msg.error
			end
		end
		return nil
	end,

	view = function(state: State, app)
		local acp = app.acp
		local runs = ours(acp)
		local running, asking = 0, false
		for _, run in runs do
			if going(run) then
				running += 1
			end
			asking = asking or run.status == "waiting"
		end
		local unseen = state.latest > state.seen
		local window = app.window
		local git = app.git

		-- What goes with the request: what the user is looking at.
		local shape = window.platform
		if window.fullscreen then
			shape ..= ", fullscreen"
		elseif window.maximized then
			shape ..= ", maximized"
		end
		local context = {
			main_tab = state.tab,
			window = shape,
			branch = git and git.available and git.branch or nil,
			changed_files = git and git.available and git.changed or nil,
			build = git and git.available and git.head and `{git.head.short} {git.head.subject}` or nil,
		}

		-- The agents this machine can run, for the picker.
		local options = {}
		local chosen = state.agent or (acp and acp.agent) or "claude"
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
			note = `A background agent ({title}) changes {short(writable)} {asks}; your window reloads with the change, a change that breaks the app is undone, and it reports back under Feedback & fixes. It gets what you're looking at, your window and the build.`
		end

		-- Sparkles, not a help `?`: this changes the app, it is not docs.
		local icon = if asking then "circle-alert" elseif running > 0 then "loader-circle" else "sparkles"
		return ui.row {
			gap = "none",
			-- The dialog is beside the button, not in it, so clicks in the
			-- dialog never reach the button.
			ui.row {
				-- `feedback-button` lets an app reshape the button (a round one
				-- in a round corner) without restyling every `tool`.
				class = if asking then "tool feedback-button feedback-magic feedback-asking" else "tool feedback-button feedback-magic",
				align = "center",
				justify = "center",
				msg = "open",
				tooltip = if asking
					then "An agent needs your answer"
					elseif running > 0 then "Make it yours (an agent is working)"
					else "Make it yours: point at anything, say what to change",
				ui.icon { name = icon :: any, spin = running > 0 and not asking },
				unseen and running == 0 and ui.row { class = "feedback-dot", float = { top = 3, right = 3 } },
			},
			feedback {
				open = state.open,
				dismiss = "close",
				value = state.text,
				text = if #options > 0 then state.text else "",
				change = "type",
				send = { send = true, context = context },
				note = note,
				error = state.error,
				runs = runs,
				expanded = state.expanded,
				follow = true,
				undo = true,
				reply = state.reply,
				actions = {
					dictation { id = "prompt", text = "dictated", failed = "dictation-failed", listening = "listening" },
					(#options > 0 and dropdown {
						label = "Agent",
						open = state.picking,
						toggle = "pick-agent",
						dismiss = "close-agents",
						selected = chosen,
						value = title,
						options = options,
						event = "agent_pick",
						side = "top",
					}) :: any,
				},
				inspect = true,
				attached = state.attached,
				detach = "detach",
				copy = "copied",
				copied = state.copied,
			},
		}
	end,

	styles = {
		[".feedback-dot"] = { width = 6, height = 6, radius = 3, background = "accent" },
		-- Accent, so it stands apart from the other tools.
		[".feedback-magic"] = { color = "accent" },
		[".feedback-magic:hover"] = { background = "accent/14", color = "accent" },
		[".feedback-asking"] = { color = "warning" },
		[".feedback-asking:hover"] = { color = "warning" },
	},
}
