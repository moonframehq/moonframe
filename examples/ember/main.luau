-- Ember's shell: a charcoal sidebar (with the traffic lights, the sidebar
-- toggle, back and forward and the chat / code switch over it) beside the
-- canvas, whose own title row holds the session's title and the tools on
-- the right. The terminal folds out under the canvas, a preview page beside
-- it.
--
-- The shell owns where you are: the mode, the session open ("" for a new
-- one) and the history behind back and forward. It shares them with the
-- sidebar and the canvas on the `ember/nav` topic, and tells the history
-- provider which transcript to read.

local menu = use "menu"
local release_notes = use "release-notes"
local browser = use "browser"

type State = {
	left: boolean,
	mode: string,
	open: string,
	back: { string },
	forward: { string },
	terminal: boolean,
	preview: boolean,
	popup: string,
	url: string,
	page: any,
}

local MENUS: { { title: string, items: { any } } } = {
	{ title = "Session", items = {
		{ label = "New Session", msg = "new", shortcut = "secondary-n" },
		{ label = "Search Sessions…", msg = "search", to = "sidebar", shortcut = "secondary-k" },
		"-",
		{ label = "Back", msg = "back", shortcut = "secondary-[" },
		{ label = "Forward", msg = "forward", shortcut = "secondary-]" },
		"-",
		{ label = "Look for New Sessions", msg = { refresh = true }, to = "provider:history", shortcut = "secondary-r" },
	} },
	{ title = "View", items = {
		{ label = "Code", msg = { mode = "code" }, shortcut = "secondary-1" },
		{ label = "Chat", msg = { mode = "chat" }, shortcut = "secondary-2" },
		"-",
		{ label = "Show or Hide the Sidebar", msg = "toggle-left", shortcut = "secondary-s" },
		{ label = "Show or Hide the Terminal", msg = "toggle-terminal", shortcut = "secondary-j" },
		{ label = "Show or Hide the Preview", msg = "toggle-preview", shortcut = "secondary-p" },
	} },
	{ title = "Window", items = {
		{ label = "Minimize", command = "minimize", shortcut = "secondary-m" },
		{ label = "Zoom", command = "maximize" },
	} },
}

local MORE: { any } = {
	{ id = "new", label = "New session", icon = "plus", shortcut = "cmd-n" },
	{ id = "sidebar", label = "Show or hide the sidebar", icon = "panel-left", shortcut = "cmd-s" },
	{ id = "terminal", label = "Terminal", icon = "terminal", shortcut = "cmd-j" },
	{ id = "preview", label = "Preview", icon = "globe", shortcut = "cmd-p" },
	"separator",
	{ id = "refresh", label = "Look for new sessions", icon = "refresh-cw", shortcut = "cmd-r" },
}

local HOME = "https://github.com/penso/moonframe"

-- What the sidebar and the canvas hear, and what the provider reads.
local function share(state: State): { any }
	return {
		{ to = "topic:ember/nav", msg = { mode = state.mode, open = state.open } },
		{ to = "provider:history", msg = if state.open ~= "" then { open = state.open } else { close = true } },
	}
end

local function go(state: State, open: string)
	if open == state.open then
		return
	end
	table.insert(state.back, state.open)
	if #state.back > 50 then
		table.remove(state.back, 1)
	end
	state.forward = {}
	state.open = open
end

-- The session's title and folder, from what the provider shares.
local function find(history: any, id: string): (string?, string?, boolean)
	if not history or id == "" then
		return nil, nil, false
	end
	for _, group in history.groups or {} do
		for _, session in group.sessions do
			if session.id == id then
				return session.title, group.path, session.gone
			end
		end
	end
	return nil, nil, false
end

-- A title-bar button: an icon, quiet until hovered.
local function tool(icon: string, msg: any, tooltip: string, off: boolean?)
	return ui.row {
		class = if off then "em-tool em-tool-off" else "em-tool",
		msg = if off then nil else msg,
		tooltip = tooltip,
		ui.icon { name = icon :: any },
	}
end

-- Sidebar toggle, back and forward: over the sidebar, or at the canvas's
-- left edge while the sidebar is folded.
local function controls(state: State)
	return ui.row {
		align = "center",
		gap = "none",
		shrink = false,
		tool("panel-left", "toggle-left", "Show or hide the sidebar"),
		ui.row { width = 4 },
		tool("chevron-left", "back", "Back", #state.back == 0),
		ui.row { width = 4 },
		tool("chevron-right", "forward", "Forward", #state.forward == 0),
	}
end

local function modes(state: State)
	return ui.row {
		class = "em-modes",
		align = "center",
		shrink = false,
		ui.row { class = "em-mode", selected = state.mode == "chat", msg = { mode = "chat" }, tooltip = "Chat", ui.icon "message-square" },
		ui.row { class = "em-mode", selected = state.mode == "code", msg = { mode = "code" }, tooltip = "Code", ui.icon "code" },
	}
end

return mod {
	window = { titlebar = "hidden", traffic_lights = { x = 17, y = 16 }, size = { width = 1500, height = 905 } },

	about = {
		version = "0.1",
		description = "Your coding-agent sessions by repository, their transcripts, and a scripted demo agent.",
		website = "https://github.com/penso/moonframe",
		copyright = "MIT licensed",
	},

	menus = MENUS,

	styles = {
		[".em-titlebar-side"] = { padding = { 0, 10, 0, 12 } },
		[".em-titlebar-main"] = { padding = { 0, 14, 0, 30 } },
		[".em-bottom"] = { background = "background", border = "top", border_color = "em-line", padding = { 6, 8, 8, 8 } },
		[".em-preview"] = { background = "background", border = "left", border_color = "em-line" },
		[".em-whatsnew"] = { padding = 10 },
	},

	init = function(previous: State?)
		local state: State = {
			left = true,
			mode = "code",
			open = "",
			back = {},
			forward = {},
			terminal = false,
			preview = false,
			popup = "",
			url = HOME,
			page = nil,
		}
		if previous then
			state.left = previous.left ~= false
			state.mode = previous.mode or "code"
			state.open = previous.open or ""
			state.terminal = previous.terminal == true
			state.preview = previous.preview == true
			state.url = previous.url or HOME
		end
		return state, share(state)
	end,

	update = function(state: State, msg)
		local moved = false
		if msg == "toggle-left" then
			state.left = not state.left
		elseif msg == "toggle-terminal" then
			state.terminal = not state.terminal
		elseif msg == "toggle-preview" then
			state.preview = not state.preview
		elseif msg == "new" then
			go(state, "")
			local sends = share(state)
			table.insert(sends, { to = "canvas", msg = { msg = "clear" } })
			return state, sends
		elseif msg == "back" and #state.back > 0 then
			table.insert(state.forward, state.open)
			state.open = table.remove(state.back) or ""
			moved = true
		elseif msg == "forward" and #state.forward > 0 then
			table.insert(state.back, state.open)
			state.open = table.remove(state.forward) or ""
			moved = true
		elseif msg == "whats-new" or msg == "more" then
			state.popup = if state.popup == msg then "" else msg
		elseif msg == "close-popup" then
			state.popup = ""
		elseif type(msg) == "table" then
			if msg.mode then
				state.mode = msg.mode
				if msg.mode == "chat" then
					go(state, "")
				end
				moved = true
			elseif msg.open then
				state.mode = "code"
				go(state, msg.open)
				moved = true
			elseif msg.navigated then
				state.page = msg
			elseif msg.pick then
				state.popup = ""
				local pick = msg.pick
				if pick == "new" then
					go(state, "")
					moved = true
				elseif pick == "sidebar" then
					state.left = not state.left
				elseif pick == "terminal" then
					state.terminal = not state.terminal
				elseif pick == "preview" then
					state.preview = not state.preview
				elseif pick == "refresh" then
					return state, { { to = "provider:history", msg = { refresh = true } } }
				end
			end
		end
		if moved then
			return state, share(state)
		end
	end,

	view = function(state: State, app)
		local window = app.window
		local lights = window.platform == "macos" and window.titlebar == "hidden" and not window.fullscreen
		local title, folder, gone = find(app.history, state.open)
		if state.open ~= "" and not title then
			title = "Session"
		end
		local heading: string = title or (if state.mode == "chat" then "New chat" else "New session")

		local side_bar = ui.row {
			class = "em-titlebar em-titlebar-side",
			height = 44,
			shrink = false,
			drag = true,
			align = "center",
			lights and ui.row { width = 68 },
			controls(state),
			ui.spacer(),
			modes(state),
		}

		local main_bar = ui.row {
			class = "em-titlebar em-titlebar-main",
			height = 44,
			shrink = false,
			drag = true,
			align = "center",
			not state.left and lights and ui.row { width = 50 },
			not state.left and controls(state),
			not state.left and modes(state),
			not state.left and ui.row { width = 18 },
			ui.text { heading, class = "em-title", truncate = true },
			ui.spacer(),
			ui.popover {
				open = state.popup == "whats-new",
				dismiss = "close-popup",
				side = "bottom",
				align = "end",
				width = 440,
				content = ui.column {
					class = "em-whatsnew",
					release_notes {
						name = "Ember",
						version = "0.1",
						badge = "Demo",
						date = "October 31, 2026",
						summary = "Your coding-agent sessions, by repository, in one window.",
						highlights = {
							{ icon = "history", title = "Your CLI sessions", detail = "Read from ~/.claude/projects, newest first, grouped by repository. Nothing is written." },
							{ icon = "message-square", title = "Transcripts", detail = "Open one to read it; tool calls fold into a line." },
							{ icon = "sparkles", title = "A scripted demo agent", detail = "New sessions answer from a script: no model is called." },
						},
					},
				},
				ui.row { class = "em-link", msg = "whats-new", ui.text "What’s new" },
			},
			ui.row { width = 2 },
			tool("terminal", "toggle-terminal", "Terminal"),
			tool("globe", "toggle-preview", "Preview"),
			ui.popover {
				open = state.popup == "more",
				dismiss = "close-popup",
				side = "bottom",
				align = "end",
				width = 260,
				content = menu { items = MORE },
				tool("ellipsis", "more", "More"),
			},
		}

		local cwd = if folder and folder ~= "" and not gone then folder else "~"

		return ui.hsplit {
			id = "body",
			divider = false,
			ui.pane {
				size = 288,
				min = 220,
				max = 440,
				fixed = true,
				visible = state.left,
				class = "em-side",
				ui.column { grow = true, gap = "none", side_bar, ui.mount "sidebar" },
			},
			ui.pane {
				class = "em-main",
				ui.column {
					grow = true,
					gap = "none",
					main_bar,
					ui.vsplit {
						id = "work",
						divider = false,
						ui.pane {
							ui.hsplit {
								id = "canvas",
								divider = false,
								ui.pane { min = 420, ui.mount "canvas" },
								ui.pane {
									size = 460,
									min = 300,
									max = 900,
									fixed = true,
									visible = state.preview,
									lazy = true,
									class = "em-preview",
									browser { id = "preview", url = state.url, page = state.page, navigate = { navigated = true } },
								},
							},
						},
						ui.pane {
							size = 240,
							min = 120,
							max = 600,
							fixed = true,
							visible = state.terminal,
							lazy = true,
							class = "em-bottom",
							ui.terminal { id = "shell", cwd = cwd },
						},
					},
				},
			},
		}
	end,
}
