-- Agent session tabs: click a tab, × closes it, + adds one. Pick a look
-- above the strip; right-click a tab to see its id.
local tabs = use "tabs"
local segmented = use "segmented"

local NEW: { any } = {
	{ label = "billing-svc", icon = "agent-claude", status = "working" },
	{ label = "infra-tf", icon = "square-terminal", status = "idle" },
	{ label = "web-app", icon = "agent-codex", status = "blocked", badge = 1 },
	{ label = "TODO.md", icon = "file" },
}

return mod {
	init = function(previous)
		return previous
			or {
				variant = "pills",
				selected = "api",
				next = 1,
				menu = nil,
				tabs = {
					{ id = "api", label = "ledger-api", icon = "square-terminal", status = "working" },
					{ id = "docs", label = "harbor-docs", icon = "agent-codex", status = "blocked", badge = 2 },
					{ id = "forge", label = "pixel-forge", icon = "agent-claude", status = "done" },
					{ id = "notes", label = "notes.md", icon = "file", status = "idle" },
				},
			}
	end,

	update = function(state, msg)
		if msg == "new-tab" then
			local template = NEW[(state.next - 1) % #NEW + 1]
			local tab = table.clone(template)
			tab.id = `new-{state.next}`
			state.next += 1
			table.insert(state.tabs, tab)
			state.selected = tab.id
		elseif msg.variant then
			state.variant = msg.variant
		elseif msg.select then
			state.selected = msg.select
		elseif msg.tab_menu then
			state.menu = msg.tab_menu
		elseif msg.close then
			for index, tab in state.tabs do
				if tab.id == msg.close then
					table.remove(state.tabs, index)
					if state.selected == msg.close then
						local next = state.tabs[index] or state.tabs[index - 1]
						state.selected = next and next.id
					end
					break
				end
			end
		end
	end,

	view = function(state)
		return ui.column {
			gap = "large",
			segmented {
				small = true,
				event = "variant",
				selected = state.variant,
				options = {
					{ id = "pills", label = "Pills" },
					{ id = "underline", label = "Underline" },
					{ id = "outline", label = "Outline" },
					{ id = "cards", label = "Cards" },
					{ id = "compact", label = "Compact" },
					{ id = "classic", label = "Classic" },
				},
			},
			tabs {
				variant = state.variant,
				selected = state.selected,
				add = "new-tab",
				tabs = state.tabs,
			},
			ui.text {
				if state.menu then `Right-clicked: {state.menu}` else `{#state.tabs} tabs open`,
				tone = "muted",
				size = "small",
			},
		}
	end,
}
