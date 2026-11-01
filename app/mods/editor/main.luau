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

-- The kinds that open a shell.
local TERMINALS = { ["component:terminal"] = true, ["component:ghostty-terminal"] = true }

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
		local kind = kind_of(app.kinds, state.tab and state.tab.kind)
		local content: any
		if kind and TERMINALS[kind.kind] and not (kind.props and kind.props.cwd) then
			-- A terminal starts in the project the explorer shows; it waits
			-- the moment until the files provider says which, as a shell
			-- started elsewhere would stay there.
			local files = app.files
			if files ~= nil then
				kind = table.clone(kind)
				kind.props = table.clone(kind.props or {})
				kind.props.cwd = if files.available then files.path else "~"
				content = tab_view { tab = state.tab, kind = kind }
			end
		else
			content = tab_view { tab = state.tab, kind = kind }
		end
		return ui.panel {
			-- The strip sits on the card: the selected tab runs on into it
			-- (`.tab-card-joined` in style.luau).
			class = if joined then "tab-card-joined" else nil,
			grow = true,
			gap = "none",
			padding = "none",
			content,
		}
	end,
}
