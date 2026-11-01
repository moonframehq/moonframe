-- The shell: lays out the whole window. Mods fill the regions it draws.
-- Like an editor or a terminal app, it has no system title bar: its top row
-- is the title bar, with room for the window buttons, the panel toggles, the
-- main tab strip, and space to drag the window by.
local tabbed = use "tabbed"

type State = {
	left: boolean,
	right: boolean,
	bottom: boolean,
	-- What the layout picker (mods/looks) chose, from the topic `looks`:
	-- { tabs?, layout? }, each unset to follow the stylesheet.
	looks: any,
}

-- The tab look in use, and whether the title bar sits over the center: as
-- the picker chose, else as style.luau says. Curved tabs need the latter.
local function looks(state: State, app): (string, boolean)
	local chosen = state.looks or {}
	local tabs = chosen.tabs or (app.looks and app.looks.tabs) or "classic"
	return tabs, tabs == "curved" or chosen.layout == "joined"
end

return mod {
	window = { titlebar = "hidden", traffic_lights = { x = 14, y = 12.5 } },

	subscribe = { "looks" },

	init = function(previous: State?): State
		return previous or { left = true, right = true, bottom = false }
	end,

	update = function(state: State, msg)
		if type(msg) == "table" and msg.topic == "looks" then
			state.looks = msg.value
		elseif msg == "toggle-left" then
			state.left = not state.left
		elseif msg == "toggle-right" then
			state.right = not state.right
		elseif msg == "toggle-bottom" then
			state.bottom = not state.bottom
		end
	end,

	view = function(state: State, app)
		-- An icon button for the toolbar: lit while its panel is shown.
		local function tool(icon, msg, on)
			return ui.row {
				class = on and "tool tool-on" or "tool",
				align = "center",
				justify = "center",
				msg = msg,
				ui.icon { icon },
			}
		end

		local window = app.window
		local mac = window.platform == "macos"
		-- macOS's window buttons sit over the top left corner: with the title
		-- bar hidden (it takes a restart) and not fullscreen.
		local lights = mac and window.titlebar == "hidden" and not window.fullscreen
		-- The curved tab look joins the selected tab to the card under it, so
		-- the strip has to sit on the main card rather than across the window;
		-- the layout picker can put it there for any look.
		local tabs_look, curved = looks(state, app)
		local bar_class = if window.active then "toolbar" else "toolbar toolbar-inactive"
		if curved then
			bar_class ..= " toolbar-joined"
		end
		-- A control in the title bar. The joined bar lines its row up at the
		-- bottom, where the strip meets the card, so each control sits in a
		-- cell as tall as the bar, centred in it.
		local function cell(child: any): any
			if not child or not curved then
				return child
			end
			return ui.row { height = 40, align = "center", child }
		end
		-- A title bar: dragging it moves the window, and a double click zooms
		-- it; the controls in it still take their clicks.
		local function bar(children: { any }): any
			local row: { [any]: any } = {
				drag = true,
				height = 40,
				gap = "small",
				align = if curved then "end" else "center",
				class = bar_class,
			}
			for _, child in children do
				table.insert(row, child)
			end
			return ui.row(row)
		end

		local toolbar = bar {
			-- Room for the window buttons, where they sit in this row: the
			-- joined bar starts past the left panel, which takes them instead.
			lights and not (curved and state.left) and ui.row { width = 72 },
			cell(tool("panel-left", "toggle-left", state.left)),
			cell(tool("panel-bottom", "toggle-bottom", state.bottom)),
			-- The main tab strip; its content fills the main area (mods/editor).
			tabbed { id = "main", show = "bar", start = "component:terminal", variant = tabs_look },
			ui.spacer(),
			-- The layout picker (mods/looks), in a window of its own that stays
			-- open while you try looks.
			cell(ui.row {
				class = "tool",
				align = "center",
				justify = "center",
				window = "looks",
				tooltip = "Layout",
				ui.icon { "layout-dashboard" },
			}),
			cell(ui.row {
				class = "tool",
				align = "center",
				justify = "center",
				command = "show-themes",
				ui.icon { "palette" },
			}),
			cell(tool("panel-right", "toggle-right", state.right)),
			-- Linux and Windows draw no window buttons without a title bar.
			cell(window.titlebar == "hidden" and not mac and ui.row {
				gap = "none",
				ui.button { "–", variant = "ghost", small = true, command = "minimize" },
				ui.button { window.maximized and "❐" or "□", variant = "ghost", small = true, command = "maximize" },
				ui.button { "×", variant = "ghost", small = true, command = "close-window" },
			}),
		}

		local center = ui.vsplit {
			ui.pane { surface = "sidebar", ui.region "main" },
			ui.pane { size = 180, min = 100, fixed = true, visible = state.bottom, surface = "sidebar", ui.region "bottom" },
		}

		-- With the joined bar over the center, each side panel starts at the
		-- top under a bar of its own to drag the window by.
		local function side(region: string): any
			if not curved then
				return ui.region(region)
			end
			return ui.column { grow = true, gap = "none", bar {}, ui.region(region) }
		end
		local middle = if curved then ui.column { grow = true, gap = "none", toolbar, center } else center

		return ui.column {
			grow = true,
			gap = "none",
			not curved and toolbar,
			ui.hsplit {
				ui.pane { size = 240, min = 160, max = 480, fixed = true, visible = state.left, surface = "sidebar", side "left" },
				ui.pane { middle },
				ui.pane { size = 280, min = 180, max = 520, fixed = true, visible = state.right, surface = "sidebar", side "right" },
			},
			ui.row {
				-- A fixed height lets the regions hide the mods that do not fit.
				height = 30,
				shrink = false,
				padding = "small",
				gap = "large",
				align = "center",
				surface = "sidebar",
				border = "top",
				-- Mods on the left, and mods pushed to the right edge.
				ui.region { name = "status", direction = "row", gap = "large" },
				ui.region { name = "status-right", direction = "row", gap = "large", justify = "end" },
			},
		}
	end,
}
