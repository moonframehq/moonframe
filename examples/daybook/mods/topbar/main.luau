-- The bar across the top: sidebar toggle, quick-create menu, back and forward,
-- centered search/create omnibox, and right-side mode buttons (Inbox, AI
-- Assistant, and right-pane toggle).

local icon_button = use "icon-button"
local menu = use "menu"
local nav_buttons = use "nav-buttons"
local kbd = use "kbd"

type State = {
	query: string,
	focus: boolean,
	create_open: boolean,
	right_mode: string,
}

return mod {
	region = "top",
	title = "Top bar",

	styles = {
		[".topbar"] = { gap = 6, padding = { 0, 12, 0, 0 } },
		-- Two equal sides keep the field centered on the whole bar.
		[".topbar-side"] = { gap = 6 },
		[".topbar-button"] = { radius = 18 },
		[".topbar-dark"] = { background = "#ffffff14" },
		[".topbar-pale"] = { background = "#00000008" },
		[".topbar-dark:selected"] = { background = "#ffffff2e" },
		[".topbar-pale:selected"] = { background = "#00000018" },
		[".omni"] = { radius = 20, padding = { 0, 12 }, gap = 8 },
		[".omni-dark"] = { background = "#ffffff12" },
		[".omni-pale"] = { background = "#0000000a" },
		[".omni-icon"] = { color = "muted-foreground" },
		-- A class outranks the app's `input` rule: no frame, no ring.
		[".bare-input"] = { background = "#00000000", border = "none", shadow = "none", padding = { 6, 0 } },
		[".nav-wrap"] = { radius = 18 },
		[".nav-dark"] = { background = "#ffffff14" },
		[".nav-pale"] = { background = "#0000000a" },
		-- The capsule's own card would glare on a dark frame.
		[".nav-buttons"] = { background = "#00000000", border = "none", shadow = "none" },
	},

	init = function(previous: State?): State
		return {
			query = previous and previous.query or "",
			focus = false,
			create_open = false,
			right_mode = previous and previous.right_mode or "inbox",
		}
	end,

	update = function(state: State, msg)
		if msg == "focus" then
			state.focus = true
			state.create_open = false
			return
		elseif msg == "toggle-create" then
			state.create_open = not state.create_open
			return
		elseif msg == "close-create" then
			state.create_open = false
			return
		elseif msg == "open-inbox" then
			state.right_mode = "inbox"
			return state, {
				{ to = "shell", msg = "show-right" },
				{ to = "inbox", msg = { mode = "inbox" } },
			}
		elseif msg == "open-assistant" then
			state.right_mode = "assistant"
			return state, {
				{ to = "shell", msg = "show-right" },
				{ to = "inbox", msg = { mode = "assistant" } },
			}
		end
		if type(msg) ~= "table" then
			return
		end
		if msg.right_mode then
			state.right_mode = msg.right_mode
		elseif msg.pick then
			state.create_open = false
			if msg.pick == "task" then
				state.focus = true
			elseif msg.pick == "note" then
				return state, { { to = "agenda", msg = "new-note" } }
			elseif msg.pick == "event" then
				return state, { { to = "sidebar", msg = { view = "events" } } }
			elseif msg.pick == "habit" then
				return state, { { to = "sidebar", msg = { view = "habits" } } }
			end
		elseif msg.msg == "query" then
			state.query = msg.value
			state.focus = false
			return state, { { to = "agenda", msg = { query = msg.value } } }
		elseif msg.msg == "create" then
			local title = msg.value
			state.query = ""
			state.focus = false
			if string.match(title, "%S") then
				return state, {
					{ to = "provider:planner", msg = { add = title } },
					{ to = "agenda", msg = { query = "" } },
				}
			end
		elseif msg.msg == "clear" then
			state.query = ""
			state.focus = false
			return state, { { to = "agenda", msg = { query = "" } } }
		end
	end,

	view = function(state: State, app)
		local planner = app.planner
		local look = (planner and planner.look) or "classic"
		local dark = app.dark or look ~= "light"
		local tint = if dark then "dark" else "pale"
		local window = app.window
		local inset = window.platform == "macos" and not window.fullscreen
		local function button(icon: string, tooltip: string, msg: any, to: string?, selected: boolean?)
			return ui.row {
				class = { "topbar-button", `topbar-{tint}` },
				selected = selected,
				icon_button { icon = icon, tooltip = tooltip, msg = msg, to = to },
			}
		end
		local create_items: { any } = {
			{ heading = "Create" },
			{ id = "task", label = "New task", icon = "square-check", shortcut = "cmd-n" },
			{ id = "note", label = "New note", icon = "file-text", shortcut = "cmd-shift-n" },
			{ id = "event", label = "New event", icon = "calendar", shortcut = "cmd-shift-e" },
			{ id = "habit", label = "New habit", icon = "zap" },
		}
		return ui.row {
			class = "topbar",
			drag = true,
			grow = true,
			height = 60,
			align = "center",
			ui.row {
				class = "topbar-side",
				width = 0,
				grow = true,
				align = "center",
				inset and ui.row { width = 82 },
				button("panel-left", "Show or hide the sidebar", "toggle-left", "shell"),
				ui.popover {
					open = state.create_open,
					dismiss = "close-create",
					side = "bottom",
					align = "start",
					width = 210,
					content = menu { items = create_items },
					button("square-pen", "Create task, note, event or habit", "toggle-create"),
				},
				ui.row { class = { "nav-wrap", `nav-{tint}` }, nav_buttons { back = "back", forward = "forward", to = "sidebar" } },
			},
			ui.row {
				class = { "omni", `omni-{tint}` },
				width = 520,
				height = 40,
				align = "center",
				ui.icon { name = "search", class = "omni-icon" },
				ui.input {
					id = "omni",
					value = state.query,
					placeholder = "Search, or create anything…",
					change = "query",
					submit = "create",
					keys = { escape = "clear" },
					focus = state.focus,
					grow = true,
					class = "bare-input",
				},
				kbd { keys = "cmd-k", small = true, plain = true },
			},
			ui.row {
				class = "topbar-side",
				width = 0,
				grow = true,
				align = "center",
				justify = "end",
				button("inbox", "Inbox", "open-inbox", nil, state.right_mode == "inbox"),
				button("sparkles", "AI Assistant", "open-assistant", nil, state.right_mode == "assistant"),
				button("panel-right", "Show or hide the right pane", "toggle-right", "shell"),
			},
		}
	end,
}
