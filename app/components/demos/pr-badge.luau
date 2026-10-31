-- Every PR state as a badge: click one to select it; the segmented control
-- sets the checks dot and the switch shows GitLab's "!" numbers.
local pr_badge = use "pr-badge"
local segmented = use "segmented"
local switch = use "switch"

local TONES = {
	{ tone = "draft", number = 839 },
	{ tone = "waiting", number = 847 },
	{ tone = "ready", number = 850, label = "Ready" },
	{ tone = "attention", number = 842 },
	{ tone = "merged", number = 831 },
	{ tone = "closed", number = 812 },
	{ tone = "held", number = 805 },
}

return mod {
	init = function(previous)
		return previous or { selected = 850, checks = "passed", mr = false }
	end,

	update = function(state, msg)
		if msg == "toggle-mr" then
			state.mr = not state.mr
		elseif type(msg) == "table" and msg.open then
			state.selected = msg.open
		elseif type(msg) == "table" and msg.checks then
			state.checks = msg.checks
		end
	end,

	view = function(state)
		local badges = {}
		local chosen = ""
		for index, entry in TONES do
			badges[index] = pr_badge {
				number = entry.number,
				kind = if state.mr then "mr" else "pr",
				tone = entry.tone,
				label = entry.label,
				checks = if state.checks == "none" then nil else state.checks,
				msg = { open = entry.number },
			}
			if entry.number == state.selected then
				chosen = entry.tone
			end
		end
		return ui.column {
			gap = "normal",
			ui.row {
				gap = "normal",
				align = "center",
				segmented {
					selected = state.checks,
					small = true,
					event = "checks",
					options = {
						{ id = "none", label = "No checks" },
						{ id = "passed", label = "Passed" },
						{ id = "pending", label = "Pending" },
						{ id = "failed", label = "Failed" },
					},
				},
				switch { on = state.mr, label = "GitLab", small = true, msg = "toggle-mr" },
			},
			ui.row { gap = "small", wrap = true, table.unpack(badges) },
			ui.text { `Selected {if state.mr then "!" else "#"}{state.selected} ({chosen})`, tone = "muted", size = "small" },
		}
	end,
}
