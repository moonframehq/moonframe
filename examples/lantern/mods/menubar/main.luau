-- Lantern in the menu bar: a flame with the CPU in use beside it, the
-- busiest processes in its menu, and ⌃⌥L from any app to bring Lantern
-- forward. It follows the `procs` provider (`provider/procs`), and asks
-- the app again only when what the item shows has changed. Nothing in
-- the window draws it.

local HOTKEY = "ctrl-alt-l"

type State = { shown: string }

-- The item for `procs` (nil until it first publishes), and a key that
-- changes when what it shows does.
local function item(procs: any): (any, string)
	local items: { any } = {}
	local title: string? = nil
	if procs and procs.available then
		title = `{math.floor(procs.cpu + 0.5)}%`
		table.insert(items, { label = "Busiest processes" })
		for at, group in procs.groups do
			if at > 3 then
				break
			end
			table.insert(items, { label = string.format("%s · %d%%", group.name, math.floor(group.cpu + 0.5)), msg = "processes" })
		end
		table.insert(items, "-")
	end
	table.insert(items, { label = "Show Lantern", msg = "show", shortcut = HOTKEY })
	local labels = {}
	for _, line in items do
		table.insert(labels, if type(line) == "table" then line.label else line)
	end
	return {
		symbol = "flame",
		title = title,
		tooltip = "Lantern: CPU in use",
		items = items,
	}, `{title or ""}|{table.concat(labels, "|")}`
end

return mod {
	subscribe = { "provider/procs" },

	init = function(): (State, { any })
		local look, shown = item(nil)
		return { shown = shown }, {
			{ to = "app:status-item", msg = look },
			{ to = "app:hotkey", msg = { keys = HOTKEY, msg = "show" } },
		}
	end,

	update = function(state: State, msg: any)
		if msg == "show" then
			return state, { { to = "app:command", msg = "activate" } }
		elseif msg == "processes" then
			local sends: { any } = {
				{ to = "app:command", msg = "activate" },
				{ to = "workspace", msg = { kind = "processes" } },
			}
			return state, sends
		elseif type(msg) == "table" and msg.topic == "provider/procs" then
			local look, shown = item(msg.value)
			if shown ~= state.shown then
				state.shown = shown
				return state, { { to = "app:status-item", msg = look } }
			end
		end
		return state
	end,

	view = function()
		return ui.column {}
	end,
}
