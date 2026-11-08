-- Home: how many sessions there are, how many agents work, and the sessions
-- with something for the user: a question waiting, or an answer not read.

local stat_tile = use "stat-tile"
local empty_state = use "empty-state"

local function going(status: string): boolean
	return status == "starting" or status == "running" or status == "waiting"
end

return mod {
	subscribe = { "cairn/sessions" },

	styles = {
		[".home"] = { padding = { 28, 28 }, gap = 18 },
		[".home-title"] = { size = "title", weight = "semibold" },
		[".home-row"] = { padding = { 9, 12 }, gap = 10, radius = 8, background = "raised" },
		[".home-row:hover"] = { background = "#2a2a2a" },
		[".unread"] = { background = "accent", radius = 999 },
		[".asking"] = { color = "warning" },
	},

	init = function(_previous)
		return { list = {} }
	end,

	update = function(state, msg)
		if type(msg) == "table" and msg.topic == "cairn/sessions" and msg.value then
			state.list = msg.value.list
		end
	end,

	view = function(state, app)
		local working, asking = 0, 0
		local rows = {}
		for _, session: any in state.list do
			if going(session.status) then
				working += 1
			end
			if session.status == "waiting" then
				asking += 1
			end
			if session.status == "waiting" or session.unread then
				table.insert(rows, ui.row { class = "home-row", align = "center", msg = { select = session.id }, to = "sidebar",
					if session.status == "waiting"
						then ui.icon { name = "circle-alert", class = "asking" }
						else ui.row { class = "unread", width = 7, height = 7, shrink = false },
					ui.text { session.title, truncate = true },
					ui.spacer(),
					ui.text { session.name, tone = "muted", size = "small" },
				})
			end
		end
		local found = false
		local agents: { any } = (app.acp and app.acp.agents) or {}
		for _, agent in agents do
			found = found or agent.installed or agent.fetched
		end
		return ui.column { class = "home", grow = true, scroll = true,
			ui.text { "Home", class = "home-title" },
			ui.row { gap = "normal",
				ui.column { grow = true, stat_tile { label = "Sessions", value = tostring(#state.list), icon = "layout-grid" } },
				ui.column { grow = true, stat_tile { label = "Working", value = tostring(working), icon = "activity" } },
				ui.column { grow = true, stat_tile { label = "Need you", value = tostring(asking), icon = "circle-alert" } },
			},
			app.acp and not found and empty_state {
				icon = "circle-alert",
				title = "No coding agent found",
				message = "Install one with an ACP mode (claude-agent-acp, codex-acp, gemini, opencode or copilot), or Node for npx.",
			},
			ui.text { "For you", tone = "muted" },
			#rows == 0 and empty_state { icon = "inbox", title = "All caught up", message = "Questions from agents and new answers show up here." },
			table.unpack(rows),
		}
	end,
}
