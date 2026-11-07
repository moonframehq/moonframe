-- A tab showing the repository's commits as a graph. Git reads them only
-- while a bar shows this tab: the mod in its own place follows the bars and
-- asks, and each tab draws what comes back. A copy mounted in a panel of its
-- own, which no bar speaks for, says so with `always = true` and asks itself.
local commit_graph = use "commit-graph"
local empty_state = use "empty-state"

local COMMITS = 200
local KIND = "mod:history"

type State = {
	-- The bars showing this tab now, by id.
	shown: { [string]: boolean? },
	-- Whether git was asked for the log. Every message to it looks at the
	-- status again, and a bar publishes at each change of a title, so only a
	-- change of answer is sent.
	asking: boolean,
}

-- The tab bar a message is the news of, if any. Only the mod in its own place
-- follows them, not each tab.
local function bar_of(msg: any, props: any): string?
	if props == nil and type(msg) == "table" and type(msg.topic) == "string" then
		return string.match(msg.topic, "^tabs/([^/]+)$")
	end
	return nil
end

return mod {
	title = "History",
	tab = { title = "History", icon = "git-branch" },
	subscribe = { "tabs/*" },

	-- Never the saved state: git starts with no log asked for, and each bar's
	-- topic comes again as this loads.
	init = function(previous: State?, props: any): (State, { any }?)
		local state: State = { shown = {}, asking = false }
		if props and props.always then
			return state, { { to = "provider:git", msg = { log = COMMITS } } }
		end
		return state
	end,

	update = function(state: State, msg: any, props: any): (State?, { any }?)
		local bar = bar_of(msg, props)
		if not bar then
			return nil
		end
		local tab = type(msg.value) == "table" and msg.value.tab
		state.shown[bar] = if tab and tab.kind == KIND then true else nil
		local asking = next(state.shown) ~= nil
		if asking == state.asking then
			return state
		end
		state.asking = asking
		return state, { { to = "provider:git", msg = { log = if asking then COMMITS else false } } }
	end,

	view = function(state: State, app, props: any)
		local log = app.git and app.git.log
		if not log and app.git and app.git.repository then
			-- This tab was only just shown, or git has only just started.
			return ui.text { "Reading commits…", tone = "muted" }
		end
		if not log or #log == 0 then
			return empty_state {
				icon = "git-branch",
				title = "No commits",
				description = "Commits show here once the repository has some.",
			}
		end
		return ui.column {
			grow = true,
			scroll = true,
			padding = "small",
			commit_graph { commits = log, compact = props and props.compact },
		}
	end,
}
