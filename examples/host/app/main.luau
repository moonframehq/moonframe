-- The shell of an app on the Moonframe library: a header with the time
-- from the `clock` provider (Rust, in src/main.rs), a word looked up by the
-- async `lookup` provider (Rust on tokio), and the `counter` view (Rust
-- too), which tells this mod each time it is clicked.

return mod {
	about = {
		version = "0.1",
		description = "An app of its own on the Moonframe library.",
	},

	init = function(previous)
		return previous or { counted = 0, step = 1, word = "" }
	end,

	update = function(state, msg)
		if type(msg) == "table" and msg.counted then
			state.counted = msg.counted
		elseif type(msg) == "table" and msg.msg == "word" then
			state.word = msg.value
		elseif msg == "step" then
			state.step = if state.step >= 5 then 1 else state.step + 1
		end
	end,

	view = function(state, app)
		local clock = app.clock or { time = "…", ticks = 0 }
		local lookup = app.lookup or { answered = 0 }
		return ui.column {
			grow = true,
			padding = "large",
			gap = "normal",
			ui.row {
				align = "center",
				ui.text { "Host Example", size = "large", weight = "bold" },
				ui.spacer(),
				ui.text { clock.time, tone = "muted" },
				ui.badge { `{clock.ticks} ticks` },
				ui.button { "Reset", small = true, msg = { reset = true }, to = "provider:clock" },
			},
			ui.panel {
				title = "A view written in Rust",
				grow = true,
				actions = { ui.button { `Step: {state.step}`, small = true, msg = "step" } },
				ui.view {
					name = "counter",
					id = "main",
					props = { step = state.step, label = "Click to count, in Rust" },
				},
			},
			ui.row {
				align = "center",
				ui.input { id = "word", value = state.word, placeholder = "A word to look up", change = "word", grow = true },
				ui.button { "Look up", msg = { look = state.word }, to = "provider:lookup", disabled = state.word == "" },
				ui.text {
					if lookup.word then `{lookup.word}: {lookup.length} letters, in {lookup.took} ms (async, on tokio)` else "Answered async, on tokio",
					tone = "muted",
				},
			},
			ui.text { `The view last said: counted {state.counted}`, tone = "muted" },
		}
	end,
}
