-- A dashboard row: click a tile to select it as the filter; the tiles show
-- deltas that are good when up (tokens) or good when down (failing checks).
local stat_tile = use "stat-tile"

local TILES = {
	{ id = "tokens", label = "Tokens today", icon = "activity", value = "1.84M", delta = "+12%", detail = "vs. yesterday · 3 agents" },
	{ id = "prs", label = "Open PRs", icon = "git-pull-request", value = "14", delta = "+2", detail = "4 waiting for review" },
	{ id = "failing", label = "Failing checks", icon = "circle-x", value = "3", delta = "+1", good = "down", detail = "ci / clippy on 2 branches" },
	{ id = "latency", label = "Daemon latency", icon = "clock", value = "38", unit = "ms", delta = "−6ms", good = "down", detail = "p95 over 24h" },
}

return mod {
	init = function(previous)
		return previous or { selected = "tokens" }
	end,

	update = function(state, msg)
		if type(msg) == "table" and msg.filter then
			state.selected = if state.selected == msg.filter then nil else msg.filter
		end
	end,

	view = function(state)
		local tiles = {}
		for index, tile in TILES do
			tiles[index] = stat_tile {
				label = tile.label,
				icon = tile.icon,
				value = tile.value,
				unit = tile.unit,
				delta = tile.delta,
				good = tile.good,
				detail = tile.detail,
				width = 220,
				selected = state.selected == tile.id,
				msg = { filter = tile.id },
			}
		end
		return ui.column {
			gap = "normal",
			ui.row { gap = "normal", wrap = true, table.unpack(tiles) },
			ui.text { if state.selected then `Filter: {state.selected}` else "No filter", tone = "muted", size = "small" },
		}
	end,
}
