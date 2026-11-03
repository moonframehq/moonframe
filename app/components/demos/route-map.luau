-- A day of meetings around San Francisco on each provider that needs no
-- key: switch the tiles above the map, click a stop to pick it.
local route_map = use "route-map"
local segmented = use "segmented"

local STOPS = {
	{ lat = 37.7880, lon = -122.4040 },
	{ lat = 37.7865, lon = -122.3995 },
	{ lat = 37.7935, lon = -122.3940 },
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

return mod {
	init = function(previous)
		return previous or { provider = "openstreetmap", stop = 1 }
	end,

	update = function(state, msg)
		if type(msg) ~= "table" then
			return
		end
		if msg.select then
			state.provider = msg.select
		elseif msg.stop then
			state.stop = msg.stop
		end
	end,

	view = function(state)
		return ui.column {
			gap = "normal",
			width = 640,
			segmented { options = PROVIDERS, selected = state.provider, small = true },
			route_map {
				stops = STOPS,
				legs = LEGS,
				provider = state.provider,
				selected = state.stop,
				height = 380,
			},
		}
	end,
}
