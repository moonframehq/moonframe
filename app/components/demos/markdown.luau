-- A chat answer written a few words at a time: each click on "Write more"
-- adds the next words, and the dot follows the end while it is unfinished,
-- as a streamed answer looks. "Restart" starts it over.
local markdown = use "markdown"

local ANSWER = [[I'd call it **Shape-Shifting Software**: software you keep reshaping in plain language, with an agent rewriting it underneath. [Loomwell +1](cite:loomwell)

| Term | Emphasis |
|---|---|
| Clay Software | Software that keeps taking the shape of your needs |
| Live Software | Changes land at once, with no redeploy |

- **Clay Software** — the most memorable.
- **Self-Rewriting Software** — the most literal, and `diff`-friendly.

The key distinction: **you change what the software is.**]]

-- The answer cut into pieces of a few words each, with their spaces.
local PIECES = {}
for piece in string.gmatch(ANSWER, "%s*%S+%s*%S*%s*%S*") do
	table.insert(PIECES, piece)
end

return mod {
	init = function(previous)
		return previous or { shown = 6 }
	end,

	update = function(state, msg)
		if msg == "more" then
			state.shown = math.min(#PIECES, state.shown + 4)
		elseif msg == "restart" then
			state.shown = 1
		elseif msg == "all" then
			state.shown = #PIECES
		end
	end,

	view = function(state)
		local text = table.concat(PIECES, "", 1, state.shown)
		return ui.column {
			width = 560,
			gap = "large",
			ui.row {
				gap = "small",
				ui.button { "Write more", icon = "play", small = true, msg = "more", disabled = state.shown >= #PIECES },
				ui.button { "All", small = true, variant = "ghost", msg = "all" },
				ui.button { "Restart", icon = "rotate-cw", small = true, variant = "ghost", msg = "restart" },
			},
			markdown { text = text, cursor = state.shown < #PIECES },
		}
	end,
}
