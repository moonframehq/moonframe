-- A pull request in a sheet: the button opens it, × or Esc closes it,
-- Approve / Request changes set the review. Pick the edge it slides from.
local sheet = use "sheet"
local segmented = use "segmented"

return mod {
	init = function(previous)
		return previous or { open = true, side = "right", review = nil :: string? }
	end,

	update = function(state, msg)
		if msg == "open" then
			state.open = true
		elseif msg == "close" then
			state.open = false
		elseif msg == "approve" then
			state.review = "Approved"
			state.open = false
		elseif msg == "changes" then
			state.review = "Changes requested"
			state.open = false
		elseif type(msg) == "table" and msg.select then
			state.side = msg.select
		end
	end,

	view = function(state)
		return ui.column {
			gap = "normal",
			ui.row {
				gap = "normal",
				align = "center",
				ui.button { "Open #482", icon = "git-pull-request", variant = "outline", msg = "open" },
				segmented {
					selected = state.side,
					small = true,
					options = {
						{ id = "left", label = "Left" },
						{ id = "right", label = "Right" },
						{ id = "bottom", label = "Bottom" },
					},
				},
			},
			state.review ~= nil and ui.text { `#482: {state.review}`, tone = "muted", size = "small" },
			sheet {
				open = state.open,
				dismiss = "close",
				side = state.side,
				title = "Pull request #482",
				subtitle = "feature/inspect → main",
				icon = "git-pull-request",
				footer = ui.row {
					gap = "small",
					ui.spacer(),
					ui.button { "Request changes", small = true, variant = "outline", msg = "changes" },
					ui.button { "Approve", small = true, variant = "primary", msg = "approve" },
				},
				ui.text { "Add an element inspector to the shell", weight = "medium" },
				ui.text {
					"Picks any element under the pointer and shows its mod, classes and resolved style.",
					size = "small",
					tone = "muted",
				},
				ui.row {
					gap = "small",
					ui.badge { "checks passing", tone = "success" },
					ui.badge { "2 reviews", tone = "accent" },
					ui.badge { "+412 −38", tone = "muted" },
				},
			},
		}
	end,
}
