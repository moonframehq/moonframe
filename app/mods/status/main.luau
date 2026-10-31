-- The status bar along the bottom.
return mod {
	region = "status-right",

	init = function(previous)
		return previous or { edits = 0 }
	end,

	update = function(state, msg)
		if type(msg) == "table" and msg.edits then
			state.edits = msg.edits
		end
	end,

	view = function(state, app)
		-- `app.git` comes from the git provider (providers/git), a process
		-- the host runs; clicking asks it to look again.
		local git = app.git
		local branch: any = false
		if git and git.repository then
			local extra = ""
			if git.changed > 0 then
				extra ..= ` +{git.changed}`
			end
			if git.ahead > 0 or git.behind > 0 then
				extra ..= ` ↑{git.ahead} ↓{git.behind}`
			end
			branch = ui.row {
				class = "usage-widget",
				align = "center",
				msg = "refresh",
				to = "provider:git",
				ui.text { `⎇ {git.branch}{extra}`, class = "status-text" },
			}
		end
		return ui.row {
			gap = "large",
			align = "center",
			branch,
			ui.text { "Ready", size = "small", tone = "success" },
			ui.text { `{state.edits} edits`, size = "small", tone = "muted" },
			ui.text { app.dark and "Dark" or "Light", size = "small", tone = "muted" },
		}
	end,
}
