-- A navigation sidebar that folds to an icon rail. The width eases between
-- the two, the icons never move, and everything else (the account's name,
-- section headings, labels, counts, rows without an icon) fades out and in
-- one item after another, top to bottom.
--
-- How it moves is all stylesheet: the column takes the state `full` or
-- `rail` (`.side-nav:full` and `.side-nav:rail` set the widths and how the
-- width eases), and each fading part takes `shown` or `hidden`
-- (`.side-nav-fade:*`). Only each part's `delay`, from its place in the
-- list, is set here, so the fade runs down the list.

type Item = {
	id: string,
	label: string,
	icon: string?,
	-- A colour dot in the icon's place (labels).
	dot: string?,
	-- A number (shown as "99+" past 99) or text on the right.
	count: (number | string)?,
	-- A second, muted line: two-line rows (threads, sessions).
	detail: string?,
	-- Muted text at the top right of a two-line row ("now", "2m").
	meta: string?,
	-- A small coloured mark under `meta` (unread, working).
	mark: string?,
	primary: boolean?,
	tooltip: string?,
	msg: any,
}

type Section = {
	title: string?,
	items: { Item },
}

type Account = {
	name: string,
	initial: string?,
	color: string?,
	badge: string?,
	subtitle: string?,
	menu: any,
}

type Props = {
	collapsed: boolean?,
	selected: string?,
	sections: { Section }?,
	footer: { Item }?,
	account: Account?,
	header: any,
	toggle: any,
	event: string?,
	to: string?,
	stagger: number?,
	height: number?,
}

-- Lists in the preview mix optional fields; typed loosely for the checker.
local function loose(list: { any }): { any }
	return list
end

-- A count as the mail apps show it.
local function count_text(count: (number | string)?): string?
	if type(count) == "number" then
		return if count > 99 then "99+" else tostring(count)
	end
	return count :: string?
end

return component {
	title = "Side Nav",
	category = "Navigation",
	icon = "panel-left-close",
	description = "A sidebar of icon rows under section headings that folds to an icon rail: the width eases, icons stay put, and labels, headings and counts fade out and in one after another.",
	when = "An app's main navigation that the user folds to icons and back (mail folders, views, projects, threads), with an account header and counts. For workspace tiles with status use `rail` beside a `sidebar`; for foldable groups of arbitrary rows use `sidebar`; for a VS Code strip beside a panel use `activity-bar`.",
	props = {
		["collapsed"] = "fold to the icon rail",
		["selected"] = "id of the current item, drawn raised as a card",
		["sections"] = "list of { title?, items = { { id, label, icon? or dot? (a colour), count? (number, 99+ past 99, or text), detail? (a second muted line), meta? (top right text), mark? (a colour dot under meta), primary? (stronger text), tooltip?, msg? } } }",
		["footer"] = "items pinned to the bottom, same shape (Settings, Support)",
		["account"] = "{ name, initial?, color?, badge? (a pill after the name), subtitle?, menu? (msg for the chevron) }: the header; in the rail its avatar unfolds the sidebar",
		["header"] = "an element instead of `account`, drawn as is",
		["toggle"] = "msg the header's fold button (and the rail's avatar) sends; no button without it",
		["event"] = "key of the msg an item sends, default `select`: { select = id }",
		["to"] = "mod to send the msgs to, if not the drawing one",
		["stagger"] = "milliseconds between one item's fade and the next (default 12); 0 fades them together",
		["height"] = "fixed height in pixels; in a row it stretches to the row's height",
	},
	styles = {
		-- The column: its width in each state, and how it eases between them.
		-- Folding waits a beat so the labels are mostly gone before it narrows.
		[".side-nav"] = {
			background = "sidebar",
			border = "right",
			border_color = "border",
			gap = 0,
			clip = true,
			transition = "slide 280ms cubic-bezier(0.2, 0, 0, 1)",
		},
		[".side-nav:full"] = { width = 252 },
		[".side-nav:rail"] = { width = 58, transition = "slide 240ms cubic-bezier(0.4, 0, 0.2, 1) 70ms" },
		-- What fades: in slowly from below, out quickly.
		[".side-nav-fade"] = { transition = "fade 180ms ease-out" },
		[".side-nav-fade:shown"] = { opacity = 1 },
		[".side-nav-fade:hidden"] = { opacity = 0, transition = "fade 90ms ease-in" },

		[".side-nav-head"] = { padding = { 14, 14 }, gap = 10, border = "bottom", border_color = "border" },
		[".side-nav-avatar"] = { radius = 999, weight = "medium" },
		[".side-nav-avatar:hover"] = { border_color = "accent/40" },
		[".side-nav-name"] = { weight = "medium" },
		[".side-nav-badge"] = { background = "muted", color = "muted-foreground", radius = 999, padding = { 1, 8 } },
		[".side-nav-subtitle"] = { color = "muted-foreground", size = "small" },
		[".side-nav-button"] = { radius = 6, padding = 4, color = "muted-foreground" },
		[".side-nav-button:hover"] = { background = "list-hover", color = "foreground" },

		[".side-nav-body"] = { padding = { 10, 8 }, gap = 2 },
		[".side-nav-foot"] = { padding = { 8, 8, 10, 8 }, gap = 2, border = "top", border_color = "border" },
		[".side-nav-title"] = { padding = { 14, 12, 6, 12 }, color = "muted-foreground", size = "small" },
		-- A row: 34 high, the icon 12 in from the row and 20 from the edge, so
		-- it stays put in a 58 wide rail. The border is there but clear, so a
		-- selected row does not shift by a pixel.
		[".side-nav-item"] = {
			height = 34,
			padding = { 0, 11 },
			gap = 10,
			radius = 8,
			border = true,
			border_color = "#00000000",
			color = "foreground/85",
		},
		[".side-nav-item:hover"] = { background = "list-hover", color = "foreground" },
		[".side-nav-item:selected"] = {
			background = "card",
			border_color = "border",
			color = "accent",
			shadow = "0 1 2 #00000010",
		},
		[".side-nav-item-primary"] = { color = "foreground", weight = "medium" },
		[".side-nav-two-line"] = { height = 48, padding = { 0, 11 } },
		[".side-nav-slot"] = { width = 18 },
		[".side-nav-dot"] = { radius = 999 },
		[".side-nav-count"] = { color = "muted-foreground" },
		[".side-nav-detail"] = { color = "muted-foreground", size = "small", font = "mono" },
		[".side-nav-meta"] = { color = "muted-foreground", size = "small" },
		[".side-nav-mark"] = { radius = 999 },
	},
	usage = [[local side_nav = use "side-nav"

side_nav {
	collapsed = state.collapsed, toggle = "fold", selected = state.selected,
	account = { name = "Jay", badge = "Personal", subtitle = "Alex's Workspace", color = "#7c3aed" },
	sections = {
		{ items = {
			{ id = "compose", icon = "square-pen", label = "Compose", primary = true },
			{ id = "search", icon = "search", label = "Search" },
		} },
		{ title = "View", items = {
			{ id = "inbox", icon = "inbox", label = "Inbox", count = 128 },
			{ id = "starred", icon = "star", label = "Starred", count = 2 },
		} },
		{ title = "Label", items = {
			{ id = "work", dot = "#8b5cf6", label = "Work", count = 12 },
		} },
	},
	footer = { { id = "settings", icon = "settings", label = "Settings" } },
}
-- update: "fold" flips state.collapsed; msg.select is the chosen item's id.]],
	example = {
		selected = "calendar",
		height = 560,
		toggle = "fold",
		account = { name = "Jay", initial = "J", badge = "Personal", subtitle = "Alex's Workspace", color = "#7c3aed" },
		sections = loose {
			{
				items = loose {
					{ id = "compose", icon = "square-pen", label = "Compose", primary = true },
					{ id = "search", icon = "search", label = "Search" },
					{ id = "calendar", icon = "calendar-days", label = "Calendar" },
					{ id = "automation", icon = "wand-sparkles", label = "Automation" },
				},
			},
			{
				title = "View",
				items = loose {
					{ id = "inbox", icon = "inbox", label = "Inbox", count = 128 },
					{ id = "starred", icon = "star", label = "Starred", count = 2 },
					{ id = "scheduled", icon = "clock", label = "Scheduled", count = 12 },
					{ id = "drafts", icon = "pencil", label = "Drafts", count = 4 },
				},
			},
			{
				title = "Label",
				items = loose {
					{ id = "l1", dot = "#8b5cf6", label = "Label 1", count = 12 },
					{ id = "l3", dot = "#e07a45", label = "Label 3", count = 11 },
					{ id = "add-label", icon = "plus", label = "Add Label" },
				},
			},
		},
	},

	build = function(props: Props)
		local collapsed = props.collapsed == true
		local fade_state = if collapsed then "hidden" else "shown"
		local stagger = props.stagger or 12
		local event = props.event or "select"
		local to = props.to
		-- The n-th part to fade waits n steps. Folding takes a third of a
		-- step each, so the labels are gone before the column narrows much;
		-- unfolding lets the width lead by 80ms, then runs down the list.
		local order = 0
		local function next_delay(): number
			order += 1
			if collapsed then
				return math.floor((order - 1) * stagger / 3)
			end
			return 80 + (order - 1) * stagger
		end
		-- A row (or column) that fades with the others, in its turn.
		local function fade_props(element: { [any]: any }): any
			element.state = fade_state
			local class = element.class
			element.class = if class then `{class} side-nav-fade` else "side-nav-fade"
			element.style = { delay = next_delay() }
			return element
		end
		local function fading(element: { [any]: any }): any
			return ui.row(fade_props(element))
		end
		local function fading_column(element: { [any]: any }): any
			return ui.column(fade_props(element))
		end

		local function item_row(item: Item): any
			local selected = props.selected ~= nil and item.id == props.selected
			local text = count_text(item.count)
			local leading: any = nil
			if item.icon then
				leading = ui.row {
					class = "side-nav-slot",
					justify = "center",
					shrink = false,
					ui.icon { name = item.icon :: any },
				}
			elseif item.dot then
				leading = ui.row {
					class = "side-nav-slot",
					justify = "center",
					align = "center",
					shrink = false,
					ui.row { class = "side-nav-dot", width = 8, height = 8, style = { background = item.dot } },
				}
			end
			local class = "side-nav-item"
			if item.primary then
				class ..= " side-nav-item-primary"
			end
			local msg = item.msg or { [event] = item.id }
			if leading == nil then
				-- Nothing to keep in the rail: the whole row fades, and takes
				-- no clicks while hidden.
				local two = item.detail ~= nil
				return fading {
					class = if two then `{class} side-nav-two-line` else class,
					align = "center",
					selected = selected,
					msg = if collapsed then nil else msg,
					to = to,
					tooltip = item.tooltip,
					ui.column {
						gap = "none",
						grow = true,
						ui.row {
							align = "center",
							gap = "normal",
							ui.text { item.label, truncate = true },
							ui.spacer(),
							item.meta ~= nil and ui.text { item.meta, class = "side-nav-meta" },
							text ~= nil and ui.text { text, class = "side-nav-count" },
						},
						two and ui.row {
							align = "center",
							gap = "normal",
							ui.text { item.detail :: string, class = "side-nav-detail", truncate = true },
							ui.spacer(),
							item.mark ~= nil and ui.row {
								class = "side-nav-mark",
								width = 6,
								height = 6,
								style = { background = item.mark },
							},
						},
					},
				}
			end
			return ui.row {
				class = class,
				align = "center",
				selected = selected,
				msg = msg,
				to = to,
				-- Folded, the icon is all there is: name it under the pointer.
				tooltip = if collapsed then item.label else item.tooltip,
				leading,
				fading {
					align = "center",
					gap = "normal",
					grow = true,
					ui.text { item.label, truncate = true },
					ui.spacer(),
					text ~= nil and ui.text { text, class = "side-nav-count" },
				},
			}
		end

		local head: any = props.header
		local account = props.account
		if head == nil and account then
			local color = account.color or "accent"
			head = ui.row {
				class = "side-nav-head",
				align = "center",
				shrink = false,
				ui.row {
					class = "side-nav-avatar",
					width = 30,
					height = 30,
					align = "center",
					justify = "center",
					shrink = false,
					style = { background = `{color}/18`, color = color },
					msg = if collapsed then props.toggle else account.menu,
					to = to,
					tooltip = if collapsed and props.toggle ~= nil then "Expand sidebar" else nil,
					ui.text { account.initial or string.upper(string.sub(account.name, 1, 1)) },
				},
				fading_column {
					gap = "none",
					grow = true,
					ui.row {
						align = "center",
						gap = "small",
						ui.text { account.name, class = "side-nav-name", truncate = true },
						account.badge ~= nil and ui.text { account.badge, class = "side-nav-badge" },
						account.menu ~= nil and ui.row {
							class = "side-nav-button",
							msg = account.menu,
							to = to,
							ui.icon { name = "chevron-down", size = "small" },
						},
					},
					account.subtitle ~= nil and ui.text { account.subtitle, class = "side-nav-subtitle", truncate = true },
				},
				props.toggle ~= nil and fading {
					class = "side-nav-button",
					msg = if collapsed then nil else props.toggle,
					to = to,
					tooltip = "Collapse sidebar",
					ui.icon { name = "panel-left-close" },
				},
			}
		end

		local function section_column(section: Section): any
			local children: { any } = {}
			if section.title then
				table.insert(children, fading {
					class = "side-nav-title",
					ui.text { section.title, truncate = true },
				})
			end
			for _, item in section.items do
				table.insert(children, item_row(item))
			end
			return ui.column { gap = "none", shrink = false, table.unpack(children) }
		end

		local body: { any } = {}
		local sections: { Section } = props.sections or {}
		for _, section in sections do
			table.insert(body, section_column(section))
		end
		local foot: any = nil
		if props.footer and #props.footer > 0 then
			local rows = {}
			for _, item in props.footer do
				table.insert(rows, item_row(item))
			end
			foot = ui.column { class = "side-nav-foot", shrink = false, table.unpack(rows) }
		end

		return ui.column {
			class = "side-nav",
			state = if collapsed then "rail" else "full",
			shrink = false,
			height = props.height,
			head or false,
			ui.column { class = "side-nav-body", grow = true, scroll = true, table.unpack(body) },
			foot or false,
		}
	end,
}
