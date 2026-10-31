-- The window's top row when the layout draws its own title bar: room on the
-- left for macOS's traffic lights, an optional leading slot (a sidebar
-- toggle), back and forward arrows, the content (tabs, a title) and trailing
-- actions. It moves the window when dragged (`drag = true`; double-click
-- zooms) and controls inside still take their clicks. On Linux and Windows
-- with a hidden system title bar it draws minimize, maximize and close.

local nav_buttons = use "nav-buttons"
local icon_button = use "icon-button"
local tabs = use "tabs"

type Window = {
	platform: string?,
	titlebar: string?,
	maximized: boolean?,
	fullscreen: boolean?,
	active: boolean?,
}

type Props = {
	window: Window?,
	inset: number?,
	leading: { any }?,
	back: any,
	forward: any,
	can_back: boolean?,
	can_forward: boolean?,
	title: string?,
	center: boolean?,
	actions: { any }?,
	to: string?,
	[number]: any,
}

-- Lists in the preview mix optional fields; typed loosely for the checker.
local function loose(list: { any }): { any }
	return list
end

return component {
	title = "Title bar",
	category = "Navigation",
	icon = "window-maximize",
	description = "A unified window title bar: traffic-light inset, leading slot, back/forward, tabs or a title, actions, window buttons; drags the window.",
	when = "The top row of a window whose shell sets `titlebar = \"hidden\"`: pass `window = app.window` and it leaves room for macOS's traffic lights or draws its own window buttons elsewhere. Put `tabs` in its content. For a bar along the bottom use `status-bar`; for a path inside a pane use `breadcrumbs`; for a plain row of buttons use `toolbar`.",
	props = {
		["window"] = "`app.window`: the platform and title bar mode decide the inset and the window buttons",
		["inset"] = "left padding for macOS's traffic lights, in pixels (default 76 on macOS with a hidden title bar out of full screen, else 10)",
		["leading"] = "elements right after the inset (a sidebar toggle)",
		["back"] = "msg for a back arrow; giving `back` or `forward` shows both arrows",
		["forward"] = "msg for a forward arrow",
		["can_back"] = "false greys out the back arrow",
		["can_forward"] = "false greys out the forward arrow",
		["title"] = "text shown when there is no content",
		["center"] = "centre the content between the two sides",
		["actions"] = "elements on the right (icon buttons, a branch menu, an avatar)",
		["to"] = "mod to send the arrows' msgs to, if not the drawing one",
		["[children]"] = "the content: tabs, a title, a search field",
	},
	styles = {
		[".title-bar"] = { background = "sidebar", border = "bottom", border_color = "border", padding = { 0, 10 }, gap = 8 },
		[".title-bar-inactive"] = { color = "muted-foreground" },
		[".title-bar-title"] = { weight = "medium", color = "muted-foreground" },
		[".title-bar-controls"] = { gap = 2, padding = { left = 6 } },
		[".title-bar-close:hover"] = { background = "danger", color = "#ffffff" },
	},
	usage = [[local title_bar = use "title-bar"
local tabs = use "tabs"
local icon_button = use "icon-button"

title_bar {
	window = app.window,
	leading = { icon_button { icon = "panel-left", tooltip = "Toggle sidebar", msg = "toggle-sidebar" } },
	back = "back", forward = "forward", can_forward = false,
	actions = { icon_button { icon = "columns-2", tooltip = "Split right", msg = "split" } },
	tabs { selected = state.tab, add = "new-tab", tabs = {
		{ id = "editor", label = "editor", icon = "terminal" },
		{ id = "lazygit", label = "lazygit", icon = "git-branch" },
	} },
}]],
	example = {
		window = { platform = "macos", titlebar = "hidden", maximized = false, fullscreen = false, active = true },
		leading = { icon_button { icon = "panel-left", tooltip = "Toggle sidebar", msg = "toggle-sidebar" } },
		back = "back",
		forward = "forward",
		can_forward = false,
		actions = {
			icon_button { icon = "columns-2", tooltip = "Split right", msg = "split" },
			icon_button { icon = "ellipsis", tooltip = "More", msg = "more" },
			icon_button { icon = "git-branch", tooltip = "feat/multi-currency", msg = "branches" },
			icon_button { icon = "brand-github", tooltip = "ledger-api on GitHub", msg = "github" },
		},
		tabs {
			selected = "editor",
			add = "new-tab",
			tabs = loose {
				{ id = "editor", label = "editor", icon = "terminal", status = "working" },
				{ id = "lazygit", label = "lazygit", icon = "git-branch" },
			},
		},
	},

	build = function(props: Props)
		local window: Window = props.window or ({} :: Window)
		local hidden = window.titlebar == "hidden"
		local mac = window.platform == "macos" or window.platform == nil
		local inset = props.inset
		if inset == nil then
			inset = if mac and hidden and not window.fullscreen then 76 else 10
		end

		local left = {}
		for _, child in ipairs(props.leading or {}) do
			left[#left + 1] = child
		end
		if props.back ~= nil or props.forward ~= nil then
			left[#left + 1] = nav_buttons {
				back = props.back,
				forward = props.forward,
				can_back = props.can_back,
				can_forward = props.can_forward,
				small = true,
				to = props.to,
			}
		end

		local content = {}
		for _, child in ipairs(props) do
			content[#content + 1] = child
		end
		if #content == 0 and props.title then
			content[1] = ui.text { props.title, class = "title-bar-title", truncate = true }
		end

		local right = {}
		for _, child in ipairs(props.actions or {}) do
			right[#right + 1] = child
		end
		if hidden and not mac then
			right[#right + 1] = ui.row {
				class = "title-bar-controls",
				align = "center",
				icon_button { icon = "window-minimize", tooltip = "Minimize", command = "minimize" },
				icon_button {
					icon = if window.maximized then "window-restore" else "window-maximize",
					tooltip = if window.maximized then "Restore" else "Maximize",
					command = "maximize",
				},
				ui.row {
					class = "icon-button title-bar-close",
					width = 28,
					height = 28,
					align = "center",
					justify = "center",
					tooltip = "Close",
					command = "close-window" :: any,
					ui.icon "window-close",
				},
			}
		end

		local classes = { "title-bar" }
		if window.active == false then
			classes[#classes + 1] = "title-bar-inactive"
		end
		return ui.row {
			class = classes,
			height = 40,
			align = "center",
			drag = true,
			shrink = false,
			style = { padding = { left = inset } },
			#left > 0 and ui.row { class = "title-bar-leading", gap = "small", align = "center", table.unpack(left) },
			ui.row {
				class = "title-bar-content",
				grow = true,
				gap = "small",
				align = "center",
				justify = if props.center then "center" else "start",
				table.unpack(content),
			},
			#right > 0 and ui.row { class = "title-bar-actions", gap = "small", align = "center", table.unpack(right) },
		}
	end,
}
