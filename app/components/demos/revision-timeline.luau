-- Both looks use the same Rust element. All records here are demo data.
local ruler = use "revision-timeline"
local segmented = use "segmented"
local slider = use "slider"
local switch = use "switch"
local revisions = {}
for _, day in { "2 Nov 2026", "3 Nov 2026", "4 Nov 2026", "5 Nov 2026", "6 Nov 2026", "Yesterday", "Today" } do
	for revision = 1, 6 do
		local index = #revisions + 1
		table.insert(revisions, { id = `r{index}`, day = day, label = `{8 + revision * 2}:00 · Revision {index}` })
	end
end
local turns = {}
for index, label in {
	"A new workspace", "Choose a project", "Explore the source", "Sketch the interface",
	"Review the first draft", "Refine the typography", "Add keyboard navigation", "Try the compact view",
	"Check the light theme", "The timeline is ready", "Review the changes", "What should we build next?",
} do
	table.insert(turns, { id = `t{index}`, label = label,
		summary = if index % 3 == 1 then "A demo conversation turn. Move along the rail to preview another message, then click to jump to it." else nil })
end

local function preset(name: string)
	local chat = name == "chat"
	return { preset = name, selected = if chat then 10 else 36, width = 300,
		tick_width = if chat then 6 else 12, magnification = if chat then 4 else 4.5,
		radius = if chat then 3 else 4.5, spread = if chat then 0 else 1.8,
		dates = not chat, preview = chat, side = if chat then "left" else "right", labels = if chat then "hover" else "all" }
end

return mod {
	init = function() return preset("time-machine") end,
	update = function(state, msg)
		if type(msg) ~= "table" then return end
		if msg.preset then return preset(msg.preset)
		elseif msg.msg == "select" then state.selected = msg.index
		elseif msg.set then state[msg.set] = msg.value
		elseif msg.flip then state[msg.flip] = not state[msg.flip]
		elseif msg.side then state.side = msg.side
		elseif msg.labels then state.labels = msg.labels end
	end,
	view = function(state)
		local chat = state.preset == "chat"
		local entries = if chat then turns else revisions
		local function control(field: string, label: string, min: number, max: number, step: number)
			return slider { label = label, value = state[field], min = min, max = max,
				step = step, decimals = if step < 1 then 1 else 0, change = { set = field }, width = 220 }
		end
		return ui.column { gap = "normal",
			segmented { options = { { id = "time-machine", label = "Time Machine" }, { id = "chat", label = "Chat rail" } }, selected = state.preset, event = "preset" },
			ui.row { gap = "large", align = "center",
				ui.column { width = 230, gap = "normal",
					ui.badge { "DEMO DATA", tone = "muted" },
					control("width", "Component width", 120, 340, 10),
					control("tick_width", "Tick width", 2, 24, 1),
					control("magnification", "Magnification", 1, 8, 0.5),
					control("radius", "Lens radius", 1, 10, 0.5),
					control("spread", "Vertical expansion", 0, 3, 0.2),
					switch { label = "Dates (when supplied)", on = state.dates, msg = { flip = "dates" } },
					switch { label = "Preview card", on = state.preview, msg = { flip = "preview" } },
					segmented { options = { { id = "left", label = "Left" }, { id = "right", label = "Right" } }, selected = state.side, event = "side" },
					segmented { small = true, options = { { id = "all", label = "All" }, { id = "active", label = "Active" }, { id = "hover", label = "Hover" }, { id = "none", label = "None" } }, selected = state.labels, event = "labels" },
					ui.text { entries[state.selected].label, tone = "accent", size = "small" },
				},
				ruler { id = "history", entries = entries, selected = entries[state.selected].id,
					style = { color = if chat then "foreground" else "accent" },
					width = state.width, height = if chat then 220 else 460,
					tick_width = state.tick_width, tick_height = if chat then 1.5 else 2,
					magnification = state.magnification, radius = state.radius, spread = state.spread,
					side = state.side, dates = state.dates, labels = state.labels, preview = state.preview },
			},
		}
	end,
}
