-- Scheduled tasks: a prompt pi runs as a new thread at set times, while the
-- app is open.

local dialog = use "dialog"
local dropdown = use "dropdown"
local empty_state = use "empty-state"
local segmented = use "segmented"
local switch = use "switch"

type Draft = { id: string?, name: string, prompt: string, workspace: string?, cadence: string, time: string, weekday: number, worktree: boolean }
type State = { draft: Draft?, workspace_open: boolean, weekday_open: boolean, problem: string? }

local WEEKDAYS = { "Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday" }

-- The view's data, for `update`, which has no `app`.
local latest: any = nil

local function blank(): Draft
	local first = latest and (latest.workspaces[1] or latest.chats)
	return { name = "", prompt = "", workspace = first and first.path, cadence = "daily", time = "09:00", weekday = 1, worktree = false }
end

local function when_next(pi: any, task: any): string
	if not task.enabled then
		return "Paused"
	end
	if not task.next then
		return task.when
	end
	local seconds = task.next - pi.now
	local away = if seconds < 3600 then `{math.max(1, seconds // 60)}m` elseif seconds < 86400 then `{seconds // 3600}h` else `{seconds // 86400}d`
	return `{task.when} · next in {away}`
end

local function task_card(pi: any, task: any): any
	return ui.column {
		class = "page-card",
		ui.row {
			align = "center",
			gap = "normal",
			ui.icon { "clock", tone = "muted" },
			ui.column {
				grow = true,
				gap = "none",
				ui.text { task.name, weight = "medium", truncate = true },
				ui.text { `{task.workspace_name} · {when_next(pi, task)}`, tone = "muted", size = "small", truncate = true },
			},
			ui.button { "Run now", small = true, msg = { run_schedule = task.id }, to = "provider:pi" },
			ui.button { "Edit", small = true, variant = "ghost", msg = { edit = task.id } },
			ui.button { icon = "trash-2", small = true, variant = "ghost", msg = { delete_schedule = task.id }, to = "provider:pi" },
			switch { on = task.enabled, msg = { toggle = task.id, on = not task.enabled } },
		},
		ui.text { task.prompt, tone = "muted", lines = 2 },
		task.last_thread and ui.row { ui.button { "Open last run", small = true, variant = "ghost", msg = { open_run = task.last_thread } } },
	}
end

return mod {
	init = function(): State
		return { workspace_open = false, weekday_open = false }
	end,

	update = function(state: State, msg: any)
		local draft = state.draft
		if msg == "new" then
			state.draft = blank()
			state.problem = nil
		elseif msg == "cancel" then
			state.draft = nil
		elseif msg == "toggle-workspace" then
			state.workspace_open = not state.workspace_open
		elseif msg == "close-workspace" then
			state.workspace_open = false
		elseif msg == "toggle-weekday" then
			state.weekday_open = not state.weekday_open
		elseif msg == "close-weekday" then
			state.weekday_open = false
		elseif msg == "toggle-worktree" and draft then
			draft.worktree = not draft.worktree
		elseif msg == "save" and draft then
			if draft.name == "" or draft.prompt == "" or not draft.workspace then
				state.problem = "A task needs a name, a prompt and a project."
				return
			elseif not string.match(draft.time, "^%d%d?:%d%d$") then
				state.problem = "Write the time as 09:30."
				return
			end
			state.draft = nil
			return state, { { to = "provider:pi", msg = { schedule = draft } } }
		elseif type(msg) ~= "table" then
			return
		elseif msg.toggle then
			return state, { { to = "provider:pi", msg = { schedule = { id = msg.toggle, enabled = msg.on } } } }
		elseif msg.open_run then
			local sends: { any } = { { to = "shell", msg = { page = "threads" } }, { to = "provider:pi", msg = { select = msg.open_run } } }
			return state, sends
		elseif msg.edit then
			for _, task in latest.schedules do
				if task.id == msg.edit then
					state.draft = { id = task.id, name = task.name, prompt = task.prompt, workspace = task.workspace, cadence = task.cadence, time = task.time, weekday = task.weekday, worktree = task.worktree }
				end
			end
		elseif draft then
			if msg.msg == "name" then
				draft.name = msg.value
			elseif msg.msg == "prompt" then
				draft.prompt = msg.value
			elseif msg.msg == "time" then
				draft.time = msg.value
			elseif msg.cadence then
				draft.cadence = msg.cadence
			elseif msg.workspace then
				state.workspace_open = false
				draft.workspace = msg.workspace
			elseif msg.weekday then
				state.weekday_open = false
				draft.weekday = tonumber(msg.weekday) or 1
			end
		end
	end,

	view = function(state: State, app: any, props: any)
		if props == nil then
			return ui.column { width = 0, height = 0 }
		end
		local pi = app.pi
		latest = pi
		local cards: { any } = {}
		for _, task in (pi and pi.schedules) or {} do
			table.insert(cards, task_card(pi, task))
		end
		local draft = state.draft
		local workspaces: { any } = {}
		local workspace_name = "Choose a project"
		for _, workspace in (pi and pi.workspaces) or {} do
			table.insert(workspaces, { id = workspace.path, label = workspace.name, detail = workspace.short })
			if draft and workspace.path == draft.workspace then
				workspace_name = workspace.name
			end
		end
		if pi then
			table.insert(workspaces, { id = pi.chats.path, label = pi.chats.name, detail = "Pilot's own empty folder" })
			if draft and pi.chats.path == draft.workspace then
				workspace_name = pi.chats.name
			end
		end
		local weekdays: { any } = {}
		for index, name in WEEKDAYS do
			table.insert(weekdays, { id = tostring(index - 1), label = name })
		end
		return ui.column {
			grow = true,
			gap = "none",
			ui.row { drag = true, height = 52, shrink = false },
			ui.column {
				grow = true,
				scroll = true,
				gap = "none",
				ui.column {
					class = "page",
					shrink = false,
					ui.row {
						align = "center",
						ui.column { grow = true, gap = "none", ui.text { "Scheduled", class = "page-title" }, ui.text { "Prompts pi runs by itself as new threads, while this app is open.", tone = "muted" } },
						ui.button { "New task", icon = "plus", variant = "primary", msg = "new" },
					},
					#cards == 0 and empty_state { icon = "clock", title = "Nothing scheduled", message = "A weekly dependency check, a nightly summary of what changed.", action = "New task", msg = "new" },
					table.unpack(cards),
				},
			},
			dialog {
				open = draft ~= nil,
				dismiss = "cancel",
				title = if draft and draft.id then "Edit task" else "New task",
				size = "medium",
				note = state.problem,
				submit = "save",
				buttons = { { label = "Cancel", msg = "cancel", variant = "ghost" }, { label = "Save", msg = "save", variant = "primary" } },
				draft and ui.column {
					gap = "normal",
					ui.row { class = "page-field", ui.input { id = "task-name", grow = true, focus = true, value = draft.name, placeholder = "Name", change = "name" } },
					ui.row { class = "page-field", ui.input { id = "task-prompt", grow = true, multiline = true, rows = 4, value = draft.prompt, placeholder = "What should pi do?", change = "prompt" } },
					ui.row {
						align = "center",
						dropdown { icon = "folder", value = workspace_name, open = state.workspace_open, toggle = "toggle-workspace", dismiss = "close-workspace", options = workspaces, event = "workspace", selected = draft.workspace, width = 300 },
						ui.spacer(),
						switch { label = "In a new worktree", on = draft.worktree, msg = "toggle-worktree" },
					},
					ui.row {
						align = "center",
						segmented {
							selected = draft.cadence,
							event = "cadence",
							options = { { id = "hourly", label = "Hourly" }, { id = "daily", label = "Daily" }, { id = "weekdays", label = "Weekdays" }, { id = "weekly", label = "Weekly" } },
						},
						draft.cadence == "weekly" and dropdown { value = WEEKDAYS[draft.weekday + 1], open = state.weekday_open, toggle = "toggle-weekday", dismiss = "close-weekday", options = weekdays, event = "weekday", width = 160 },
						ui.spacer(),
						ui.text { "at", tone = "muted" },
						ui.row { class = "page-field", width = 76, ui.input { id = "task-time", grow = true, value = draft.time, change = "time" } },
					},
				},
			},
		}
	end,
}
