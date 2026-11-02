-- A scanner's output settings, packed for a sidebar: labels in one
-- column, controls in the other, a hint under the file name.
local form_grid = use "form-grid"
local dropdown = use "dropdown"
local number_field = use "number-field"
local checkbox = use "checkbox"

return mod {
	init = function(previous)
		return previous or { name = "scan", counter = 7, kind = "tiff16", ppi = 300, open = false, sharpen = true }
	end,

	update = function(state, msg)
		if typeof(msg) ~= "table" then
			if msg == "toggle-kind" then
				state.open = not state.open
			elseif msg == "close-kind" then
				state.open = false
			elseif msg == "sharpen" then
				state.sharpen = not state.sharpen
			end
			return
		end
		if msg.pick then
			state.kind = msg.pick
			state.open = false
		elseif msg.name then
			state.name = msg.value
		elseif msg.field then
			state.ppi = tonumber(msg.value) or state.ppi
		end
	end,

	view = function(state)
		return ui.column {
			width = 320,
			form_grid {
				label_width = 80,
				rows = ({
					{
						label = "Name",
						control = ui.input { id = "name", value = state.name, change = { name = true }, width = 180 },
						hint = `Saved as {state.name}-{string.format("%03d", state.counter)}.tif`,
					},
					{
						label = "Format",
						control = dropdown {
							open = state.open,
							toggle = "toggle-kind",
							dismiss = "close-kind",
							selected = state.kind,
							options = {
								{ id = "tiff16", label = "TIFF, 16-bit" },
								{ id = "tiff8", label = "TIFF, 8-bit" },
								{ id = "jpeg", label = "JPEG" },
							},
						},
					},
					{ label = "Resolution", control = number_field { id = "ppi", value = state.ppi, unit = "ppi", change = { field = "ppi" } } },
					{ control = checkbox { label = "Sharpen on save", checked = state.sharpen, msg = "sharpen" } },
				} :: { any }),
			},
		}
	end,
}
