-- The title row: room for the traffic lights, search and compose over the
-- sidebar, then back and forward, history, the mail tab, and the open tabs
-- spread across the rest. Emails open as tabs (the mail mod publishes them
-- on `tidings/tabs`); the calendar, note and loading tabs are SAMPLE tabs of
-- the other places, not in this example. Mounted by the shell with
-- `left`, whether the sidebar shows.

local icon_button = use "icon-button"

type Props = { left: boolean? }
type State = { threads: { string }, current: string }

-- Tabs of the other places, after the emails'.
local OTHERS: { any } = {
	{ id = "tokens", label = "Dark mode tokens for v3.4", loading = true, dot = true },
	{ id = "demo", label = "Thursday demo", icon = "calendar" },
	{ id = "planning", label = "Q4 planning", icon = "file" },
}

return mod {
	title = "Title row",
	subscribe = { "tidings/tabs" },

	styles = {
		[".bar"] = { padding = { 0, 10, 0, 0 }, gap = 0 },
		[".bar-side"] = { padding = { 0, 12, 0, 0 }, gap = 2 },
		[".bar-nav"] = { padding = { 0, 6, 0, 12 }, gap = 4 },
		[".bar-home"] = { radius = 9, padding = { 5, 9 } },
		[".bar-home:hover"] = { background = "chrome" },
		[".bar-home:selected"] = { background = "chrome", shadow = "0 1px 2px #0000000d" },
		[".bar-rule"] = { background = "#00000014" },
		[".tab"] = { radius = 9, padding = { 5, 10 }, gap = 8, color = "ink" },
		[".tab:hover"] = { background = "#ffffff59" },
		[".tab:selected"] = { background = "chrome", color = "foreground", shadow = "0 1px 2px #0000000d" },
		[".tab-icon"] = { color = "muted-foreground" },
		[".tab-dot"] = { background = "unread", radius = 3 },
		[".tab-close"] = { radius = 5, padding = 2, color = "muted-foreground" },
		[".tab-close:hover"] = { background = "list-hover", color = "foreground" },
	},

	init = function(): State
		return { threads = {}, current = "" }
	end,

	update = function(state: State, msg: any)
		if type(msg) == "table" and msg.topic == "tidings/tabs" and msg.value then
			state.threads = msg.value.threads or {}
			state.current = msg.value.current or ""
		end
	end,

	view = function(state: State, app, props: Props?)
		local window = app.window
		local left = props == nil or props.left ~= false
		local lights = window.platform == "macos" and window.titlebar == "hidden" and not window.fullscreen
		local subjects: { [string]: string } = {}
		for _, thread in (app.mailbox and app.mailbox.threads) or {} do
			subjects[thread.id] = thread.subject
		end

		local tabs: { any } = {
			ui.row {
				class = "tab",
				grow = true,
				align = "center",
				msg = { home = true },
				to = "mail",
				tooltip = "Mail",
				ui.icon { name = "inbox", class = "tab-icon" },
				ui.text { "Inbox", truncate = true },
			},
		}
		for _, id in state.threads do
			local current = id == state.current
			table.insert(tabs, ui.row {
				class = "tab",
				key = id,
				grow = true,
				align = "center",
				selected = current,
				msg = { pick_thread = id },
				to = "mail",
				ui.icon { name = "mail", class = "tab-icon" },
				ui.text { subjects[id] or id, truncate = true },
				ui.spacer(),
				current and ui.row {
					class = "tab-close",
					msg = { close_tab = id },
					to = "mail",
					tooltip = "Close",
					ui.icon { name = "close", size = "small" },
				},
			})
		end
		for _, other in OTHERS do
			table.insert(tabs, ui.row {
				class = "tab",
				grow = true,
				align = "center",
				tooltip = "Not in this example",
				other.dot and ui.row { class = "tab-dot", width = 6, height = 6 },
				if other.loading
					then ui.icon { name = "loader-circle", spin = true, tone = "accent" }
					else ui.icon { name = other.icon :: any, class = "tab-icon" },
				ui.text { other.label, truncate = true },
			})
		end

		return ui.row {
			class = "bar",
			drag = true,
			grow = true,
			align = "center",
			ui.row {
				class = "bar-side",
				width = if left then 232 else (if lights then 84 else 12),
				shrink = false,
				align = "center",
				justify = "end",
				left and icon_button { icon = "search", tooltip = "Search (/)", msg = "search", to = "mail" },
				left and icon_button { icon = "square-pen", tooltip = "New message (C)", msg = "compose", to = "mail" },
			},
			ui.row {
				class = "bar-nav",
				shrink = false,
				align = "center",
				icon_button { icon = "chevron-left", tooltip = "Back (U)", msg = "back", to = "mail" },
				icon_button { icon = "chevron-right", tooltip = "Forward", msg = "noop", disabled = true },
				icon_button { icon = "history", tooltip = "Recently opened (not in this example)", msg = "noop" },
				ui.row {
					class = "bar-home",
					align = "center",
					selected = state.current == "",
					msg = { home = true },
					to = "mail",
					tooltip = "Mail",
					ui.icon { name = "mail" },
				},
				ui.row { class = "bar-rule", width = 1, height = 18 },
			},
			ui.row { grow = true, align = "center", gap = "small", table.unpack(tabs) },
			icon_button { icon = "plus", tooltip = "New tab (not in this example)", msg = "noop" },
		}
	end,
}
