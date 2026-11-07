-- A tab bar that keeps its own tabs: how many there are, which is selected,
-- and what each shows. A tab is data, never a file: a kind (a component or
-- mod that offers itself with a `tab` entry, listed in `app.kinds`) and the
-- props it was opened with. The + button opens a menu of kinds; each tab's
-- content is that component, or an instance of that mod, with its props.
-- The tabs are saved and come back at the next launch; a terminal tab
-- starts a fresh shell in its folder, a browser tab reopens its page.
-- It is published on the topic `tabs/<id>` as it starts and at every change,
-- as { count, selected, title, tab } where `tab` is the selected tab's record.
-- With `show = "bar"` it draws only its strip (in a title bar, say) and a mod
-- draws the content from that topic with `tab-view`; what the content reports
-- comes back on `tabs/<id>/report`. A report with `exited = true` (its
-- shell ended, Ctrl-D in a terminal) closes the tab. A terminal's tab is
-- named after its shell's folder or title, and a long command ending in a
-- tab not on show, or a program's notice, posts a notification. A tab not on
-- show has a spinner while a command runs in it, and a dot once it needs the
-- user or a command ended there, until it is shown.
-- Another mod opens a tab by publishing on `tabs/<id>/open`
-- { n, kind, props, title, id? }: a tab of that kind showing the same `path` is
-- selected instead of opened twice. `n` numbers the requests, so one heard
-- again (as a topic is, when the bar loads) opens nothing.

local tabs_strip = use "tabs"
local menu = use "menu"
local tab_view = use "tab-view"

type Tab = {
	id: string,
	-- "component:terminal" or "mod:explorer", as in `app.kinds`.
	kind: string,
	-- Set once the content reports one, such as a page's title.
	title: string?,
	-- The folder its shell is in, as far as the shell says.
	folder: string?,
	-- A command runs in its shell, as far as the shell says.
	running: boolean?,
	-- What happened while it was not on show: "blocked" or "done".
	attention: string?,
	props: { [string]: any },
}

type State = {
	tabs: { Tab },
	selected: string?,
	-- Numbers new tabs' ids.
	next: number,
	-- The + menu is open.
	menu: boolean,
	-- The last `tabs/<id>/open` request handled, by its `n`.
	opened: number?,
}

type Props = {
	id: string?,
	-- The kind of the first tab, and of a new tab when nothing is open.
	start: string?,
	-- Kinds the + menu offers, by `kind`; all of `app.kinds` when unset.
	offer: { string }?,
	-- The strip's look, as `tabs` takes it; else the stylesheet's
	-- `looks.tabs`, else "classic".
	variant: string?,
	-- "bar" draws only the strip; the content is drawn elsewhere, from the
	-- `tabs/<id>` topic. "both" (the default) draws it under the strip.
	show: string?,
}

local function find(list: { any }, key: string, value: any): (any, number?)
	for index, item in list do
		if item[key] == value then
			return item, index
		end
	end
	return nil, nil
end

-- A new tab of `kind`, selected.
local function open(state: State, props: Props, kind: string)
	local id = `{props.id or "main"}-{state.next}`
	state.next += 1
	table.insert(state.tabs, { id = id, kind = kind, props = {} })
	state.selected = id
end

-- A tab another mod asked for: the open one of that kind, path and root
-- (the project a file is in), else a new one with those props.
local function open_asked(state: State, props: Props, asked: any)
	if type(asked) ~= "table" or type(asked.kind) ~= "string" or asked.n == state.opened then
		return false
	end
	state.opened = asked.n
	local wanted = type(asked.props) == "table" and asked.props or {}
	if wanted.path ~= nil then
		for _, tab in state.tabs do
			if tab.kind == asked.kind and tab.props.path == wanted.path and tab.props.root == wanted.root then
				state.selected = tab.id
				return true
			end
		end
	end
	open(state, props, asked.kind)
	local tab = state.tabs[#state.tabs]
	tab.props = table.clone(wanted)
	tab.title = asked.title
	-- The id it asked for, when no tab has it: what the app gives a tab
	-- `moonframe pane new` opens, to know its terminal.
	if type(asked.id) == "string" and asked.id ~= "" and not find(state.tabs, "id", asked.id) then
		tab.id = asked.id
		state.selected = asked.id
	end
	return true
end

-- What the strip shows, published for subscribers: with the selected tab's
-- record, for a `tab-view` drawn elsewhere.
local function report(state: State, props: Props)
	local selected = find(state.tabs, "id", state.selected)
	return {
		to = `topic:tabs/{props.id or "main"}`,
		msg = {
			count = #state.tabs,
			selected = state.selected,
			title = selected and selected.title,
			tab = selected,
		},
	}
end

-- Closes the tab `id`, selecting its neighbour if it was selected.
local function close(state: State, id: string)
	local _, index = find(state.tabs, "id", id)
	if index then
		table.remove(state.tabs, index)
		if state.selected == id then
			local neighbour = state.tabs[math.min(index, #state.tabs)]
			state.selected = neighbour and neighbour.id
		end
	end
end

-- A command ending in a tab not on show is news from this long on.
local LONG_COMMAND_MS = 10000

-- What fits a tab, and what `app:notify` takes. A shell's text can be longer
-- than either, whatever a program printed.
local MAX_TAB_TITLE = 60
local MAX_NOTICE_TITLE = 256
local MAX_NOTICE_BODY = 2048

-- `text` in `limit` bytes at most, cut between characters.
local function clip(text: string, limit: number): string
	if #text <= limit then
		return text
	end
	local at = limit + 1
	while at > 1 and bit32.band(string.byte(text, at), 0xC0) == 0x80 do
		at -= 1
	end
	return string.sub(text, 1, at - 1)
end

-- A notification about a tab.
local function notification(tab: Tab, title: string, body: string?)
	return {
		title = clip(title, MAX_NOTICE_TITLE),
		body = body and clip(body, MAX_NOTICE_BODY),
		id = tab.id,
	}
end

-- The last part of a path, or the path when it is the root.
local function folder_name(path: string): string
	return string.match(path, "([^/]+)/*$") or path
end

-- What a tab's shell reported: whether the tab changed, and a notification
-- to post, if any.
local function keep_shell_event(state: State, tab: Tab, msg): (boolean, any)
	local hidden = tab.id ~= state.selected
	local title
	if msg.event == "folder" then
		title = folder_name(msg.path)
		tab.folder = title
	elseif msg.event == "title" and msg.title ~= "" then
		title = msg.title
	elseif msg.event == "started" then
		tab.running = true
		return hidden, nil
	elseif msg.event == "notice" then
		if hidden then
			tab.attention = "blocked"
		end
		return hidden, notification(tab, msg.title or tab.title or "Terminal", msg.body)
	elseif msg.event == "finished" then
		tab.running = nil
		if not hidden then
			return false, nil
		end
		tab.attention = "done"
		if msg.took_ms < LONG_COMMAND_MS then
			return true, nil
		end
		local failed = msg.status ~= nil and msg.status ~= 0
		-- The folder, not the command or a title a shell may have set to
		-- it: a command line can hold a token, and notifications outlive
		-- the terminal in the system's own list.
		return true, notification(tab, if failed then "Command failed" else "Command finished", tab.folder)
	end
	title = title and clip(title, MAX_TAB_TITLE)
	if title and title ~= tab.title then
		tab.title = title
		return true, nil
	end
	return false, nil
end

-- Keeps what a tab's content reported: a browser's page and title, what a
-- shell does. A shell that exited closes its tab. Returns whether anything
-- changed, and a notification to post, if any.
local function keep_report(state: State, msg): (boolean, any)
	local tab = find(state.tabs, "id", msg.tab_report)
	if not tab then
		return false, nil
	end
	if msg.exited then
		close(state, tab.id)
		return true, nil
	end
	if msg.event then
		return keep_shell_event(state, tab, msg)
	end
	-- The whole report, as the browser takes it back: address, title,
	-- loading and history.
	tab.props.page = {
		url = msg.url,
		title = msg.title,
		loading = msg.loading,
		back = msg.back,
		forward = msg.forward,
	}
	if msg.title and msg.title ~= "" then
		tab.title = msg.title
	end
	return true, nil
end

-- What a tab's status dot says: nothing on the tab on show, which speaks for
-- itself, so a quick command there does not flicker.
local function status_of(state: State, tab: Tab): string?
	if tab.id == state.selected then
		return nil
	end
	return tab.attention or (if tab.running then "working" else nil)
end

-- What to do about a report: the bar announced again only if it changed, so
-- a prompt in an unchanged folder costs nothing.
local function report_effects(state: State, props: Props, changed: boolean, notice: any): { any }
	local effects = {}
	if changed then
		table.insert(effects, report(state, props))
	end
	if notice then
		table.insert(effects, { to = "app:notify", msg = notice })
	end
	return effects
end

return component {
	title = "Tabbed",
	category = "Navigation",
	icon = "gallery-vertical-end",
	description = "A tab bar that keeps its tabs, which are saved: + opens a menu of kinds (terminals, browsers, files, any component or mod with a `tab` entry), and each tab shows its kind.",
	when = "Several terminals, browsers or tools in one place, opened and closed by the user, remembered across launches. For a strip whose tabs the mod keeps itself use `tabs`.",
	props = {
		["id"] = "names this tab bar: its tabs are kept under it, and published on `tabs/<id>`",
		["start"] = "kind of the first tab, as `app.kinds` names it (\"component:terminal\")",
		["offer"] = "kinds the + menu offers; all of `app.kinds` when unset",
		["variant"] = "the strip's look, as `tabs` takes it; by default the stylesheet's `looks.tabs`, else \"classic\"",
		["show"] = "\"bar\" draws only the strip, its content drawn elsewhere from the `tabs/<id>` topic with `tab-view`; \"both\" (default) draws the content under it",
	},
	example = { id = "example", start = "component:terminal" },

	-- Reports for its tabs, from content drawn elsewhere.
	subscribe = { "tabs/*" },

	init = function(previous: State?, props: Props)
		local state: State = previous or { tabs = {}, next = 1, menu = false }
		state.menu = false
		-- Nothing is known of the shells yet: they say again as they run.
		for _, tab in state.tabs do
			tab.running = nil
			tab.attention = nil
		end
		if not previous and props and props.start then
			open(state, props, props.start)
		end
		-- Announced as it starts, so content drawn elsewhere has its tab.
		return state, { report(state, props or {}) }
	end,

	update = function(state: State, msg, props: Props)
		if msg == "add" then
			state.menu = not state.menu
			return
		elseif msg == "close_menu" then
			state.menu = false
			return
		elseif type(msg) ~= "table" then
			return
		end
		if msg.select then
			state.selected = msg.select
			local shown = find(state.tabs, "id", msg.select)
			if shown then
				shown.attention = nil
			end
		elseif msg.pick then
			open(state, props, msg.pick)
			state.menu = false
		elseif msg.open then
			-- A tab of this kind, from a mod rather than the menu: the one
			-- already open if there is one, else a new one.
			local tab = find(state.tabs, "kind", msg.open)
			if tab then
				state.selected = tab.id
			else
				open(state, props, msg.open)
			end
		elseif msg.close_kind then
			-- Every tab of a kind, from a mod about to go away (a mockup).
			local kept = {}
			for _, tab in state.tabs do
				if tab.kind ~= msg.close_kind then
					table.insert(kept, tab)
				end
			end
			if #kept == #state.tabs then
				return
			end
			local _, index = find(state.tabs, "id", state.selected)
			state.tabs = kept
			if not find(kept, "id", state.selected) then
				local neighbour = kept[math.min(index or 1, #kept)]
				state.selected = neighbour and neighbour.id
			end
		elseif msg.close then
			close(state, msg.close)
		elseif msg.tab_report then
			-- What its own content reported: a browser's page and title.
			local changed, notice = keep_report(state, msg)
			local effects = report_effects(state, props, changed, notice)
			if #effects == 0 then
				return
			end
			return state, effects
		elseif msg.topic == `tabs/{props.id or "main"}/open` then
			if not open_asked(state, props, msg.value) then
				return
			end
		elseif msg.topic == `tabs/{props.id or "main"}/report` and type(msg.value) == "table" then
			-- The same, from content drawn elsewhere.
			local changed, notice = keep_report(state, msg.value)
			local effects = report_effects(state, props, changed, notice)
			if #effects == 0 then
				return
			end
			return state, effects
		else
			return
		end
		return state, { report(state, props) }
	end,

	build = function(props: Props, kept: any, app)
		local state: State = kept
		local kinds: { any } = app and app.kinds or {}
		local offered = {}
		for _, kind in kinds do
			if not props.offer or table.find(props.offer, kind.kind) then
				table.insert(offered, { id = kind.kind, label = kind.title, icon = kind.icon })
			end
		end

		local strip = {}
		for _, tab in state.tabs do
			local kind = find(kinds, "kind", tab.kind)
			table.insert(strip, {
				id = tab.id,
				label = tab.title or (kind and kind.title) or tab.kind,
				icon = kind and kind.icon,
				status = status_of(state, tab),
				closable = true,
			})
		end

		-- The + button opens the menu of kinds under itself.
		local bar = tabs_strip {
			tabs = strip,
			selected = state.selected,
			variant = props.variant or (app and app.looks and app.looks.tabs) or "classic",
			add = "add",
			-- The bar it is drawn in has its own line.
			in_bar = props.show == "bar",
			add_menu = { open = state.menu, dismiss = "close_menu", content = menu { items = offered } },
		}

		if props.show == "bar" then
			return bar
		end

		local selected = find(state.tabs, "id", state.selected)
		local body
		if selected then
			body = tab_view { tab = selected, kind = (find(kinds, "kind", selected.kind)) }
		else
			body = ui.column {
				grow = true,
				align = "center",
				justify = "center",
				gap = "small",
				ui.text { "No tabs open", tone = "muted" },
				ui.button { "New tab", msg = "add", small = true },
			}
		end

		return ui.column {
			grow = true,
			gap = "none",
			bar,
			ui.column { grow = true, gap = "none", body },
		}
	end,
}
