-- A window's title bar: click a tab, × closes it, + adds one; ← → walk the
-- tabs you visited; the platform switch shows Linux's window buttons.
local title_bar = use "title-bar"
local tabs = use "tabs"
local icon_button = use "icon-button"
local segmented = use "segmented"

return mod {
	init = function(previous)
		return previous
			or {
				platform = "macos",
				sidebar = true,
				branch = "feat/multi-currency",
				next = 3,
				tabs = ({
					{ id = "editor", label = "editor", icon = "terminal", status = "working" },
					{ id = "lazygit", label = "lazygit", icon = "git-branch" },
				} :: { any }),
				history = { "lazygit", "editor" },
				at = 2,
			}
	end,

	update = function(state, msg)
		local function visit(id: string)
			-- Drop the forward history, then add the tab.
			for i = #state.history, state.at + 1, -1 do
				table.remove(state.history, i)
			end
			table.insert(state.history, id)
			state.at = #state.history
		end
		if msg == "new-tab" then
			local id = `shell-{state.next}`
			state.next += 1
			table.insert(state.tabs, { id = id, label = `zsh {state.next - 1}`, icon = "square-terminal" })
			visit(id)
		elseif msg == "back" then
			state.at = math.max(1, state.at - 1)
		elseif msg == "forward" then
			state.at = math.min(#state.history, state.at + 1)
		elseif msg == "branches" then
			state.branch = if state.branch == "main" then "feat/multi-currency" else "main"
		elseif msg == "toggle-sidebar" then
			state.sidebar = not state.sidebar
		elseif typeof(msg) == "table" then
			if msg.platform then
				state.platform = msg.platform
			elseif msg.select then
				visit(msg.select)
			elseif msg.close then
				for i, tab in state.tabs do
					if tab.id == msg.close then
						table.remove(state.tabs, i)
						break
					end
				end
				-- Forget the closed tab in the history.
				local kept = {}
				for _, id in state.history do
					if id ~= msg.close and kept[#kept] ~= id then
						kept[#kept + 1] = id
					end
				end
				if #kept == 0 and state.tabs[1] then
					kept[1] = state.tabs[1].id
				end
				state.history = kept
				state.at = #kept
			end
		end
	end,

	view = function(state)
		local selected = state.history[state.at]
		return ui.column {
			gap = "normal",
			segmented {
				event = "platform",
				selected = state.platform,
				small = true,
				options = { { id = "macos", label = "macOS" }, { id = "linux", label = "Linux" } },
			},
			title_bar {
				window = { platform = state.platform, titlebar = "hidden", maximized = false, fullscreen = false, active = true },
				leading = {
					icon_button { icon = "panel-left", tooltip = "Toggle sidebar", msg = "toggle-sidebar", active = state.sidebar },
				},
				back = "back",
				forward = "forward",
				can_back = state.at > 1,
				can_forward = state.at < #state.history,
				actions = { icon_button { icon = "git-branch", tooltip = state.branch, msg = "branches" } },
				tabs { selected = selected, add = "new-tab", tabs = state.tabs },
			},
			ui.text {
				`Tab: {selected or "none"} · {state.branch} · sidebar {if state.sidebar then "shown" else "hidden"}`,
				size = "small",
				tone = "muted",
			},
		}
	end,
}
