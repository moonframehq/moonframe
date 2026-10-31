-- A diff header: click Unified or Split to switch views, and − or + to
-- zoom (each greys out at its limit; the % resets it). Refresh counts its clicks.
local button_group = use "button-group"

return mod {
	init = function(previous)
		return previous or { view = "unified", scope = "uncommitted", zoom = 100, refreshed = 0 }
	end,

	update = function(state, msg)
		if msg == "refresh" then
			state.refreshed += 1
		elseif msg.select then
			state.view = msg.select
		elseif msg.scope then
			state.scope = msg.scope
		elseif msg.zoom then
			state.zoom = math.clamp(state.zoom + msg.zoom, 50, 200)
		end
	end,

	view = function(state)
		return ui.column {
			gap = "large",
			ui.row {
				gap = "normal",
				align = "center",
				button_group {
					selected = state.view,
					buttons = {
						{ id = "unified", label = "Unified", icon = "rows-2" },
						{ id = "split", label = "Split", icon = "columns-2" },
						{ id = "refresh", icon = "refresh-cw", tooltip = "Refresh", msg = "refresh" },
					} :: { any },
				},
				button_group {
					small = true,
					event = "scope",
					selected = state.scope,
					buttons = {
						{ id = "uncommitted", label = "Uncommitted" },
						{ id = "branch", label = "Branch" },
					},
				},
				button_group {
					small = true,
					buttons = {
						{ icon = "minus", tooltip = "Zoom out", msg = { zoom = -25 }, disabled = state.zoom <= 50 },
						{ label = `{state.zoom}%`, tooltip = "Reset zoom", msg = { zoom = 100 - state.zoom } },
						{ icon = "plus", tooltip = "Zoom in", msg = { zoom = 25 }, disabled = state.zoom >= 200 },
					} :: { any },
				},
			},
			ui.text {
				`{state.view} diff of {state.scope} changes at {state.zoom}%, refreshed {state.refreshed} times`,
				tone = "muted",
				size = "small",
			},
		}
	end,
}
