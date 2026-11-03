-- Sound levels as a meter bridge draws them: a bar per channel in decibels,
-- with a level, a held peak, and its colour turning near full scale.

type Channel = {
	level: number,
	peak: number?,
	name: string?,
}

type Props = {
	channels: { Channel },
	height: number?,
	bar: number?,
	floor: number?,
	warn: number?,
	danger: number?,
}

return component {
	title = "Audio Meter",
	category = "Data",
	icon = "mic",
	description = "A vertical bar per channel in dBFS, with a held peak and colours near full scale.",
	when = "Sound levels while recording or playing, from values an engine published. For a fraction that isn't sound use `vertical-bar` or `meter`.",
	props = {
		["channels"] = "list of { level, peak?, name? }, levels in dBFS (0 is full scale)",
		["height"] = "pixels (default 140)",
		["bar"] = "each bar's width in pixels (default 10)",
		["floor"] = "the level at the bottom, in dBFS (default -60)",
		["warn"] = "the level it turns to the warning colour at (default -12)",
		["danger"] = "the level it turns to the danger colour at (default -3)",
	},
	styles = {
		[".audio-meter-track"] = { background = "muted-foreground/20", radius = 2, clip = true },
		[".audio-meter-fill"] = { background = "success" },
		[".audio-meter-warn"] = { background = "warning" },
		[".audio-meter-danger"] = { background = "danger" },
		[".audio-meter-peak"] = { background = "foreground" },
		[".audio-meter-label"] = { size = "small", color = "muted-foreground" },
	},
	example = {
		channels = { { name = "L", level = -18, peak = -9 }, { name = "R", level = -14, peak = -2 } },
		height = 120,
	},

	build = function(props: Props)
		local height = props.height or 140
		local bar = props.bar or 10
		local floor = props.floor or -60
		local warn = props.warn or -12
		local danger = props.danger or -3
		local function fraction(level: number): number
			return math.clamp((level - floor) / -floor, 0, 1)
		end
		local bars = {}
		for index, channel in props.channels do
			local class = if channel.level >= danger
				then { "audio-meter-fill", "audio-meter-danger" }
				elseif channel.level >= warn then { "audio-meter-fill", "audio-meter-warn" }
				else "audio-meter-fill"
			local peak = if channel.peak
				then ui.row {
					class = "audio-meter-peak",
					float = { left = 0, bottom = fraction(channel.peak) * (height - 2) },
					width = bar,
					height = 2,
				}
				else nil
			bars[index] = ui.column {
				align = "center",
				gap = "small",
				hug = true,
				ui.column {
					class = "audio-meter-track",
					width = bar,
					height = height,
					justify = "end",
					ui.row { class = class, width = bar, height = fraction(channel.level) * height },
					peak,
				},
				channel.name and ui.text { channel.name, class = "audio-meter-label" },
			}
		end
		return ui.row { gap = "small", hug = true, table.unpack(bars) }
	end,
}
