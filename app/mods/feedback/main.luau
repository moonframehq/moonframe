-- Make it yours: accent sparkles in the title bar open a dialog where the
-- user writes what should change in the app. The `acp` provider hands it
-- to a coding agent (Claude by default, over the Agent Client Protocol)
-- working on the app's own files, so the window reloads with the change;
-- the dialog follows each run under "Feedback & fixes", where the agent
-- asks before each change unless providers/acp's `allow` lets it. The icon
-- spins while an agent works, and shows a dot when one finished since the
-- dialog was last open. Inspect in the dialog picks an element in the
-- window to attach. The mic beside it dictates the request instead (see
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
	-- Picking for the dialog: it hides until the pick comes back.
	inspecting: boolean,
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
	-- From `app.acp`: our runs (newest first), the app folder agents
	-- change, and this window's process, to screenshot.
	runs: { any },
	folder: string?,
	pid: number?,
	-- From `app.acp`, in an installed app: where its signed files are, and
	-- the user's folder over them, which agents change instead.
	installed: any?,
}

-- How the agent checks its change in the user's window, the process `pid`.
local function look(pid: number?): string
	local shot = if pid
		then `\`just screenshot target/feedback.png {pid}\` (the user's window, process {pid}, which reloaded with your change)`
		else "`just screenshot target/feedback.png <pid of the user's window>`"
	return `Then look at it: screenshot it with {shot}, read the picture, and check it shows what they asked for. Checks passing is not enough. If it does not, find out why, fix it, and look again, until it does or you know why it cannot.`
end

-- How the agent looks at the window of an installed app, which has no
-- justfile around it.
local function look_installed(pid: number?): string
	local window = if pid then `the user's window (process {pid})` else "the user's window"
	return `Then look at it: screenshot {window}, which reloaded with your change (on macOS, find its window number with CGWindowListCopyWindowInfo, then \`screencapture -x -o -l<number> <file>\`), read the picture, and check it shows what they asked for. Checks passing is not enough. If it does not, find out why, fix it, and look again, until it does or you know why it cannot. If this machine will not let you take a screenshot, say so.`
end

-- Where an agent works on the installed app: never in its signed bundle.
local function overlay(installed: any): string
	return `This is the installed app. Its files are in {installed.files}, inside {installed.bundle}, which is signed: read and copy from there, but never change, add or delete anything in {installed.bundle}, by any means, shell commands included. That breaks the signature, and the next update takes the change away. Work in {installed.folder}, the user's folder, laid out the same way: a file there wins over the one at the same path in the app, the open window reloads at once, and removing it brings the app's back. So to change a built-in file, copy it to the same path there and change the copy. Keep the override as small as you can, because a copied mod no longer gets that mod's updates: a style.luau there is merged over the app's, so for looks write only the rules that change.`
end

-- How the agent should work on the app in `folder` (from the repository
-- agents work in: `app`, or an app package such as `examples/ledger`), or
-- on the `installed` app; the user's words follow.
local function instructions(folder: string?, pid: number?, follow: boolean, installed: any?): string
	local where, check
	local guide = "Read the moonframe skill (.claude/skills/moonframe/SKILL.md in the Moonframe repository) before changing anything, and the moonframe-components catalogue before writing UI."
	local seen = look(pid)
	if installed then
		where = overlay(installed)
		check = `Run \`{installed.check}\` until it passes.`
		seen = look_installed(pid)
		guide = if installed.skill
			then `Read the moonframe skill ({installed.skill}) before changing anything, and the moonframe-components skill beside it before writing UI.`
			else "Read the moonframe skill, if you have it, before changing anything."
	elseif folder == nil or folder == "app" or folder == "." then
		where =
			"The UI is Luau under app/ (or ~/.config/moonframe, laid out the same way), and the user's open window reloads as files change, so a change that passes the checks is live at once."
		check = "Run `just check` and `just types` (or `moonframe --check <folder>` where there is no justfile) until they pass."
	else
		where = `The app is the package in {folder}: its moonframe.toml names it, main.luau is its shell, and its mods/, components/, providers/ and style.luau sit beside it; its README.md says what each part does, so read it first. It runs as a profile over the built-in app in app/, whose components and providers it uses: change {folder}, and app/ only for a component every app should share. The user's open window reloads as files change, so a change that passes the checks is live at once.`
		check = `Check it as \`just check-examples\` does, until it passes: link it into a throwaway user folder (\`target/release/moonframe --user "$DIR" app link {folder} --yes\`, then \`target/release/moonframe --user "$DIR" --profile <its name> --check\`), and run \`just types {folder}\`. Build target/release/moonframe first only if it is missing or you changed Rust.`
	end
	return [[
The user of this Moonframe app wrote this in the app's "Send feedback" dialog. Change the app so it does what they ask.

How to work:
- ]] .. guide .. " " .. where .. [[

- Keep the change to what they ask for.
- ]] .. check .. [[

- ]] .. seen .. [[

- Do not commit, push, or stop the user's running app.
- End with two or three plain sentences for the user: what you changed and where, and what you saw in the window; or why you did not.]]
		.. if follow
			then [[


This continues a conversation: `conversation` in the context has the user's earlier requests and the answers given, oldest first, and their new words say what is still wrong. Do not trust the earlier answers: read the files as they are now and look at the window before changing anything.]]
			else ""
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
			inspecting = false,
			expanded = nil,
			error = nil,
			latest = seen,
			seen = seen,
			tab = nil,
			reply = "",
			following = nil,
			runs = {},
			folder = nil,
			installed = nil,
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
				else
					-- Closing drops the pick; Inspect hides the dialog
					-- without `close`, so it survives picking.
					state.attached = nil
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
			end
			return nil
		end
		if msg.topic == "app/inspect" then
			-- Inspect Element, started from the dialog's Inspect button.
			local pick = msg.value
			if pick.asker ~= "feedback" then
				return nil
			elseif pick.picking then
				state.inspecting = true
				state.open = false
				if state.listening then
					state.listening = false
					return state, { STOP_LISTENING }
				end
			elseif state.inspecting then
				state.inspecting = false
				state.open = true
				if pick.picked then
					state.attached = { element = pick.element, mod = pick.mod, at = pick.at, report = pick.report }
				end
			end
		elseif msg.topic == "tabs/main" then
			state.tab = msg.value and msg.value.title
		elseif msg.topic == "provider/acp" then
			state.runs = ours(msg.value)
			state.folder = msg.value and msg.value.app
			state.installed = msg.value and msg.value.installed
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
			return state,
				{
					{
						to = "provider:acp",
						msg = {
							start = text,
							id = "feedback",
							thread = msg.follow,
							agent = state.agent,
							instructions = instructions(state.folder, state.pid, true, state.installed),
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
			state.text = ""
			state.open = false
			state.error = nil
			state.attached = nil
			local sends: { any } = {
				{
					to = "provider:acp",
					msg = {
						start = text,
						id = "feedback",
						agent = state.agent,
						instructions = instructions(state.folder, state.pid, false, state.installed),
						context = context,
					},
				},
			}
			if state.listening then
				state.listening = false
				table.insert(sends, STOP_LISTENING)
			end
			return state, sends
		elseif msg.reply == "feedback" and msg.action == "start" then
			local following = state.following
			state.following = nil
			if msg.ok then
				state.expanded = following or msg.run
			else
				-- Back in the dialog, with why.
				state.open = true
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
			note = `A background agent ({title}) in {short(acp.workdir)}{if acp.app and acp.app ~= "." then "/" .. acp.app else ""} implements it, asking you here before each change unless providers/acp allows it; your window reloads with the change, and it reports back under Feedback & fixes. It gets what you're looking at, your window and the build.`
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
