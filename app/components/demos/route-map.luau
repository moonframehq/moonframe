-- A day of meetings around San Francisco: switch the tiles, how the stops
-- are joined (walked along streets, or drawn legs) and how they are marked;
-- click a stop to pick it. Routed legs report how long they take.
local route_map = use "route-map"
local segmented = use "segmented"

local STOPS = {
	{ lat = 37.7880, lon = -122.4040 },
	{ lat = 37.7865, lon = -122.3995 },
	{ lat = 37.7935, lon = -122.3940, icon = "coffee" },
	{ lat = 37.7885, lon = -122.4045 },
}

local LEGS = {
	{ points = { 37.7880, -122.4040, 37.7872, -122.4030, 37.7865, -122.3995 }, color = "success" },
	{ points = { 37.7865, -122.3995, 37.7935, -122.3940 }, color = "success" },
	{ points = { 37.7935, -122.3940, 37.7860, -122.4030, 37.7885, -122.4045 }, color = "danger", dashed = true },
}

local PROVIDERS = {
	{ id = "openstreetmap", label = "OpenStreetMap" },
	{ id = "carto-light", label = "Positron" },
	{ id = "carto-voyager", label = "Voyager" },
	{ id = "carto-dark", label = "Dark Matter" },
	{ id = "esri-streets", label = "Esri" },
}

local JOINS = {
	{ id = "walk", label = "Walk" },
	{ id = "bike", label = "Bike" },
	{ id = "car", label = "Car" },
	{ id = "drawn", label = "Drawn" },
}

local MARKERS = {
	{ id = "disc", label = "Disc" },
	{ id = "balloon", label = "Balloon" },
}

return mod {
	init = function(previous)
		return previous or { provider = "openstreetmap", join = "walk", marker = "balloon", stop = 1, minutes = {} }
	end,

	update = function(state, msg)
		if type(msg) ~= "table" then
			return
		end
		if msg.provider then
			state.provider = msg.provider
		elseif msg.join then
			if state.join ~= msg.join then
				state.join = msg.join
				state.minutes = {}
			end
		elseif msg.marker then
			state.marker = msg.marker
		elseif msg.stop then
			state.stop = msg.stop
		elseif msg.leg and msg.route then
			state.minutes[msg.route] = math.max(1, math.floor(msg.seconds / 60 + 0.5))
		end
	end,

	view = function(state)
		local times = {}
		for leg = 1, #STOPS - 1 do
			local minutes = state.minutes[leg]
			times[#times + 1] = `{leg}→{leg + 1}: ` .. (if minutes then `{minutes} min` else "…")
		end
		return ui.column {
			gap = "normal",
			width = 640,
			segmented { options = PROVIDERS, selected = state.provider, small = true, event = "provider" },
			ui.row {
				gap = "normal",
				segmented { options = JOINS, selected = state.join, small = true, event = "join" },
				segmented { options = MARKERS, selected = state.marker, small = true, event = "marker" },
			},
			route_map {
				stops = STOPS,
				legs = if state.join == "drawn" then LEGS else nil,
				travel = if state.join == "drawn" then nil else state.join,
				marker = state.marker,
				provider = state.provider,
				selected = state.stop,
				routed = { leg = true },
				height = 380,
			},
			if state.join == "drawn" then nil else ui.text { table.concat(times, "   "), tone = "muted" },
		}
	end,
}
