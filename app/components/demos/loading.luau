-- Three ways to wait: pick inline, block or skeleton; the stepper sets how
-- many skeleton bars.
local loading = use "loading"
local segmented = use "segmented"
local stepper = use "stepper"

return mod {
	init = function(previous)
		return previous or { kind = "block", bars = 4 }
	end,

	update = function(state, msg)
		if type(msg) ~= "table" then
			return
		end
		if msg.select then
			state.kind = msg.select
		elseif msg.step then
			state.bars = math.clamp(state.bars + msg.step, 1, 8)
		end
	end,

	view = function(state)
		local shown
		if state.kind == "inline" then
			shown = loading { text = "Fetching pull requests…" }
		elseif state.kind == "block" then
			shown = loading { block = true, text = "Connecting to devbox", detail = "ssh penso@devbox.local" }
		else
			shown = loading { skeleton = true, count = state.bars }
		end
		return ui.column {
			gap = "normal",
			ui.row {
				gap = "normal",
				align = "center",
				segmented {
					selected = state.kind,
					small = true,
					options = {
						{ id = "inline", label = "Inline" },
						{ id = "block", label = "Block" },
						{ id = "skeleton", label = "Skeleton" },
					},
				},
				state.kind == "skeleton" and stepper { label = "Bars", value = state.bars, min = 1, max = 8 },
			},
			ui.column { height = 160, shown },
		}
	end,
}
