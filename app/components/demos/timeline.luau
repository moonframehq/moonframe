-- Checkpoints: click one to select it and see its details; New checkpoint
-- adds one at the top.
local timeline = use "timeline"

return mod {
	init = function(previous)
		return previous
			or {
				selected = "c4",
				next = 6,
				entries = {
					{ id = "c5", day = "Today", time = "14:32", title = "checkpoint-5", detail = "Balance per currency", added = 38, removed = 16, tone = "accent" },
					{ id = "c4", day = "Today", time = "13:05", title = "checkpoint-4", detail = "Posting struct replaces tuples", added = 12, removed = 4, tone = "accent" },
					{ id = "c3", day = "Today", time = "11:47", title = "checkpoint-3", detail = "Tests for rounding", added = 64 },
					{ id = "c2", day = "Yesterday", time = "18:20", title = "checkpoint-2", detail = "Agent stopped: needs input", tone = "warning" },
					{ id = "c1", day = "Yesterday", time = "17:02", title = "checkpoint-1", detail = "Initial worktree", added = 3, removed = 1 },
				} :: { any },
			}
	end,

	update = function(state, msg)
		if msg == "add" then
			local id = `c{state.next}`
			table.insert(state.entries, 1, {
				id = id,
				day = "Today",
				time = `15:{string.format("%02d", (state.next * 7) % 60)}`,
				title = `checkpoint-{state.next}`,
				detail = "Tests pass for multi-currency books",
				added = 5 * state.next,
				removed = state.next,
				tone = "success",
			})
			state.next += 1
			state.selected = id
		elseif type(msg) == "table" and msg.select then
			state.selected = msg.select
		end
	end,

	view = function(state)
		local chosen
		for _, entry in state.entries do
			if entry.id == state.selected then
				chosen = entry
			end
		end
		return ui.column {
			gap = "normal",
			ui.row { ui.button { "New checkpoint", icon = "plus", small = true, variant = "outline", msg = "add" } },
			timeline { selected = state.selected, entries = state.entries },
			chosen ~= nil and ui.text { `{chosen.title}: {chosen.detail}`, tone = "muted", size = "small" },
		}
	end,
}
