-- A day's stops on a street map: numbered pins in order, and the legs
-- between them as coloured lines, given as points or routed along streets
-- (`travel`). The map frames them all; drag to pan, scroll or pinch to
-- zoom.

type Stop = {
	lat: number,
	lon: number,
	label: string?,
	icon: string?,
	color: string?,
}

type Leg = {
	points: { number }?,
	via: { number }?,
	mode: string?,
	color: string?,
	dashed: boolean?,
}

type Props = {
	stops: { Stop },
	legs: { Leg }?,
	provider: string?,
	key: string?,
	travel: string?,
	marker: string?,
	selected: number?,
	event: string?,
	routed: any,
	height: number?,
	grow: boolean?,
	to: string?,
}

-- Around Union Square and the Embarcadero, San Francisco.
local EXAMPLE_STOPS = {
	{ lat = 37.7880, lon = -122.4040 },
	{ lat = 37.7865, lon = -122.3995 },
	{ lat = 37.7935, lon = -122.3940 },
	{ lat = 37.7885, lon = -122.4045 },
}

local EXAMPLE_LEGS = {
	{ points = { 37.7880, -122.4040, 37.7872, -122.4030, 37.7865, -122.3995 }, color = "success" },
	{ points = { 37.7865, -122.3995, 37.7935, -122.3940 }, color = "success" },
	{ points = { 37.7935, -122.3940, 37.7860, -122.4030, 37.7885, -122.4045 }, color = "danger", dashed = true },
}

return component {
	title = "Route map",
	category = "Data",
	icon = "map",
	description = "A street map with numbered stops and the legs between them, framed to fit; a click on a stop sends { stop = index }.",
	when = "Where a day's meetings are, a delivery round, a trip: places in order and the way between them. For one place without a route, `ui.map` with a pin is enough.",
	props = {
		["stops"] = "list of { lat, lon, label? (default its number), icon? (instead of the label), color? }",
		["legs"] = "list of { points = { lat1, lon1, ... } (drawn as they are) or via = { lat1, lon1, ... } and mode (\"walk\", \"bike\", \"car\", routed along streets), color?, dashed? }",
		["travel"] = "without `legs`: join each stop to the next along streets, \"walk\", \"bike\" or \"car\"",
		["marker"] = "how stops are drawn: \"disc\" (default) or \"balloon\" (a pin above the place)",
		["routed"] = "sent once each routed leg's way is known, with `route` (the leg, from 1), `seconds` and `meters` added",
		["provider"] = "where the tiles come from (default \"openstreetmap\"); see `MapProvider` in types/moonframe.d.luau",
		["key"] = "the provider's API key, for those that need one",
		["selected"] = "index of the stop drawn in the accent colour",
		["event"] = "key of the message a click on a stop sends, default \"stop\": { stop = index }",
		["height"] = "pixels (default: fill the parent)",
		["grow"] = "take the free space along the parent's axis",
		["to"] = "mod to send messages to, if not the drawing one",
	},
	styles = {
		[".route-map"] = { radius = 8, border = "all", border_color = "border" },
	},
	example = {
		stops = EXAMPLE_STOPS,
		legs = EXAMPLE_LEGS,
		selected = 2,
		height = 320,
	},

	build = function(props: Props)
		local pins = {}
		for index, stop in props.stops do
			pins[index] = {
				lat = stop.lat,
				lon = stop.lon,
				label = if stop.icon then nil else stop.label or tostring(index),
				icon = stop.icon :: any,
				color = if props.selected == index then "accent" else stop.color,
				shape = props.marker :: any,
				selected = props.selected == index,
				msg = { [props.event or "stop"] = index },
			}
		end
		local legs: { any }? = props.legs
		if not legs and props.travel then
			local routed = {}
			for index = 2, #props.stops do
				local from, to = props.stops[index - 1], props.stops[index]
				routed[#routed + 1] = { via = { from.lat, from.lon, to.lat, to.lon }, mode = props.travel }
			end
			legs = routed
		end
		return ui.map {
			class = "route-map",
			provider = props.provider :: any,
			key = props.key,
			pins = pins,
			routes = legs :: any,
			routed = props.routed,
			to = props.to,
			height = props.height,
			grow = props.grow,
		}
	end,
}
