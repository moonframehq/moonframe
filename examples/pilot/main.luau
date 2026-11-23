-- Pilot's shell: the sidebar, the page in the middle, and the workbench beside
-- a conversation. It keeps which page shows, tells the mods on the topic
-- `pilot/page`, and turns the events `mods/watch` passes on into notifications.

local toasts = use "toasts"

local MENUS: { { title: string, items: { any } } } = {
	{ title = "File", items = {
		{ label = "New Thread", msg = { page = "new" }, shortcut = "secondary-n" },
		{ label = "Add Project Folder…", msg = "add-folder", to = "sidebar", shortcut = "secondary-o" },
		"-",
		{ label = "Search…", msg = { open = "all" }, to = "palette", shortcut = "secondary-k" },
		{ label = "Go to File…", msg = { open = "files" }, to = "palette", shortcut = "secondary-p" },
		{ label = "Switch Model…", msg = { open = "models" }, to = "palette", shortcut = "secondary-shift-m" },
	} },
	{ title = "Thread", items = {
		{ label = "Find in Thread", msg = "find", to = "conversation", shortcut = "secondary-f" },
		{ label = "Stop", msg = { abort = true }, to = "provider:pi", shortcut = "secondary-." },
		"-",
		{ label = "Next Recent Thread", msg = "next-recent", to = "sidebar", shortcut = "ctrl-tab" },
		{ label = "Previous Recent Thread", msg = "previous-recent", to = "sidebar", shortcut = "ctrl-shift-tab" },
		"-",
		{ label = "Thread 1", msg = { jump = 1 }, to = "sidebar", shortcut = "secondary-1" },
		{ label = "Thread 2", msg = { jump = 2 }, to = "sidebar", shortcut = "secondary-2" },
		{ label = "Thread 3", msg = { jump = 3 }, to = "sidebar", shortcut = "secondary-3" },
		{ label = "Thread 4", msg = { jump = 4 }, to = "sidebar", shortcut = "secondary-4" },
		{ label = "Thread 5", msg = { jump = 5 }, to = "sidebar", shortcut = "secondary-5" },
		{ label = "Thread 6", msg = { jump = 6 }, to = "sidebar", shortcut = "secondary-6" },
		{ label = "Thread 7", msg = { jump = 7 }, to = "sidebar", shortcut = "secondary-7" },
		{ label = "Thread 8", msg = { jump = 8 }, to = "sidebar", shortcut = "secondary-8" },
		{ label = "Thread 9", msg = { jump = 9 }, to = "sidebar", shortcut = "secondary-9" },
	} },
	{ title = "View", items = {
		{ label = "Toggle Sidebar", msg = "toggle-sidebar", shortcut = "secondary-b" },
		{ label = "Toggle Workbench", msg = "toggle-workbench", shortcut = "secondary-alt-b" },
		{ label = "Terminal", msg = { toggle_tab = "terminal" }, to = "workbench", shortcut = "secondary-j" },
		{ label = "Changes", msg = { toggle_tab = "changes" }, to = "workbench", shortcut = "secondary-r" },
		{ label = "Files", msg = { toggle_tab = "files" }, to = "workbench", shortcut = "secondary-e" },
		"-",
		{ label = "Settings…", msg = { page = "settings" }, shortcut = "secondary-," },
	} },
	{ title = "Window", items = {
		{ label = "Minimize", command = "minimize", shortcut = "secondary-m" },
		{ label = "Zoom", command = "maximize" },
	} },
}

type State = {
	page: string,
	sidebar: boolean,
	workbench: boolean,
	toasts: { any },
}

-- Whether the window is in front, for `update`, which has no `app`.
local window_active = true

local function open_page(state: State, page: string): any
	state.page = page
	return { to = "topic:pilot/page", msg = page }
end

-- What a new event in `app.pi.events` asks for.
local function notice(state: State, event: any, pi: any, app_active: boolean): { any }
	local sends: { any } = {}
	if event.kind == "toast" then
		table.insert(state.toasts, {
			id = event.seq,
			level = if event.level == "error" then "error" else (if event.level == "warning" then "warning" else "info"),
			title = event.title,
			body = event.body,
		})
		while #state.toasts > 4 do
			table.remove(state.toasts, 1)
		end
		return sends
	end
	-- Only what the user is not already looking at.
	local watching = app_active and state.page == "threads" and pi.selected == event.thread
	if watching or not pi.prefs.notifications then
		return sends
	end
	local words = if event.kind == "failed" then "Failed" elseif event.kind == "waiting" then "Needs you" else "Finished"
	-- A thread's latest news takes the place of what it said before.
	table.insert(sends, {
		to = "app:notify",
		msg = { title = `{words}: {event.title}`, body = event.body, id = event.thread, replace = event.thread ~= nil },
	})
	if pi.prefs.sounds then
		table.insert(sends, { to = "app:sound", msg = "default" })
	end
	return sends
end

return mod {
	window = { titlebar = "hidden", traffic_lights = { x = 18, y = 22 }, size = { width = 1520, height = 960 } },

	about = {
		version = "0.1",
		description = "A desktop app for the pi coding agent: threads, review, a terminal and files in one window.",
		website = "https://github.com/moonframehq/moonframe",
		copyright = "MIT licensed",
	},

	menus = MENUS,
	subscribe = { "app/notification" },

	init = function(previous: State?): (State, { any })
		local state: State = previous or { page = "threads", sidebar = true, workbench = true, toasts = {} }
		state.toasts = {}
		return state, { { to = "topic:pilot/page", msg = state.page } }
	end,

	update = function(state: State, msg: any, _props: any)
		if msg == "toggle-sidebar" then
			state.sidebar = not state.sidebar
		elseif msg == "toggle-workbench" then
			state.workbench = not state.workbench
		elseif msg == "show-workbench" then
			state.workbench = true
		elseif type(msg) ~= "table" then
			return
		elseif msg.page then
			return state, { open_page(state, msg.page) }
		elseif msg.dismiss then
			for index, toast in state.toasts do
				if toast.id == msg.dismiss then
					table.remove(state.toasts, index)
					break
				end
			end
		elseif msg.topic == "app/notification" then
			if msg.value and msg.value.clicked and msg.value.id then
				return state, { open_page(state, "threads"), { to = "provider:pi", msg = { select = msg.value.id } } }
			end
		elseif msg.events then
			local sends: { any } = {}
			for _, event in msg.events do
				for _, send in notice(state, event, msg, window_active) do
					table.insert(sends, send)
				end
			end
			return state, sends
		end
	end,

	view = function(state: State, app: any)
		window_active = app.window.active
		local thread_page = state.page == "threads"
		-- Pages are instances, so one not showing doesn't run: a mod's own view
		-- runs whenever what it reads changes, drawn or not, and pi publishes
		-- on every streamed word.
		local page: any = if thread_page
			then ui.mount "conversation"
			elseif state.page == "new" then ui.mount { "start", id = "page", props = {} }
			elseif state.page == "scheduled" then ui.mount { "scheduled", id = "page", props = {} }
			elseif state.page == "settings" then ui.mount { "settings", id = "page", props = {} }
			else ui.mount { "resources", id = "page", props = { kind = state.page } }
		return ui.column {
			grow = true,
			gap = "none",
			ui.hsplit {
				id = "body",
				ui.pane { size = 300, min = 230, max = 440, fixed = true, visible = state.sidebar, surface = "sidebar", ui.mount "sidebar" },
				ui.pane { min = 420, page },
				ui.pane { size = 620, min = 340, max = 1000, fixed = true, lazy = true, visible = state.workbench and thread_page, ui.mount "workbench" },
			},
			ui.mount "palette",
			ui.mount "watch",
			toasts { items = state.toasts, place = "top-end" },
			ui.row { class = "feedback-corner", float = { at = "bottom-left", inset = 14 }, ui.mount "feedback" },
		}
	end,
}
