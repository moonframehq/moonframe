-- The main area: the content of the tab selected in the title bar's tab
-- strip (`tabbed { id = "main", show = "bar" }` in main.luau). It hears the
-- strip on the `tabs/main` topic, draws that tab with `tab-view`, and sends
-- what the tab reports (a browser's page) back on `tabs/main/report`.
local tab_view = use "tab-view"

type State = {
	-- The selected tab, as the strip publishes it.
	tab: any,
	-- The shell's layout menu choices: { tabs?, layout? }, on `looks`.
	looks: any,
}

local function kind_of(kinds: { any }, kind: string?)
	for _, entry in kinds do
		if entry.kind == kind then
			return entry
		end
	end
	return nil
end

return mod {
	region = "main",
	subscribe = { "tabs/main", "looks" },

	init = function(previous: State?): State
		return { tab = previous and previous.tab, looks = previous and previous.looks }
	end,

	update = function(state: State, msg)
		if type(msg) ~= "table" then
			return
		end
		if msg.topic == "tabs/main" then
			state.tab = msg.value and msg.value.tab
		elseif msg.topic == "looks" then
			state.looks = msg.value
		elseif msg.tab_report then
			return state, { { to = "topic:tabs/main/report", msg = msg } }
		end
	end,

	view = function(state: State, app)
		-- As the shell decides: the title bar over the center (the layout
		-- menu's choice, or always for curved tabs) meets the card.
		local chosen = state.looks or {}
		local tabs = chosen.tabs or (app.looks and app.looks.tabs)
		local joined = tabs == "curved" or chosen.layout == "joined"
		return ui.panel {
			-- The strip sits on the card: the selected tab runs on into it
			-- (`.tab-card-joined` in style.luau).
			class = if joined then "tab-card-joined" else nil,
			grow = true,
			gap = "none",
			padding = "none",
			tab_view { tab = state.tab, kind = kind_of(app.kinds, state.tab and state.tab.kind) },
		}
	end,
}
