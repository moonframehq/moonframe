-- A changelog: click a release's header to expand or collapse it.
local release_notes = use "release-notes"

local RELEASES: { any } = {
	{
		version = "1.4.0",
		date = "October 28, 2026",
		badge = "Latest",
		summary = "Settings get a new home, and remote hosts reconnect on their own.",
		highlights = {
			{ icon = "settings", title = "A settings window", detail = "Every option in one place, grouped by page, with search and per-host overrides marked Local." },
			{ icon = "wifi", title = "Automatic reconnect", detail = "When an SSH host drops, Moonframe retries with backoff and keeps your panes where they were." },
		},
		fixes = {
			"The sidebar no longer jumps when an agent finishes.",
			"Pasting into a terminal with bracketed paste keeps tabs.",
			"Toasts stack upward instead of over each other.",
		},
	},
	{
		version = "1.3.2",
		date = "October 9, 2026",
		fixes = { "Worktree notes survive a rename.", "⌘W closes the focused pane, not the tab." },
	},
	{
		version = "1.3.0",
		date = "September 30, 2026",
		highlights = {
			{ icon = "git-pull-request", title = "Pull requests in the sidebar", detail = "Each branch shows its PR number, review state and checks." },
		},
		fixes = { "Scrollback search finds wrapped lines." },
	},
}

return mod {
	init = function(previous)
		return previous or { open = { ["1.4.0"] = true } }
	end,

	update = function(state, msg)
		if type(msg) == "table" and msg.toggle then
			state.open[msg.toggle] = not state.open[msg.toggle]
		end
	end,

	view = function(state)
		local items = {}
		for _, release in RELEASES do
			items[#items + 1] = release_notes {
				name = "Moonframe",
				version = release.version,
				date = release.date,
				badge = release.badge,
				summary = release.summary,
				highlights = release.highlights,
				fixes = release.fixes,
				collapsed = not state.open[release.version],
				toggle = true,
			}
		end
		return ui.column { gap = "large", table.unpack(items) }
	end,
}
