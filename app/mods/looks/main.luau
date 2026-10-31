-- The layout picker: the tab style and where the title bar goes, over what
-- style.luau says. It draws in no region: the title bar's layout button opens
-- it in a window of its own (`window = "looks"`), which stays open while you
-- try one look after another. The choices are kept across launches and
-- published on the topic `looks` as { tabs?, layout? }; the shell and the
-- main area (mods/editor) draw to match.
local tabs = use "tabs"

type State = {
	-- A tab look; unset follows style.luau's `looks.tabs`.
	look: string?,
	-- "across" or "joined"; unset joins the title bar to the main card for
	-- the curved look only, which needs it.
	layout: string?,
}

local LOOKS = { "pills", "underline", "outline", "cards", "compact", "classic", "square", "curved" }

local LAYOUTS = {
	{ id = "auto", label = "Automatic", detail = "over the editor for curved tabs, else across" },
	{ id = "across", label = "Across the window", detail = "the side panels start under it" },
	{ id = "joined", label = "Over the editor", detail = "the strip sits on the main card" },
}

local function announce(state: State)
	return { to = "topic:looks", msg = { tabs = state.look, layout = state.layout } }
end

local function title(name: string): string
	return string.upper(string.sub(name, 1, 1)) .. string.sub(name, 2)
end

-- A strip in `look` as the title bar would draw it, over a strip of card as
-- the main area would be, so a joined look shows its join. Clicking anywhere
-- in it picks the look.
local function sample(look: string, pick)
	local strip = {}
	for index, label in { "Terminal", "Browser", "Files" } do
		table.insert(strip, {
			id = label,
			label = label,
			icon = ({ "square-terminal", "globe", "folder" })[index],
			msg = pick,
		})
	end
	return ui.column {
		gap = "none",
		ui.row {
			class = "sample-bar",
			align = "end",
			height = 40,
			tabs { tabs = strip, selected = "Terminal", variant = look, add = pick },
		},
		ui.row { class = "sample-card", height = 14 },
	}
end

return mod {
	title = "Layout",

	styles = {
		[".looks-page"] = { padding = 16, gap = 14 },
		[".looks-heading"] = { size = "small", weight = "medium", color = "muted-foreground", case = "upper" },
		[".look-tile"] = { padding = 8, gap = 6, radius = 10, border = true, border_color = "border", background = "card" },
		[".look-tile:hover"] = { border_color = "muted-foreground/50" },
		[".look-tile:selected"] = { border_color = "accent", shadow = "0 0 0 2px accent/25" },
		[".look-name"] = { weight = "medium" },
		[".look-note"] = { size = "small", color = "muted-foreground" },
		[".sample-bar"] = { background = "sidebar", padding = { 0, 12 }, radius = { 6, 6, 0, 0 } },
		[".sample-card"] = { background = "card" },
		[".layout-row"] = { padding = { 8, 10 }, gap = 10, radius = 8 },
		[".layout-row:hover"] = { background = "list-hover" },
		[".layout-row:selected"] = { background = "accent/12", color = "accent" },
	},

	init = function(previous: State?)
		local state: State = previous or {}
		return state, { announce(state) }
	end,

	update = function(state: State, msg)
		if type(msg) ~= "table" then
			return
		end
		if msg.look then
			state.look = if msg.look == "stylesheet" then nil else msg.look
		elseif msg.layout then
			state.layout = if msg.layout == "auto" then nil else msg.layout
		else
			return
		end
		return state, { announce(state) }
	end,

	view = function(state: State, app)
		local sheet = (app.looks and app.looks.tabs) or "classic"
		local tiles = {}
		local function tile(id: string, name: string, look: string, note: string?)
			local pick = { look = id }
			table.insert(tiles, ui.column {
				class = "look-tile",
				selected = (state.look or "stylesheet") == id,
				msg = pick,
				ui.row {
					gap = "small",
					align = "center",
					ui.text { name, class = "look-name" },
					note and ui.text { note, class = "look-note" },
				},
				sample(look, pick),
			})
		end
		tile("stylesheet", "As style.luau says", sheet, sheet)
		for _, look in LOOKS do
			tile(look, title(look), look)
		end

		local layouts = {}
		for _, layout in LAYOUTS do
			table.insert(layouts, ui.row {
				class = "layout-row",
				align = "center",
				selected = (state.layout or "auto") == layout.id,
				msg = { layout = layout.id },
				ui.text { layout.label, class = "look-name" },
				ui.text { layout.detail, class = "look-note" },
			})
		end

		return ui.column {
			grow = true,
			scroll = "y",
			class = "looks-page",
			ui.text { "Tab style", class = "looks-heading" },
			ui.column { gap = "small", table.unpack(tiles) },
			ui.text { "Title bar", class = "looks-heading" },
			ui.column { gap = "none", table.unpack(layouts) },
		}
	end,
}
