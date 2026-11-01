-- A framed pane card, as terminal panes are framed: a header with the
-- program's icon, its name, the folder, the agent's status, and buttons to
-- split right, split down, zoom and close; the body under it. `focused`
-- draws the accent border. The header may sit at the bottom instead.

local status_dot = use "status-dot"
local icon_button = use "icon-button"

type Props = {
	title: string,
	icon: string?,
	subtitle: string?,
	status: string?,
	focused: boolean?,
	zoomed: boolean?,
	header: string?,
	focus: any,
	split_right: any,
	split_down: any,
	zoom: any,
	close: any,
	actions: { any }?,
	leading: { any }?,
	menu: any,
	click_focus: boolean?,
	to: string?,
	height: number?,
	[number]: any,
}

return component {
	title = "Pane frame",
	category = "Layout",
	icon = "square-terminal",
	description = "A framed pane card: header with icon, title, folder, status and split/zoom/close buttons over its body; accent border when focused.",
	when = "Framing each pane of a tiled workspace (a terminal, an agent, a log) with its own header and controls. The panes' sizes come from `ui.hsplit`/`ui.vsplit` (see `split-view`); this only draws the frame inside one. For a titled box of content use `card`; for a sidebar panel use `ui.panel`.",
	props = {
		["title"] = "the program or agent's name",
		["icon"] = "icon before the title (agent-claude, terminal, git-branch...)",
		["subtitle"] = "muted text after the title: the folder, `~/code/app`",
		["status"] = "working (spinner), blocked (a \"Needs you\" chip), done or idle (a dot)",
		["focused"] = "accent border: the pane that takes keys",
		["zoomed"] = "the zoom button shows restore",
		["header"] = "\"top\" (default) or \"bottom\"",
		["focus"] = "msg sent when the header's title is clicked",
		["split_right"] = "msg for the split-right button (omit to hide it)",
		["split_down"] = "msg for the split-down button (omit to hide it)",
		["zoom"] = "msg for the zoom button (omit to hide it)",
		["close"] = "msg for the close button (omit to hide it)",
		["actions"] = "more elements before the built-in buttons",
		["leading"] = "elements before the icon (a status dot)",
		["menu"] = "msg for a chevron after the title (a menu of what the pane shows); omit for none",
		["click_focus"] = "a click anywhere in the body that no control takes sends `focus` too",
		["to"] = "mod to send the msgs to, if not the drawing one",
		["height"] = "fixed height in pixels; leave unset to fill the parent",
		["[children]"] = "the body: a `ui.terminal`, a log, any content",
	},
	styles = {
		[".pane-frame"] = { background = "background", border = "all", border_color = "border", radius = 8, gap = 0 },
		[".pane-frame-focused"] = { border_color = "accent/70" },
		[".pane-frame-header"] = { padding = { 3, 6, 3, 10 }, gap = 8, size = "small" },
		[".pane-frame-header-top"] = { border = "bottom", border_color = "border" },
		[".pane-frame-header-bottom"] = { border = "top", border_color = "border" },
		[".pane-frame-focus"] = { gap = 8 },
		[".pane-frame-icon"] = { color = "muted-foreground" },
		[".pane-frame-title"] = { weight = "medium" },
		[".pane-frame-subtitle"] = { color = "muted-foreground", font = "mono" },
		[".pane-frame-body"] = { padding = 8 },
		[".pane-frame-menu"] = { gap = 6, padding = { 2, 4 }, radius = 6 },
		[".pane-frame-menu:hover"] = { background = "list-hover" },
		[".pane-frame-chevron"] = { color = "muted-foreground" },
	},
	usage = [[local pane_frame = use "pane-frame"

pane_frame { title = "claude", icon = "agent-claude", subtitle = "~/code/ledger-api",
	status = "blocked", focused = state.focus == "agent",
	focus = { focus = "agent" }, split_right = { split = "right" }, split_down = { split = "down" },
	zoom = "zoom", close = { close = "agent" },
	ui.terminal { id = "agent", cwd = "~/code/ledger-api" },
}]],
	example = {
		title = "claude",
		icon = "agent-claude",
		subtitle = "~/code/ledger-api",
		status = "blocked",
		focused = true,
		focus = "focus",
		split_right = "split-right",
		split_down = "split-down",
		zoom = "zoom",
		close = "close",
		height = 180,
		ui.text { "running 2 tests", size = "small" },
		ui.text { "test result: ok. 2 passed; 0 failed; finished in 0.00s", size = "small" },
		ui.text { "ledger-api main ›", tone = "accent", size = "small" },
	},

	build = function(props: Props)
		local status = nil
		if props.status == "blocked" then
			status = status_dot { status = "blocked", chip = true }
		elseif props.status == "working" then
			status = status_dot { status = "working", spin = true }
		elseif props.status then
			status = status_dot { status = props.status, size = "small" }
		end

		local buttons = {}
		for _, action in ipairs(props.actions or {}) do
			buttons[#buttons + 1] = action
		end
		local function add(msg: any, icon: string, tooltip: string)
			if msg ~= nil then
				buttons[#buttons + 1] = icon_button { icon = icon, tooltip = tooltip, msg = msg, to = props.to, size = "small" }
			end
		end
		add(props.split_right, "square-split-horizontal", "Split right")
		add(props.split_down, "square-split-vertical", "Split down")
		add(props.zoom, if props.zoomed then "minimize-2" else "maximize-2", if props.zoomed then "Restore" else "Zoom")
		add(props.close, "close", "Close")

		local bottom = props.header == "bottom"
		local header = ui.row {
			class = { "pane-frame-header", if bottom then "pane-frame-header-bottom" else "pane-frame-header-top" },
			align = "center",
			height = 32,
			shrink = false,
			ui.row {
				class = "pane-frame-focus",
				grow = true,
				align = "center",
				msg = props.focus,
				to = props.to,
				props.leading ~= nil and ui.row { gap = "small", align = "center", shrink = false, table.unpack(props.leading) },
				props.icon ~= nil and ui.icon { name = props.icon :: any, class = "pane-frame-icon" },
				if props.menu ~= nil
					then ui.row {
						class = "pane-frame-menu",
						align = "center",
						msg = props.menu,
						to = props.to,
						ui.text { props.title, class = "pane-frame-title", truncate = true },
						ui.icon { name = "chevron-down", class = "pane-frame-chevron", size = "small" },
					}
					else ui.text { props.title, class = "pane-frame-title", truncate = true },
				props.subtitle ~= nil and ui.text { props.subtitle, class = "pane-frame-subtitle", truncate = true },
				ui.spacer(),
				status or false,
			},
			#buttons > 0 and ui.row { class = "pane-frame-actions", gap = "none", align = "center", table.unpack(buttons) },
		}
		local body = ui.column {
			class = "pane-frame-body",
			grow = true,
			msg = if props.click_focus then props.focus else nil,
			to = if props.click_focus then props.to else nil,
			table.unpack(props),
		}

		return ui.column {
			class = if props.focused then "pane-frame pane-frame-focused" else "pane-frame",
			grow = props.height == nil,
			height = props.height,
			if bottom then body else header,
			if bottom then header else body,
		}
	end,
}
