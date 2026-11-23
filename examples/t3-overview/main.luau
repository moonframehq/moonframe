local card = use "dashboard-card"
local system_load = use "system-load"
local meter = use "meter"
local sessions = use "t3-sessions"

type State = { tab: string, folded: { [string]: boolean }, selected: string? }

local tabs = { "Sessions", "Pull requests", "Issues", "Worktrees" }

return mod {
	subscribe = { "provider/t3", "provider/system", "provider/usage" },
	init = function(previous: State?): State
		return previous or { tab = "Sessions", folded = {} }
	end,
	update = function(state: State, msg)
		if msg == "refresh" then
			return state, { { to = "provider:t3", msg = "refresh" } }
		elseif type(msg) == "table" then
			if msg.tab then
				state.tab = msg.tab
			elseif msg.fold then
				state.folded[msg.fold] = not state.folded[msg.fold]
			elseif msg.thread then
				state.selected = if state.selected == msg.thread then nil else msg.thread
			end
		end
		return state
	end,
	view = function(state: State, app)
		local data = app.t3
		local threads = if data then data.threads else {}
		local filtered = {}
		local prs: { [string]: boolean } = {}
		for _, thread in threads do
			for _, pr in thread.pullRequests do
				prs[pr.url] = true
			end
			if state.tab == "Sessions" or (state.tab == "Pull requests" and #thread.pullRequests > 0)
				or (state.tab == "Issues" and #thread.issues > 0) or (state.tab == "Worktrees" and thread.worktree) then
				table.insert(filtered, thread)
			end
		end
		local prCount = 0
		for _ in prs do prCount += 1 end
		local system = app.system
		local services: { Element } = {}
		if app.usage then
			for _, service in app.usage.providers do
				table.insert(services, ui.row { align = "center", ui.text(service.name), ui.spacer(),
					if service.used then meter { value = service.used, width = 56 } else ui.text "Unavailable" })
			end
		end
		if #services == 0 then table.insert(services, ui.text "No usage reported") end
		local buttons: { Element } = {}
		for _, tab in tabs do
			table.insert(buttons, ui.button { label = tab, variant = if state.tab == tab then "primary" else "ghost", msg = { tab = tab } })
		end
		return ui.column {
			class = "overview", grow = true,
			ui.row { align = "center", ui.text { "Overview", size = "large", weight = "semibold" }, ui.spacer(), ui.button { label = "Refresh", msg = "refresh" } },
			ui.row { align = "stretch",
				card { title = "Linked pull requests", grow = true,
					if data and data.available then ui.text { tostring(prCount), size = "headline", weight = "semibold" } else ui.text "Unavailable" },
				card { title = "This machine", grow = true,
					if system then system_load { cpu = system.cpu, memory = system.memory, history = system.cpu_history } else ui.text "Waiting for sample" },
				card { title = "Usage", grow = true, table.unpack(services) },
			},
			ui.row { table.unpack(buttons) },
			if not data then ui.text "Connecting to T3 Code…"
			elseif not data.available then ui.text(data.error or "T3 Code unavailable")
			elseif #filtered == 0 then ui.text "No matching sessions"
			else ui.column { scroll = true, grow = true, sessions { threads = filtered, folded = state.folded, selected = state.selected } },
			data and data.total > #threads and ui.text { `Showing the newest {#threads} of {data.total} sessions` },
		}
	end,
}
