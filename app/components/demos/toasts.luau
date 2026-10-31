-- A toast stack: the buttons push a toast, × dismisses one, Clear removes all.
local toasts = use "toasts"

local KINDS = {
	success = { level = "success", source = "moonframe", title = "Worktree created", body = "feature/inspect at ~/code/app-inspect" },
	warning = {
		level = "warning",
		source = "Local",
		title = "Local Git and PR details unavailable",
		body = "The daemon socket directory must be owned by your user and not world-writable.",
		action = { label = "Reconnect", msg = "reconnect" },
	},
	error = { level = "error", icon = true, source = "devbox", title = "Agent exited with code 1", body = "cargo test failed in crates/moonframe-core" },
}

local function push(state: any, kind: string)
	state.next += 1
	local item = table.clone(KINDS[kind])
	item.id = state.next
	table.insert(state.items, item)
end

return mod {
	init = function(previous)
		if previous then
			return previous
		end
		local state = { items = {} :: { any }, next = 0, note = "" }
		push(state, "success")
		push(state, "warning")
		return state
	end,

	update = function(state, msg)
		if type(msg) == "table" and msg.dismiss then
			for index, item in state.items do
				if item.id == msg.dismiss then
					table.remove(state.items, index)
					break
				end
			end
		elseif type(msg) == "table" and msg.push then
			push(state, msg.push)
		elseif msg == "clear" then
			state.items = {}
		elseif msg == "reconnect" then
			state.note = "Reconnecting to the daemon…"
		end
	end,

	view = function(state)
		return ui.column {
			gap = "normal",
			ui.row {
				gap = "small",
				ui.button { "Success", small = true, variant = "outline", msg = { push = "success" } },
				ui.button { "Warning", small = true, variant = "outline", msg = { push = "warning" } },
				ui.button { "Error", small = true, variant = "outline", msg = { push = "error" } },
				ui.button { "Clear", small = true, variant = "ghost", msg = "clear" },
			},
			ui.text { `{#state.items} toasts{if state.note ~= "" then " · " .. state.note else ""}`, tone = "muted", size = "small" },
			toasts { items = state.items, max = 3 },
		}
	end,
}
