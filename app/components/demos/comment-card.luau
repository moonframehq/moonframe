-- A review thread: click the file:line chip to jump to it; Resolve and
-- Reply are header actions on the first comment. Click a reaction to add
-- or take back yours; the smiley button opens a picker to add another.
local comment_card = use "comment-card"
local emoji_picker = use "emoji-picker"

-- The reactions as the card takes them, with `mine` from who reacted.
local function shown(reactions: { any }): { any }
	local list = {}
	for _, reaction in reactions do
		local count = reaction.others + (if reaction.mine then 1 else 0)
		if count > 0 then
			table.insert(list, { emoji = reaction.emoji, count = count, mine = reaction.mine, tooltip = reaction.tooltip })
		end
	end
	return list
end

return mod {
	init = function(previous)
		return previous
			or {
				opened = nil,
				resolved = false,
				replies = 0,
				picking = false,
				query = "",
				reactions = {
					{ emoji = "👍", others = 2, mine = true, tooltip = "You, bob and carol" },
					{ emoji = "👀", others = 1, mine = false, tooltip = "bob" },
				} :: { any },
			}
	end,

	update = function(state, msg)
		if type(msg) == "table" and msg.open then
			state.opened = msg.open
		elseif msg == "resolve" then
			state.resolved = not state.resolved
		elseif msg == "reply" then
			state.replies += 1
		elseif msg == "add-reaction" then
			state.picking = not state.picking
			state.query = ""
		elseif type(msg) == "table" and msg.value ~= nil then
			state.query = msg.value
		elseif type(msg) == "table" and (msg.react or msg.pick) then
			local emoji = msg.react or msg.pick
			local found: any = nil
			for _, reaction in state.reactions do
				if reaction.emoji == emoji then
					found = reaction
				end
			end
			if found == nil then
				table.insert(state.reactions, { emoji = emoji, others = 0, mine = true })
			elseif msg.pick then
				found.mine = true
			else
				found.mine = not found.mine
			end
			state.picking = false
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
				reactions = shown(state.reactions),
				add_reaction = "add-reaction",
				picker = emoji_picker { recent = { "👍", "🎉", "❤️", "🚀", "👀" }, query = state.query, search = { search = true }, cancel = "add-reaction", focus = true },
				picker_open = state.picking,
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
