-- A mic button: hold it and talk, or tap to start and tap to stop, and the
-- words come back to the mod, which puts them where they belong (a prompt, a note). Listening and
-- transcribing are the `speech` provider's; which engine turns sound into
-- text is the user's choice, in its provider.toml.

type Props = {
	id: string?,
	text: any,
	failed: any,
	listening: any,
	engine: string?,
	speech: any,
}

type State = {
	-- This button started what the provider is doing now.
	mine: boolean,
	-- Opening the microphone counts as listening for the parent's cleanup.
	pending: boolean?,
	-- The press under way started listening, so letting go may end it.
	held: boolean?,
	error: string?,
}

-- What `ui.input` does with its messages: a table gets `value`, anything
-- else arrives as `{ msg = ..., value = ... }`.
local function shaped(msg: any, value: any)
	if type(msg) == "table" then
		local copy = table.clone(msg)
		copy.value = value
		return copy
	end
	return { msg = msg, value = value }
end

local function tell(sends: { any }, msg: any, value: any)
	if msg ~= nil then
		table.insert(sends, { to = "parent", msg = shaped(msg, value) })
	end
end

return component {
	title = "Dictation",
	category = "Forms",
	icon = "mic",
	description = "A mic button, held or tapped, that turns what the user says into text for the mod, through a local or remote speech engine.",
	when = "Let the user talk instead of type: beside a prompt, in `composer`'s `actions`. The drawing mod must be in `recorders` of providers/speech. Give it `listening`, and send `provider:speech` `{ cancel = true }` if it stops being drawn while listening (a closing dialog), or the microphone stays on until the limit.",
	props = {
		["id"] = "names this button, so a mod can reach it as `dictation#<id>` (\"toggle\" or \"cancel\", for a shortcut); default \"dictation\"",
		["text"] = "sent to the mod with `value` set to what was said, as `ui.input`'s `change` is",
		["failed"] = "sent with `value` set to why it failed, to show it (it is on the button's tooltip anyway)",
		["listening"] = "sent with `value` true when microphone startup is requested and false once it is done",
		["engine"] = "an engine's name from providers/speech; default the first ready in its `prefer`",
		["speech"] = "stands in for `app.speech`, for a preview",
	},
	styles = {
		[".dictation"] = { width = 28, height = 28, radius = 14, color = "muted-foreground", transition = "fast ease-out" },
		[".dictation:hover"] = { background = "list-hover", color = "foreground" },
		[".dictation:recording"] = { background = "danger/15", color = "danger" },
		[".dictation:transcribing"] = { color = "accent" },
		[".dictation:off"] = { opacity = 0.45 },
		[".dictation:failed"] = { color = "danger" },
		[".dictation-level"] = { height = 2, radius = 1, background = "danger" },
	},
	example = { id = "example", speech = { available = true, state = "idle", level = 0, seconds = 0 } },
	usage = [[
local dictation = use "dictation"

-- in view: beside the field, with the words added to the draft
composer {
	value = state.draft,
	change = "draft",
	actions = { dictation { id = "prompt", text = "dictated", listening = "listening" } },
}

-- in update
if msg.msg == "dictated" then
	state.draft = if state.draft == "" then msg.value else state.draft .. " " .. msg.value
end
]],

	-- A live reload leaves the provider running. End the old take before
	-- resetting ownership; persisted state from a prior launch is harmless.
	init = function(previous: State?, props: any)
		local sends: { any } = {}
		if previous and (previous.mine or previous.pending) then
			table.insert(sends, { to = "provider:speech", msg = { cancel = true } })
			tell(sends, props.listening, false)
		end
		return { mine = false }, sends
	end,

	update = function(state: State, msg: any, props: any)
		local sends: { any } = {}
		if type(msg) == "table" and msg.press then
			-- The button: down does what it shows, and up is a release only
			-- for the press that started listening.
			if msg.phase == "down" and msg.button == "left" then
				state.held = msg.press == "start"
				msg = msg.press
			elseif msg.phase == "up" and state.held then
				state.held = false
				return state, { { to = "provider:speech", msg = { release = true } } }
			else
				return nil
			end
		end
		if msg == "toggle" then
			msg = if state.pending then "cancel" elseif state.mine then "stop" else "start"
		end
		if msg == "start" then
			if state.mine or state.pending then
				return nil
			end
			state.error = nil
			state.pending = true
			tell(sends, props.listening, true)
			table.insert(sends, { to = "provider:speech", msg = { start = true, id = "dictation", engine = props.engine } })
		elseif msg == "stop" then
			table.insert(sends, { to = "provider:speech", msg = { stop = true } })
		elseif msg == "cancel" then
			table.insert(sends, { to = "provider:speech", msg = { cancel = true } })
		elseif type(msg) == "table" and msg.reply == "dictation" then
			if msg.action == "start" and msg.ok then
				-- A reply queued before a reload must not resurrect its take.
				if not state.pending then
					return state, { { to = "provider:speech", msg = { cancel = true } } }
				end
				state.pending = false
				state.mine = true
			else
				if not state.mine and not state.pending then
					return nil
				end
				tell(sends, props.listening, false)
				state.pending = false
				state.mine = false
				if msg.ok and msg.text ~= "" then
					tell(sends, props.text, msg.text)
				elseif not msg.ok and not msg.cancelled then
					state.error = msg.error
					tell(sends, props.failed, msg.error)
				end
			end
		end
		return state, sends
	end,

	build = function(props: Props, kept: any, app: any)
		local state: State = kept
		local speech = props.speech or app.speech
		-- A build without the provider: no button that can't work.
		if speech == nil then
			return ui.row {}
		end
		local doing = speech.state or "idle"
		local ours = props.speech ~= nil or state.mine
		local available, reason = speech.available, speech.reason
		if props.engine then
			available, reason = false, `No speech engine named \`{props.engine}\``
			for _, engine in speech.engines or {} do
				if engine.name == props.engine then
					available, reason = engine.ready, engine.reason
					break
				end
			end
		end
		local look, tip, icon = "idle", "Hold to talk, or tap to start and stop", "mic"
		local press: string? = "start"
		if state.pending then
			look, tip, icon, press = "transcribing", "Opening microphone… click to cancel", "loader-circle", "cancel"
		elseif doing ~= "idle" and not ours then
			look, tip, press = "off", "Another mod is listening", nil
		elseif doing == "recording" then
			look, tip, icon, press = "recording", "Listening: let go, or tap to finish", "square", "stop"
		elseif doing == "transcribing" then
			look, tip, icon, press = "transcribing", "Transcribing… click to cancel", "loader-circle", "cancel"
		elseif not available then
			look, tip, press = "off", reason or "No speech engine is ready", nil
		elseif state.error then
			look, tip = "failed", state.error
		end
		local fill = math.clamp(speech.level or 0, 0, 1)
		-- One element whatever it is doing: a press is followed by its place
		-- in the tree, so nothing may come or go around it while held. The
		-- level sits inside, which also keeps the button under the finger.
		return ui.row {
			class = "dictation",
			state = look,
			align = "center",
			justify = "center",
			shrink = false,
			tooltip = tip,
			pointer = if press then { press = press } else nil,
			ui.icon { name = icon :: any, size = "small", spin = state.pending == true or (doing == "transcribing" and ours) },
			doing == "recording" and ours and ui.row {
				class = "dictation-level",
				float = { bottom = 3, left = 6 },
				width = 16 * fill,
			},
		}
	end,
}
