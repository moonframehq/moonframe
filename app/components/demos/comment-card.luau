-- A review thread: click the file:line chip to jump to it; Resolve and
-- Reply are header actions on the first comment.
local comment_card = use "comment-card"

return mod {
	init = function(previous)
		return previous or { opened = nil, resolved = false, replies = 0 }
	end,

	update = function(state, msg)
		if type(msg) == "table" and msg.open then
			state.opened = msg.open
		elseif msg == "resolve" then
			state.resolved = not state.resolved
		elseif msg == "reply" then
			state.replies += 1
		end
	end,

	view = function(state)
		local thread = {
			comment_card {
				author = "alice",
				action = "requested changes",
				age = "3d ago",
				badge = "reviewer",
				location = "src/ledger.rs:26",
				locate = { open = "src/ledger.rs:26" },
				body = "Reject mixing currencies here: return a Result instead of silently grouping.\n\nA builder would also keep the call sites short.",
				actions = {
					ui.button { if state.resolved then "Unresolve" else "Resolve", small = true, variant = "ghost", msg = "resolve" },
					ui.button { "Reply", small = true, variant = "ghost", msg = "reply" },
				},
			},
			comment_card {
				author = "claude",
				badge = "bot",
				seconds = 2 * 3600,
				body = "Done: `balances` now returns `Result<HashMap<Currency, i64>, MixedCurrency>`.",
			},
		}
		for index = 1, state.replies do
			thread[#thread + 1] = comment_card { author = "penso", badge = "author", seconds = 30 * index, body = "Thanks, looks good now." }
		end
		thread[#thread + 1] = ui.text {
			(if state.resolved then "Thread resolved" else "Thread open") .. (if state.opened then ` · opened {state.opened}` else ""),
			tone = "muted",
			size = "small",
		}
		return ui.column { gap = "normal", table.unpack(thread) }
	end,
}
