-- The console: the picture and its instruments, and what a scan asks of the
-- app. All of it follows what the engine says: the buttons only ask.
local audio_meter = use "audio-meter"
local histogram = use "histogram"
local progress = use "progress"
local slider = use "slider"
local switch = use "switch"
local vectorscope = use "vectorscope"
local video_viewer = use "video-viewer"
local waveform = use "waveform"

local RED, GREEN, BLUE = "#ef4444", "#22c55e", "#3b82f6"
local SIDE = 340

local function panel(title: string, body: any)
	return ui.column { class = "panel", shrink = false, ui.text { title, class = "panel-title" }, body }
end

-- What the app is asked as a scan starts, moves on, and ends.
local function began(): { any }
	return {
		{ to = "app:awake", msg = { hold = true, display = false, reason = "Lightbox is scanning" } },
		{ to = "app:guard", msg = { hold = "closing", timeout = 60 } },
	}
end

local function ended(state: any, done: boolean): { any }
	local asked: { any } = {
		{ to = "app:awake", msg = { hold = false } },
		{ to = "app:dock", msg = { badge = false, progress = false } },
		{ to = "app:guard", msg = if state.leaving then { release = true } else { hold = false } },
	}
	if done then
		table.insert(asked, { to = "app:notify", msg = { title = "Scan finished", body = "All frames are in." } })
		table.insert(asked, { to = "app:sound", msg = "default" })
	end
	return asked
end

local function follow(state: any, scanner: any)
	local asked: { any } = {}
	if scanner.running and not state.running then
		asked = began()
	elseif state.running and not scanner.running then
		asked = ended(state, scanner.done)
		state.closing, state.leaving = nil, false
	end
	state.running = scanner.running
	local percent = math.floor(scanner.frame / scanner.total * 100)
	if scanner.running and percent ~= state.percent then
		state.percent = percent
		table.insert(asked, { to = "app:dock", msg = { progress = percent / 100, badge = `{percent}%` } })
	end
	return state, asked
end

return mod {
	region = "main",
	subscribe = { "provider/scanner" },

	init = function(previous)
		local state = previous or {
			exposure = 1,
			parade = true,
			zoom = nil :: number?,
			center = { 0.5, 0.5 },
			stats = {},
			running = false,
			percent = -1,
			closing = nil :: string?,
			leaving = false,
		}
		-- Holds and pending close attempts belong to the app process, not
		-- its saved state. Restore protection until the engine reports back.
		state.closing, state.leaving = nil, false
		state.percent = -1
		return state, if state.running then began() else {}
	end,

	update = function(state, msg)
		if msg == "start" or msg == "stop" then
			return state, { { to = "provider:scanner", msg = msg } }
		elseif msg == "parade" then
			state.parade = not state.parade
		elseif msg == "stop-and-close" then
			state.leaving = true
			return state, { { to = "provider:scanner", msg = "stop" } }
		elseif msg == "keep-scanning" then
			state.closing = nil
			return state, { { to = "app:guard", msg = { hold = "closing", timeout = 60 } } }
		elseif typeof(msg) == "table" then
			if msg.topic == "provider/scanner" then
				return follow(state, msg.value)
			elseif msg.closing then
				state.closing = msg.closing
			elseif msg.viewed then
				state.zoom, state.center = msg.zoom, msg.center
			elseif msg.stats then
				state.stats = msg
			elseif msg.exposure then
				state.exposure = msg.value
				return state, { { to = "provider:scanner", msg = { exposure = msg.value } } }
			end
		end
		return state
	end,

	view = function(state, app)
		local scanner: any = (app :: any).scanner or {}
		local stats: any = state.stats
		local wave: any = scanner.waveform or { {}, {}, {} }
		local tones: any = scanner.tones or { {}, {}, {} }
		local frame, total = scanner.frame or 0, scanner.total or 1
		return ui.row {
			grow = true,
			gap = "normal",
			padding = "normal",
			ui.column {
				grow = true,
				gap = "normal",
				if state.closing
					then ui.row {
						class = "closing",
						align = "center",
						ui.text { "A scan is running. Stop it first?", grow = true },
						ui.button { "Keep scanning", msg = "keep-scanning" },
						ui.button { "Stop and close", msg = "stop-and-close", variant = "primary" },
					}
					else nil,
				video_viewer {
					source = "lightbox",
					controls = true,
					gain = state.exposure,
					zoom = state.zoom,
					center = state.center,
					change = { viewed = true },
					stats = { stats = true },
					guides = { { x = 0.05, y = 0.05, width = 0.9, height = 0.9, color = "#ffcc00aa" } },
				},
				ui.row {
					gap = "normal",
					align = "center",
					shrink = false,
					ui.button {
						if frame > 0 and not scanner.done then "Resume" else "Start",
						msg = "start",
						variant = "primary",
						disabled = scanner.running,
					},
					ui.button { "Stop", msg = "stop", disabled = not scanner.running },
					progress {
						value = frame / total,
						label = if scanner.done then "Scan finished" elseif scanner.running then "Scanning" else "Ready",
						detail = `{frame} of {total} frames`,
						width = 320,
					},
					ui.spacer(),
					ui.text { `{string.format("%.0f", stats.fps or 0)} i/s shown`, size = "small", tone = "muted" },
					slider {
						label = "Exposure",
						inline = true,
						label_width = 56,
						value = state.exposure,
						min = 0.25,
						max = 2,
						decimals = 2,
						unit = "×",
						default = 1,
						width = 120,
						change = { exposure = true },
					},
				},
			},
			ui.column {
				width = SIDE,
				shrink = false,
				gap = "normal",
				scroll = true,
				panel(
					"Waveform",
					ui.column {
						gap = "small",
						waveform {
							width = SIDE - 26,
							height = 170,
							parade = state.parade,
							channels = {
								{ name = "Red", color = RED .. "c0", bands = wave[1] },
								{ name = "Green", color = GREEN .. "c0", bands = wave[2] },
								{ name = "Blue", color = BLUE .. "c0", bands = wave[3] },
							},
						},
						switch { label = "Parade", on = state.parade, msg = "parade" },
					}
				),
				panel(
					"Vectorscope and sound",
					ui.row {
						gap = "normal",
						vectorscope { size = 200, points = scanner.scope or {} },
						audio_meter { height = 200, bar = 14, channels = scanner.sound or {} },
					}
				),
				panel(
					"Tones",
					histogram {
						width = SIDE - 26,
						height = 110,
						log = true,
						channels = {
							{ color = RED, values = tones[1] },
							{ color = GREEN, values = tones[2] },
							{ color = BLUE, values = tones[3] },
						},
					}
				),
			},
		}
	end,
}
