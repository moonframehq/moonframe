-- The picture, what the engine says it did, and what reached the screen.
-- The buttons send commands; only what `app.engine` says afterwards counts
-- as having happened.
local video_viewer = use "video-viewer"
local slider = use "slider"

local RATES = { 18, 25, 50 }

local function number(value: number?, digits: number): string
	return if value then string.format(`%.{digits}f`, value) else "-"
end

local function figure(label: string, value: string)
	return ui.column {
		hug = true,
		ui.text { label, size = "small", tone = "muted" },
		ui.text { value, weight = "semibold" },
	}
end

return mod {
	region = "main",

	init = function(previous)
		return previous or { zoom = nil :: number?, center = { 0.5, 0.5 }, gain = 1, stats = {}, probed = nil :: any }
	end,

	update = function(state, msg)
		if typeof(msg) ~= "table" then
			if msg == "start" or msg == "stop" then
				return state, { { to = "provider:engine", msg = msg } }
			elseif msg == "preview-window" then
				return state, { { to = "app:window", msg = { open = "screen", display = "external" } } }
			end
			return
		end
		if msg.viewed then
			state.zoom, state.center = msg.zoom, msg.center
		elseif msg.stats then
			state.stats = msg
		elseif msg.probed then
			state.probed = msg
		elseif msg.rate then
			return state, { { to = "provider:engine", msg = { rate = msg.rate } } }
		elseif msg.gain then
			state.gain = msg.value
			return state, { { to = "provider:engine", msg = { gain = msg.value } } }
		end
	end,

	view = function(state, app)
		local engine: any = (app :: any).engine or {}
		local links: any = app.links or {}
		local link: any = links.engine or {}
		local stats: any = state.stats
		local buttons = {}
		for _, rate in RATES do
			table.insert(buttons, ui.button {
				`{rate} i/s`,
				msg = { rate = rate },
				variant = if engine.rate == rate then "primary" else nil,
			})
		end
		local probe: any = state.probed
		return ui.column {
			grow = true,
			ui.row {
				gap = "small",
				padding = "small",
				align = "center",
				ui.button { "Start", msg = "start", variant = if engine.running then "primary" else nil },
				ui.button { "Stop", msg = "stop" },
				buttons[1],
				buttons[2],
				buttons[3],
				ui.spacer(),
				ui.button { "Second screen", msg = "preview-window" },
				ui.text {
					if link.connected then "Engine linked" else "No engine: run engine/engine.py",
					tone = if link.connected then "success" else "warning",
				},
			},
			video_viewer {
				source = "framebench",
				controls = true,
				white = 4095,
				gain = state.gain,
				zoom = state.zoom,
				center = state.center,
				change = { viewed = true },
				stats = { stats = true },
				probed = { probed = true },
				guides = { { x = 0.05, y = 0.05, width = 0.9, height = 0.9, color = "#ffcc00aa" } },
			},
			ui.row {
				gap = "normal",
				padding = "small",
				figure("Shown", `{number(stats.fps, 1)} i/s`),
				figure("Skipped /s", tostring(stats.skipped or "-")),
				figure("Age on screen", `{number(stats.latency, 1)} ms`),
				figure("Engine wrote", tostring(engine.written or "-")),
				figure("Engine late", tostring(engine.late or "-")),
				figure("Queue", `{link.waiting or 0} / {link.dropped or 0} dropped`),
				ui.column {
					gap = "small",
					ui.text { "Gain", size = "small" },
					slider { value = state.gain, min = 0, max = 4, width = 140, change = { gain = true } },
					ui.text {
						if probe then `{probe.channel or "pixel"} {probe.x},{probe.y}: {number(probe.source[1], 0)} shows as {number(probe.display[1], 3)}` else "Press the picture to read a pixel",
						size = "small",
					},
				},
			},
		}
	end,
}
