-- Devices: click a header to sort by it (again to reverse), click a row to
-- select it; the switch toggles zebra stripes.
local table_view = use "table"
local switch = use "switch"

local DEVICES = {
	{ id = "studio", name = "mac-studio", os = "macOS 26.1", cpu = 42, memory = 72, seen = 0 },
	{ id = "build", name = "build-01", os = "Ubuntu 24.04", cpu = 31, memory = 48, seen = 60 },
	{ id = "air", name = "macbook-air", os = "macOS 26.0", cpu = 8, memory = 61, seen = 300 },
	{ id = "pi", name = "pi-kitchen", os = "Debian 12", cpu = 3, memory = 22, seen = 7200 },
	{ id = "devbox", name = "devbox", os = "Fedora 41", cpu = 67, memory = 83, seen = 20 },
}

local function ago(seconds: number): string
	if seconds < 30 then
		return "now"
	elseif seconds < 3600 then
		return `{math.floor(seconds / 60)}m`
	end
	return `{math.floor(seconds / 3600)}h`
end

return mod {
	init = function(previous)
		return previous or { sort = "cpu", descending = true, selected = "studio", zebra = true }
	end,

	update = function(state, msg)
		if msg == "toggle-zebra" then
			state.zebra = not state.zebra
		elseif type(msg) == "table" and msg.sort then
			if state.sort == msg.sort then
				state.descending = not state.descending
			else
				state.sort = msg.sort
				state.descending = msg.sort ~= "name" and msg.sort ~= "os"
			end
		elseif type(msg) == "table" and msg.select then
			state.selected = msg.select
		end
	end,

	view = function(state)
		local sorted = table.clone(DEVICES)
		table.sort(sorted, function(a, b)
			local x, y = a[state.sort], b[state.sort]
			if state.descending then
				return x > y
			end
			return x < y
		end)
		local rows = {}
		for index, device in sorted do
			rows[index] = {
				id = device.id,
				name = device.name,
				os = device.os,
				cpu = `{device.cpu}%`,
				memory = `{device.memory}%`,
				seen = ago(device.seen),
			}
		end
		return ui.column {
			gap = "normal",
			ui.row { switch { on = state.zebra, label = "Zebra stripes", small = true, msg = "toggle-zebra" } },
			table_view {
				sort = state.sort,
				descending = state.descending,
				selected = state.selected,
				zebra = state.zebra,
				columns = {
					{ key = "name", title = "Device" },
					{ key = "os", title = "OS", width = 110, muted = true },
					{ key = "cpu", title = "CPU", width = 56, align = "right", mono = true },
					{ key = "memory", title = "Memory", width = 72, align = "right", mono = true },
					{ key = "seen", title = "Seen", width = 56, align = "right", muted = true },
				},
				rows = rows,
			},
		}
	end,
}
