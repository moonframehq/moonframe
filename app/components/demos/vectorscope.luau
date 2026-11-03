-- A picture's chroma in a circle, with the primaries and secondaries
-- marked. An engine publishes the trace; here it is made up.
local vectorscope = use "vectorscope"

local TRACE = {}
for step = 1, 240 do
	local radius = 0.1 + 0.3 * math.abs(math.sin(step / 11))
	table.insert(TRACE, radius * math.cos(step / 5) + 0.05)
	table.insert(TRACE, radius * math.sin(step / 7) + 0.08)
end

return mod {
	view = function()
		return vectorscope { size = 220, points = TRACE }
	end,
}
