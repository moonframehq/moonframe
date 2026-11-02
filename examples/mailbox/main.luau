-- Mailbox's shell: a title bar with the sidebar toggle, then the sidebar on
-- the window's tint and the folder's messages on a white sheet beside it.
-- The sidebar is a row's child, not a split pane, so its width can ease
-- between full and rail as `side-nav`'s states say.

local MENUS: { { title: string, items: { any } } } = {
	{ title = "View", items = {
		{ label = "Fold or Unfold the Sidebar", msg = "fold", to = "sidebar", shortcut = "secondary-s" },
	} },
	{ title = "Window", items = {
		{ label = "Minimize", command = "minimize", shortcut = "secondary-m" },
		{ label = "Zoom", command = "maximize" },
	} },
}

return mod {
	window = { titlebar = "hidden", traffic_lights = { x = 16, y = 14 }, size = { width = 1180, height = 780 } },

	about = {
		version = "0.1",
		description = "A sidebar that folds to an icon rail, with a staggered fade. Shows sample data.",
		website = "https://github.com/moonframehq/moonframe",
		copyright = "MIT licensed",
	},

	menus = MENUS,

	view = function(state, app)
		local window = app.window
		local lights = window.platform == "macos" and window.titlebar == "hidden" and not window.fullscreen
		local body = ui.column {
			class = "frame",
			grow = true,
			gap = "none",
			ui.row {
				class = "titlebar",
				drag = true,
				height = 40,
				shrink = false,
				align = "center",
				lights and ui.row { width = 76 },
				-- The toggle in the title bar, as in the reference: it stays
				-- where it is whichever way the sidebar is.
				ui.row {
					class = "titlebar-button",
					msg = "fold",
					to = "sidebar",
					tooltip = "Fold sidebar (⌘S)",
					ui.icon "panel-left",
				},
			},
			ui.row {
				grow = true,
				gap = "none",
				ui.mount "sidebar",
				ui.column { class = "sheet-pane", grow = true, ui.column { class = "sheet", grow = true, ui.mount "inbox" } },
			},
		}
		-- Send feedback (the built-in mods/feedback): ask an agent to change this app.
		return ui.column { grow = true, gap = "none", body, ui.row { class = "feedback-corner", float = { at = "bottom-right", inset = 20 }, ui.mount "feedback" } }
	end,
}
