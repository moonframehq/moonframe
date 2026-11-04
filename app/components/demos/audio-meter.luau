-- Two channels of sound with their held peaks, the right one near full
-- scale. An engine publishes these levels.
local audio_meter = use "audio-meter"

return mod {
	view = function()
		return audio_meter {
			height = 150,
			channels = {
				{ name = "L", level = -20, peak = -12 },
				{ name = "R", level = -8, peak = -1.5 },
			},
		}
	end,
}
