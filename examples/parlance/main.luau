-- Parlance's shell: a title bar across the top (window controls, history,
-- the sidebar toggle, the open chat's title, Share), the icon rail down the
-- left, and beside it the sidebar and the conversation on one rounded
-- sheet. The chats come from `providers/chats` as `app.chats`.

local MENUS: { { title: string, items: { any } } } = {
	{ title = "File", items = {
		{ label = "New Chat", msg = { forward = { new = true } }, to = "chat", shortcut = "secondary-n" },
		{ label = "Stop Answer", msg = { forward = { stop = true } }, to = "chat", shortcut = "secondary-." },
		{ label = "Regenerate Answer", msg = { forward = { regenerate = true } }, to = "chat", shortcut = "secondary-r" },
	} },
	{ title = "View", items = {
		{ label = "Show or Hide the Sidebar", msg = "toggle-left", shortcut = "secondary-s" },
	} },
	{ title = "Window", items = {
		{ label = "Minimize", command = "minimize", shortcut = "secondary-m" },
		{ label = "Zoom", command = "maximize" },
	} },
}

-- A small icon button in the title bar.
local function bar_button(icon: string, tooltip: string, msg: any, off: boolean?)
	return ui.row {
		class = if off then "pl-bar-button pl-bar-button-off" else "pl-bar-button",
		align = "center",
		tooltip = tooltip,
		msg = if off then nil else msg,
		ui.icon { name = icon :: any },
	}
end

return mod {
	window = { titlebar = "hidden", traffic_lights = { x = 17, y = 15 }, size = { width = 1240, height = 1180 } },

	about = {
		version = "0.1",
		description = "A desktop chat app with a scripted demo agent: rich markdown answers that stream in, chats, projects and recents.",
		website = "https://github.com/penso/moonframe",
		copyright = "MIT licensed",
	},

	menus = MENUS,

	init = function(previous)
		return { left = if previous then previous.left ~= false else true }
	end,

	update = function(state, msg)
		if msg == "toggle-left" then
			state.left = not state.left
		end
	end,

	view = function(state, app)
		local window = app.window
		local mac = window.platform == "macos" and window.titlebar == "hidden" and not window.fullscreen
		local chats = app.chats
		local title = if chats and chats.chat then chats.chat.title else ""
		local body = ui.column {
			class = "pl-frame",
			grow = true,
			gap = "none",
			ui.row {
				class = "pl-titlebar",
				drag = true,
				height = 44,
				shrink = false,
				align = "center",
				gap = "none",
				ui.row {
					width = if state.left then 340 else nil,
					shrink = false,
					align = "center",
					gap = "none",
					ui.row { width = if mac then 84 else 12 },
					ui.row {
						gap = "small",
						align = "center",
						bar_button("arrow-left", "Back", "back", true),
						bar_button("arrow-right", "Forward", "forward", true),
						bar_button("panel-left", "Toggle sidebar", "toggle-left"),
					},
					ui.spacer(),
					state.left and ui.row { class = "pl-bar-rule" },
				},
				ui.text { title, class = "pl-title", truncate = true },
				ui.spacer(),
				ui.row {
					align = "center",
					gap = "small",
					ui.row {
						class = "pl-bar-button",
						align = "center",
						tooltip = "Share this chat",
						copy = title,
						ui.icon "upload",
						ui.text { "Share", class = "pl-bar-label" },
					},
					bar_button("ellipsis", "More", "more"),
					ui.row { class = "pl-bar-rule", height = 18 },
					ui.row {
						class = "pl-bar-button",
						align = "center",
						tooltip = "New chat",
						msg = { new = true },
						to = "provider:chats",
						ui.row {
							class = "pl-box-plus",
							width = 16,
							height = 16,
							align = "center",
							justify = "center",
							ui.icon { name = "plus", size = "small" },
						},
					},
				},
			},
			ui.row {
				grow = true,
				gap = "none",
				ui.mount "rail",
				ui.column {
					grow = true,
					gap = "none",
					style = { margin = { 0, 3, 3, 0 } },
					ui.hsplit {
						id = "body",
						divider = false,
						ui.pane { size = 288, min = 220, max = 420, fixed = true, visible = state.left, ui.mount "sidebar" },
						ui.pane { min = 420, ui.mount "chat" },
					},
				},
			},
		}
		-- Send feedback (the built-in mods/feedback): ask an agent to change this app.
		return ui.column { grow = true, gap = "none", body, ui.row { class = "feedback-corner", float = { at = "bottom-right", inset = 20 }, ui.mount "feedback" } }
	end,
}
