-- A settings window: pick a page on the left; change a setting and the
-- footer says Unsaved until you Save (Revert puts the saved values back).
local settings_page = use "settings-page"
local switch = use "switch"
local segmented = use "segmented"
local dropdown = use "dropdown"

local SAVED = { preset = "comfortable", footer = "always", agents = true, icons = false, theme = "dark", ligatures = true }

local PRESETS = {
	{ id = "comfortable", label = "Comfortable Rounded" },
	{ id = "compact", label = "Compact" },
	{ id = "cozy", label = "Cozy Square" },
}

local NAV_GROUPS = {
	{
		title = "Appearance",
		items = {
			{ id = "theme", label = "Theme", icon = "palette" },
			{ id = "sidebar", label = "Sidebar", icon = "panel-left" },
		},
	},
	{
		title = "Behaviour",
		items = {
			{ id = "general", label = "General", icon = "settings" },
			{ id = "notifications", label = "Notifications", icon = "bell" },
		},
	},
}

local function pages(state)
	local v = state.values
	return {
		sidebar = {
			title = "Sidebar",
			subtitle = "Changes apply to every open window immediately.",
			sections = {
				{
					title = "Layout",
					rows = {
						{
							title = "Layout preset",
							tag = "Local",
							description = "Row shape and density; spacing below starts from the preset.",
							control = dropdown {
								open = state.preset_open,
								selected = v.preset,
								options = PRESETS,
								toggle = "toggle-preset",
								dismiss = "close-preset",
								align = "end",
							},
						},
						{
							title = "Device footer",
							description = "The device picker and its buttons.",
							control = segmented {
								selected = v.footer,
								small = true,
								event = "footer",
								options = { { id = "always", label = "Always" }, { id = "auto", label = "Auto" }, { id = "never", label = "Never" } },
							},
						},
						{
							title = "Show agents",
							description = "List running agents under their workspace.",
							control = switch { on = v.agents, msg = { flip = "agents" } },
						},
						{
							title = "Show device icons",
							tag = "Shared",
							tag_tone = "shared",
							description = "A mark before each device's name.",
							disabled = not v.agents,
							reason = if v.agents then nil else "Needs Show agents.",
							control = switch { on = v.icons, disabled = not v.agents, msg = { flip = "icons" } },
						},
					} :: { any },
				},
			},
		},
		theme = {
			title = "Theme",
			sections = {
				{
					title = "Colors",
					rows = {
						{
							title = "Appearance",
							control = segmented {
								selected = v.theme,
								small = true,
								event = "theme",
								options = { { id = "light", label = "Light" }, { id = "dark", label = "Dark" }, { id = "system", label = "System" } },
							},
						},
						{
							title = "Font ligatures",
							description = "Join -> and != into one glyph in terminals.",
							control = switch { on = v.ligatures, msg = { flip = "ligatures" } },
						},
					} :: { any },
				},
			},
		},
	}
end

local function dirty(state): boolean
	for key in SAVED do
		if state.values[key] ~= state.saved[key] then
			return true
		end
	end
	return false
end

return mod {
	init = function(previous)
		return previous or { page = "sidebar", values = table.clone(SAVED), saved = table.clone(SAVED), preset_open = false }
	end,

	update = function(state, msg)
		if msg == "toggle-preset" then
			state.preset_open = not state.preset_open
		elseif msg == "close-preset" then
			state.preset_open = false
		elseif msg == "save" then
			state.saved = table.clone(state.values)
		elseif msg == "revert" then
			state.values = table.clone(state.saved)
		elseif type(msg) == "table" then
			if msg.page then
				state.page = msg.page
			elseif msg.pick then
				state.values.preset = msg.pick
				state.preset_open = false
			elseif msg.footer then
				state.values.footer = msg.footer
			elseif msg.theme then
				state.values.theme = msg.theme
			elseif msg.flip then
				state.values[msg.flip] = not state.values[msg.flip]
			end
		end
	end,

	view = function(state)
		local page = pages(state)[state.page]
		local unsaved = dirty(state)
		return settings_page {
			height = 480,
			nav = { groups = NAV_GROUPS, selected = state.page, event = "page" },
			title = if page then page.title else "General",
			subtitle = if page then page.subtitle else "Nothing here yet in this demo.",
			sections = if page then page.sections else {},
			status = if unsaved then "Unsaved changes" else "All changes saved.",
			state = if unsaved then "unsaved" else "saved",
			buttons = {
				{ label = "Revert", msg = "revert", variant = "ghost" },
				{ label = "Save", msg = "save", variant = "primary" },
			},
		}
	end,
}
