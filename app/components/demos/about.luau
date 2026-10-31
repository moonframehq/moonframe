-- An About box: the links report what they would open, Copy copies the
-- build line, Esc or × closes it; the switch draws it inline.
local about = use "about"
local switch = use "switch"

local OPENED = {
	["open-website"] = "Opening moonframe.dev…",
	["open-notes"] = "Opening the 1.4.0 release notes…",
	report = "Opening a new issue…",
}

return mod {
	init = function(previous)
		return previous or { open = true, inline = false, note = "" }
	end,

	update = function(state, msg)
		if msg == "open" then
			state.open = true
		elseif msg == "close" then
			state.open = false
		elseif msg == "toggle-inline" then
			state.inline = not state.inline
		elseif OPENED[msg] then
			state.note = OPENED[msg]
		end
	end,

	view = function(state)
		return ui.column {
			gap = "normal",
			ui.row {
				gap = "normal",
				align = "center",
				ui.button { "About Moonframe", icon = "info", small = true, variant = "outline", msg = "open" },
				switch { on = state.inline, label = "Inline", small = true, msg = "toggle-inline" },
				state.note ~= "" and ui.text { state.note, tone = "muted", size = "small" },
			},
			about {
				open = state.open or state.inline,
				inline = state.inline,
				dismiss = "close",
				name = "Moonframe",
				version = "1.4.0 (beta 2)",
				tagline = "Terminals, agents and worktrees in one window.",
				icon = "square-terminal",
				info = {
					{ "Commit", "6731aec" },
					{ "Built", "2026-10-28 14:02" },
					{ "Channel", "beta" },
					{ "Platform", "macOS 26 · arm64" },
				},
				links = {
					{ label = "Website", icon = "globe", msg = "open-website" },
					{ label = "Release notes", icon = "file-text", msg = "open-notes" },
					{ label = "Report an issue", icon = "bug", msg = "report" },
				},
				copy = "Moonframe 1.4.0 (beta 2) 6731aec macOS 26 arm64",
				copyright = "© 2026 Fabien Penso",
			},
		}
	end,
}
