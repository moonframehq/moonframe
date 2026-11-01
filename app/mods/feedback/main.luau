-- Send feedback: a ? in the title bar opens a dialog where the user writes
-- what should change in the app. The `acp` provider hands it to a coding
-- agent (Claude by default, over the Agent Client Protocol) working on the
-- app's own files with every permission, so the window reloads with the
-- change; the dialog follows each run under "Feedback & fixes". The icon
-- spins while an agent works, and shows a dot when one finished since the
-- dialog was last open. Inspect in the dialog picks an element in the
-- window to attach. The shell mounts it in its title bar.
local feedback = use "feedback"
local dropdown = use "dropdown"

type State = {
	open: boolean,
	text: string,
	agent: string?,
	picking: boolean,
	-- The element picked with Inspect to go with the request:
	-- { element, mod, at, report } from `app/inspect`.
	attached: any?,
	-- Picking for the dialog: it hides until the pick comes back.
	inspecting: boolean,
	expanded: string?,
	error: string?,
	-- The newest time one of our runs finished, and the newest the user has
	-- seen in the dialog.
	latest: number,
	seen: number,
	-- The main tab's title, from `tabs/main`: what they are looking at.
	tab: string?,
}

-- How the agent should work on this app; the user's words follow.
local INSTRUCTIONS = [[
The user of the Moonframe app wrote this in the app's "Send feedback" dialog. Change the app so it does what they ask.

How to work:
- Read the moonframe skill (.claude/skills/moonframe/SKILL.md in the Moonframe repository) before changing anything. The UI is Luau under app/ (or ~/.config/moonframe, laid out the same way), and the user's open window reloads as files change, so a change that passes the checks is live at once.
- Keep the change to what they ask for.
- Run `just check` and `just types` (or `moonframe --check <folder>` where there is no justfile) until they pass.
- Do not commit, push, or stop the user's running app.
- End with two or three plain sentences for the user: what you changed and where, or why you did not.]]

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
			attached = previous and previous.attached,
			inspecting = false,
			expanded = nil,
			error = nil,
			latest = seen,
			seen = seen,
			tab = nil,
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
			state.latest = newest(ours(msg.value))
			if state.open then
				state.seen = state.latest
			end
		elseif msg.msg == "type" then
			state.text = msg.value
		elseif msg.agent_pick then
			state.agent = msg.agent_pick
			state.picking = false
		elseif msg.expand then
			state.expanded = if state.expanded == msg.expand then nil else msg.expand
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
			return state,
				{
					{
						to = "provider:acp",
						msg = {
							start = text,
							id = "feedback",
							agent = state.agent,
							instructions = INSTRUCTIONS,
							context = context,
							permissions = "allow",
						},
					},
				}
		elseif msg.reply == "feedback" and msg.action == "start" then
			if msg.ok then
				state.expanded = msg.run
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
			note = `A background agent ({title}) in {short(acp.workdir)} implements it with every permission; your window reloads with the change, and it reports back under Feedback & fixes. It gets what you're looking at, your window and the build.`
		end

		local icon = if asking then "circle-alert" elseif running > 0 then "loader-circle" else "circle-help"
		return ui.row {
			gap = "none",
			-- The dialog is beside the button, not in it, so clicks in the
			-- dialog never reach the button.
			ui.row {
				class = if asking then "tool feedback-asking" else "tool",
				align = "center",
				justify = "center",
				msg = "open",
				tooltip = if asking
					then "An agent needs your answer"
					elseif running > 0 then "Send feedback (an agent is working)"
					else "Send feedback",
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
				actions = {
					#options > 0 and dropdown {
						label = "Agent",
						open = state.picking,
						toggle = "pick-agent",
						dismiss = "close-agents",
						selected = chosen,
						value = title,
						options = options,
						event = "agent_pick",
						side = "top",
					},
				},
				inspect = true,
				attached = state.attached,
				detach = "detach",
			},
		}
	end,

	styles = {
		[".feedback-dot"] = { width = 6, height = 6, radius = 3, background = "accent" },
		[".feedback-asking"] = { color = "warning" },
	},
}
