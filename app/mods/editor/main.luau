-- The main area: the content of the tab selected in the title bar's tab
-- strip (`tabbed { id = "main", show = "bar" }` in main.luau). It hears the
-- strip on the `tabs/main` topic, draws that tab with `tab-view`, and sends
-- what the tab reports (a browser's page) back on `tabs/main/report`.
-- A markdown file chosen in the explorer opens in a tab of its own (asked
-- for on `tabs/main/open`), with the project it is in (`root`), so the tab
-- keeps that file when the project moves; a link in one opens the file it
-- names in the same project, or a browser tab for a web address.
local tab_view = use "tab-view"

type State = {
	-- The selected tab, as the strip publishes it.
	tab: any,
	-- The shell's layout menu choices: { tabs?, layout? }, on `looks`.
	looks: any,
	-- Numbers the tabs it asks the strip for.
	asked: number,
}

-- The kinds that open a shell.
local TERMINALS = { ["component:terminal"] = true, ["component:ghostty-terminal"] = true }

-- Files that open in the markdown editor.
local function is_markdown(path: string): boolean
	local lower = string.lower(path)
	return string.match(lower, "%.md$") ~= nil
		or string.match(lower, "%.markdown$") ~= nil
		or string.match(lower, "%.mdx$") ~= nil
end

-- The file a relative link in `from` names, from the project's root, or nil
-- when it climbs out of it.
local function resolve(from: string, link: string): string?
	local parts = {}
	for part in string.gmatch(string.match(from, "^(.*)/") or "", "[^/]+") do
		table.insert(parts, part)
	end
	if string.sub(link, 1, 1) == "/" then
		parts = {}
	end
	for part in string.gmatch(link, "[^/]+") do
		if part == ".." then
			if #parts == 0 then
				return nil
			end
			table.remove(parts)
		elseif part ~= "." then
			table.insert(parts, part)
		end
	end
	return if #parts > 0 then table.concat(parts, "/") else nil
end

-- Asks the main strip for a tab.
local function ask(state: State, kind: string, props: { [string]: any }, title: string?)
	state.asked += 1
	return state, { { to = "topic:tabs/main/open", msg = { n = state.asked, kind = kind, props = props, title = title } } }
end

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
		return {
			tab = previous and previous.tab,
			looks = previous and previous.looks,
			asked = previous and previous.asked or 0,
		}
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
		elseif type(msg.open) == "string" and is_markdown(msg.open) then
			-- From the explorer.
			local root = if type(msg.root) == "string" then msg.root else nil
			return ask(
				state,
				"component:markdown-editor",
				{ path = msg.open, root = root },
				string.match(msg.open, "([^/]+)$")
			)
		elseif type(msg.open_link) == "string" then
			-- A link in a markdown tab: a web page, or another file.
			local url: string = msg.open_link
			if string.match(url, "^https?://") then
				return ask(state, "component:browser", { url = url })
			end
			if string.match(url, "^%a[%w+.-]*:") or string.sub(url, 1, 1) == "#" then
				return -- mailto:, an anchor in the page: nothing to open here.
			end
			local path = resolve(msg.from or "", (string.gsub(url, "[?#].*$", "")))
			if path and is_markdown(path) then
				local root = if type(msg.root) == "string" then msg.root else nil
				return ask(
					state,
					"component:markdown-editor",
					{ path = path, root = root },
					string.match(path, "([^/]+)$")
				)
			end
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
			-- A terminal starts in the project the explorer shows (that of
			-- the terminal last used, else the folder `moonframe` was
			-- started from; else home); it waits
			-- the moment until the files provider says which, as a shell
			-- started elsewhere would stay there.
			local files = app.files
			if files ~= nil then
				kind = table.clone(kind)
				kind.props = table.clone(kind.props or {})
				kind.props.cwd = if files.available then files.cwd or files.path else nil
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
