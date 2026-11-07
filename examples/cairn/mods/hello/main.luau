-- The greeter service. It owns the answer, the count and the generation, and
-- publishes them on `cairn/greeter`; Echo asks it from the workbench, and
-- the top bar, the chat's header and its toast all read what comes back.
-- Restarting it starts a new generation, as a reloaded service would.

local key_values = use "key-values"
local switch = use "switch"

type State = { on: boolean, generation: number, count: number, name: string, answer: string, words: number }

local function publish(state: State)
	return { { to = "topic:cairn/greeter", msg = {
		on = state.on, generation = state.generation, count = state.count,
		name = state.name, answer = state.answer, words = state.words,
	} } }
end

local function word_count(text: string): number
	local words = 0
	for _ in string.gmatch(text, "%S+") do
		words += 1
	end
	return words
end

return mod {
	title = "Hello",

	styles = {
		[".hello"] = { padding = { 18, 18 }, gap = 16 },
		[".hello-title"] = { size = "large", weight = "semibold" },
		[".hello-card"] = { background = "raised", radius = 10, padding = 16, gap = 12 },
	},

	init = function(previous: State?)
		local state: State = previous or { on = true, generation = 1, count = 3, name = "Kit", answer = "Hello, Kit!", words = 2 }
		return state, publish(state)
	end,

	update = function(state: State, msg)
		if msg == "toggle" then
			state.on = not state.on
		elseif msg == "restart" then
			state.generation += 1
			state.count = 0
			state.on = true
		elseif type(msg) == "table" and msg.greet and state.on then
			state.name = msg.greet
			state.answer = `Hello, {msg.greet}!`
			state.words = word_count(state.answer)
			state.count += 1
		else
			return
		end
		return state, publish(state)
	end,

	view = function(state: State)
		return ui.column { class = "hello", grow = true, scroll = true,
			ui.row { align = "center", gap = "normal",
				ui.icon { "smile-plus", size = "large" },
				ui.text { "Hello", class = "hello-title" },
			},
			ui.text { "A mod that offers the greeter service. Echo reaches it through messages and a topic, never its state.", tone = "muted" },
			ui.column { class = "hello-card",
				key_values { width = 110, rows = {
					{ "Generation", tostring(state.generation) },
					{ "Greetings", tostring(state.count) },
					{ "Last answer", state.answer },
				} },
			},
			switch { label = "Greeter service", description = "Off, Echo gets no answer.", on = state.on, msg = "toggle" },
			ui.row { ui.button { "Restart service", icon = "refresh-cw", small = true, variant = "outline", msg = "restart" } },
		}
	end,
}
