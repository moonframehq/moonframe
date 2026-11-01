-- The bar across the top: the sidebar toggle, new, back and forward through
-- the views, and one field to search or create. Typing filters the day
-- (the agenda shows what matches); Enter adds what was typed as a task.

local icon_button = use "icon-button"
local nav_buttons = use "nav-buttons"
local kbd = use "kbd"

type State = { query: string, focus: boolean }

return mod {
	region = "top",
	title = "Top bar",

	styles = {
		[".topbar"] = { padding = { 0, 14, 0, 0 }, gap = 6 },
		[".topbar-button"] = { radius = 18 },
		[".topbar-dark"] = { background = "#ffffff14" },
		[".topbar-pale"] = { background = "#00000008" },
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
		return { query = previous and previous.query or "", focus = false }
	end,

	update = function(state: State, msg)
		if msg == "focus" then
			state.focus = true
			return
		end
		if type(msg) ~= "table" then
			return
		end
		if msg.msg == "query" then
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
		local function button(icon: string, tooltip: string, msg: any, to: string?)
			return ui.row {
				class = { "topbar-button", `topbar-{tint}` },
				icon_button { icon = icon, tooltip = tooltip, msg = msg, to = to },
			}
		end
		return ui.row {
			class = "topbar",
			drag = true,
			grow = true,
			height = 60,
			align = "center",
			inset and ui.row { width = 82 },
			button("panel-left", "Show or hide the sidebar", "toggle-left", "shell"),
			button("pencil", "New task", "focus"),
			ui.row { class = { "nav-wrap", `nav-{tint}` }, nav_buttons { back = "back", forward = "forward", to = "sidebar" } },
			ui.spacer(),
			ui.row {
				class = { "omni", `omni-{tint}` },
				width = 580,
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
			ui.spacer(),
			ui.row { width = 160 },
		}
	end,
}
