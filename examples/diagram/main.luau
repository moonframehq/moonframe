-- Diagram's shell: the editor fills the window under its own top bar. The
-- menus hold every key the canvas can't take itself: single letters pick a
-- tool (a row's `keys` never take what types text; a menu's may), and undo,
-- redo and zoom have their usual shortcuts.

local function to_editor(label: string, msg: any, shortcut: string?): any
	return { label = label, msg = msg, to = "editor", shortcut = shortcut }
end

local MENUS: { { title: string, items: { any } } } = {
	{ title = "Edit", items = {
		to_editor("Undo", "undo", "secondary-z"),
		to_editor("Redo", "redo", "secondary-shift-z"),
		"-",
		to_editor("Duplicate", "duplicate", "secondary-d"),
		to_editor("Delete", "delete"),
		to_editor("Select All", "select-all", "secondary-a"),
		"-",
		to_editor("Bring to Front", "front", "secondary-]"),
		to_editor("Send to Back", "back", "secondary-["),
	} },
	{ title = "Tools", items = {
		to_editor("Select", { tool = "select" }, "v"),
		to_editor("Pan", { tool = "hand" }, "h"),
		to_editor("Box", { tool = "box" }, "r"),
		to_editor("Decision", { tool = "diamond" }, "d"),
		to_editor("Ellipse", { tool = "ellipse" }, "o"),
		to_editor("Text", { tool = "text" }, "t"),
		to_editor("Sticky Note", { tool = "sticky" }, "s"),
		to_editor("Connector", { tool = "connector" }, "c"),
	} },
	{ title = "View", items = {
		to_editor("Zoom In", { zoom_by = 1.25 }, "secondary-="),
		to_editor("Zoom Out", { zoom_by = 0.8 }, "secondary--"),
		to_editor("Actual Size", { zoom_to = 1 }, "secondary-0"),
		to_editor("Zoom to Fit", { fit = true }, "shift-1"),
	} },
	{ title = "Document", items = {
		to_editor("Add 100 Shapes (stress test)", { stress = 100 }),
		to_editor("Add 500 Shapes (stress test)", { stress = 500 }),
		"-",
		to_editor("Export PNG…", { export = "png" }, "secondary-shift-e"),
		to_editor("Export SVG…", { export = "svg" }),
		"-",
		to_editor("Start Over with the Sample", "sample"),
		to_editor("Clear", "clear"),
	} },
	{ title = "Window", items = {
		{ label = "Minimize", command = "minimize", shortcut = "secondary-m" },
		{ label = "Zoom", command = "maximize" },
	} },
}

return mod {
	window = { titlebar = "hidden", traffic_lights = { x = 16, y = 18 }, size = { width = 1400, height = 900 } },

	about = {
		version = "0.1",
		description = "Boxes, text and sticky notes on a canvas, wired by connectors that follow them.",
		website = "https://github.com/moonframehq/moonframe",
		copyright = "MIT licensed",
	},

	menus = MENUS,

	view = function()
		return ui.column {
			grow = true,
			gap = "none",
			ui.mount "editor",
			ui.row { float = { at = "bottom-right", inset = 20 }, ui.mount "feedback" },
		}
	end,
}
