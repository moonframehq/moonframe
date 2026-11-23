-- One provider crate, run both ways: `app.focus` comes from the wasm build
-- of app/providers/focus, `app["focus-native"]` from the same crate linked
-- into the app (src/main.rs). The two panels are the same code over each.

local progress = use "progress"

type Focus = {
	clock: string,
	left: number,
	length: number,
	running: boolean,
	sessions: number,
	build: string,
}

-- A timer's panel; `name` is the provider its buttons talk to.
local function timer(title: string, name: string, focus: Focus?)
	local to = `provider:{name}`
	if not focus then
		return ui.panel {
			title = title,
			grow = true,
			ui.text { "Starting…", tone = "muted" },
		}
	end
	return ui.panel {
		title = title,
		grow = true,
		actions = { ui.badge { focus.build } },
		ui.column {
			gap = "normal",
			ui.text { focus.clock, size = "large", weight = "bold" },
			progress {
				value = 1 - focus.left / focus.length,
				label = if focus.running then "Focusing" else "Paused",
				detail = `{focus.sessions} done`,
			},
			ui.row {
				gap = "small",
				ui.button { if focus.running then "Pause" else "Start", msg = if focus.running then "pause" else "start", to = to },
				ui.button { "Reset", msg = "reset", to = to },
				ui.button { "1 min", small = true, msg = { minutes = 1 }, to = to },
				ui.button { "25 min", small = true, msg = { minutes = 25 }, to = to },
			},
		},
	}
end

return mod {
	about = {
		version = "0.1",
		description = "One provider crate, built to wasm and compiled in.",
	},

	view = function(_, app)
		return ui.column {
			grow = true,
			padding = "large",
			gap = "normal",
			ui.text { "Portable Provider", size = "large", weight = "bold" },
			ui.text {
				"The same Rust crate, providers/focus, run as a sandboxed wasm component and linked into the app.",
				tone = "muted",
			},
			ui.row {
				grow = true,
				gap = "normal",
				timer("Wasm: app.focus", "focus", app.focus),
				timer('Native: app["focus-native"]', "focus-native", app["focus-native"]),
			},
		}
	end,
}
