local card = use "dashboard-card"
local system_load = use "system-load"
local meter = use "meter"
local composer = use "t3-composer"
local sessions = use "t3-sessions"

type Draft = { action: "start" | "send", target: string?, title: string?, environmentId: string, text: string, id: string }
type State = { tab: string, folded: { [string]: boolean }, selected: string?, draft: Draft?, notice: string?, picker: boolean? }

local function requestId(): string
	return string.format("%08x%08x%08x", math.random(0, 0x3fffffff), math.random(0, 0x3fffffff), math.random(0, 0x3fffffff))
end

local tabs = { "Sessions", "Pull requests", "Issues", "Worktrees" }

return mod {
	subscribe = { "provider/t3", "provider/system", "provider/usage" },
	init = function(previous: State?): State
		return { tab = if previous then previous.tab else "Sessions", folded = if previous then previous.folded else {}, selected = if previous then previous.selected else nil }
	end,
	update = function(state: State, msg)
		if msg == "refresh" then
			return state, { { to = "provider:t3", msg = "refresh" } }
		elseif msg == "t3-picker" then
			state.picker = not state.picker
		elseif msg == "t3-dismiss" then
			state.picker = false
		elseif msg == "t3-cancel" then
			state.draft, state.notice, state.picker = nil, nil, false
		elseif type(msg) == "table" then
			local draft = state.draft
			if msg.topic == "provider/t3" and draft and msg.value and msg.value.results then
				local result = msg.value.results[draft.id]
				if result and result.status == "error" then
					state.notice = result.error
					draft.id = requestId()
				elseif result and result.status ~= "sending" then
					state.notice = if result.status == "queued" then "Message queued" elseif result.status == "started" then "Chat started" else "Message sent"
					if result.status == "started" then state.tab = "Sessions" end
					state.selected, state.draft = result.threadId, nil
				end
			elseif msg.compose and not draft then
				state.notice = nil
				state.draft = { action = msg.compose, environmentId = msg.environmentId, target = msg.target, title = msg.title, text = "", id = requestId() }
			elseif msg.project and draft then
				draft.target, draft.id, state.picker = msg.project, requestId(), false
			elseif msg.msg == "draft" and draft and draft.text ~= msg.value then
				draft.text, draft.id = msg.value, requestId()
			elseif msg.tab then
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
		local selected = nil
		local filtered = {}
		local prs: { [string]: boolean } = {}
		for _, thread in threads do
			if thread.id == state.selected then selected = thread end
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
			ui.row { align = "center", ui.text { "Overview", size = "large", weight = "semibold" }, ui.spacer(),
				data and data.available and data.writer == "shell" and not state.draft and ui.button { label = "New chat", msg = {
					compose = "start", environmentId = data.environmentId, target = if #data.projects == 1 then data.projects[1].id else nil } },
				data and data.available and data.writer == "shell" and selected and not state.draft and ui.button { label = "Reply", msg = {
					compose = "send", target = selected.id, title = selected.title, environmentId = data.environmentId } },
				ui.button { label = "Refresh", msg = "refresh" } },
			ui.row { align = "stretch",
				card { title = "Linked pull requests", grow = true,
					if data and data.available then ui.text { tostring(prCount), size = "headline", weight = "semibold" } else ui.text "Unavailable" },
				card { title = "This machine", grow = true,
					if system then system_load { cpu = system.cpu, memory = system.memory, history = system.cpu_history } else ui.text "Waiting for sample" },
				card { title = "Usage", grow = true, table.unpack(services) },
			},
			ui.row { table.unpack(buttons) },
			state.notice and ui.text(state.notice),
			data and state.draft and composer { data = data, action = state.draft.action, id = state.draft.id, text = state.draft.text,
				environmentId = state.draft.environmentId, target = state.draft.target, title = state.draft.title, picker = state.picker },
			if not data then ui.text "Connecting to T3 Code…"
			elseif not data.available then ui.text(data.error or "T3 Code unavailable")
			elseif #filtered == 0 then ui.text "No matching sessions"
			else ui.column { scroll = true, grow = true, sessions { threads = filtered, folded = state.folded, selected = state.selected } },
			data and data.total > #threads and ui.text { `Showing the newest {#threads} of {data.total} sessions` },
		}
	end,
}
