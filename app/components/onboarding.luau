-- A first-run welcome: a centred card with a header (logo, title, a BETA
-- badge, ×), one page of a tour at a time, pager dots, and Back / Next /
-- Done. Arrow keys and Enter move through it. The mod keeps `current`.

local pager = use "pager"

type Page = {
	icon: string?,
	title: string?,
	text: string?,
	media: any,
	-- Extra elements under the text (a list of tips, buttons).
	content: { any }?,
}

type Props = {
	open: boolean?,
	title: string?,
	icon: string?,
	badge: string?,
	pages: { Page }?,
	current: number?,
	back: any,
	next: any,
	done: any,
	dismiss: any,
	-- Key of the msg a pager dot sends: `{ [event] = index }`.
	event: string?,
	done_label: string?,
	width: number?,
	to: string?,
	[number]: any,
}

return component {
	title = "Onboarding",
	category = "Overlays",
	icon = "rocket",
	description = "A first-run welcome card: logo, title and BETA badge, one tour page at a time, pager dots, Back / Next / Done; arrows and Enter move.",
	when = "A short guided tour or welcome on first launch or after a big update. For one release's changes use `release-notes`; for a single question use `confirm`; for any other modal use `dialog`.",
	props = {
		["open"] = "show it",
		["title"] = "header title; default \"Welcome\"",
		["icon"] = "logo icon in the header; default `sparkles`",
		["badge"] = "a badge after the title (\"BETA\")",
		["pages"] = "list of { icon, title, text, media, content = { elements } }",
		["current"] = "index of the page shown (1-based); default 1",
		["back"] = "msg for Back and ←; default `\"back\"`",
		["next"] = "msg for Next and → / ↩; default `\"next\"`",
		["done"] = "msg for Done (and ↩ on the last page); default `\"done\"`",
		["dismiss"] = "msg for ×, Escape and backdrop clicks; default the `done` msg",
		["event"] = "key of the msg a pager dot sends; default `page`: { page = index }",
		["done_label"] = "default \"Get started\"",
		["width"] = "pixels; default 520",
		["to"] = "mod the buttons send to, if not the drawing one",
		["[children]"] = "the body instead of `pages`",
	},
	styles = {
		[".onboarding"] = { background = "background", border = "all", border_color = "border", radius = 14, padding = 0 },
		[".onboarding-header"] = { padding = { 14, 16 }, gap = 10 },
		[".onboarding-logo"] = { color = "accent", size = "large" },
		[".onboarding-title"] = { weight = "bold" },
		[".onboarding-close"] = { padding = 4, radius = 6, color = "muted-foreground" },
		[".onboarding-close:hover"] = { background = "list-hover", color = "foreground" },
		[".onboarding-body"] = { padding = { 12, 28, 8, 28 }, gap = 10 },
		[".onboarding-page-icon"] = {
			color = "accent",
			size = "title",
			background = "accent/12",
			radius = 14,
			padding = 14,
		},
		[".onboarding-page-title"] = { size = "title", weight = "bold" },
		[".onboarding-page-text"] = { color = "muted-foreground" },
		[".onboarding-media"] = { radius = 10, border = "all", border_color = "border" },
		[".onboarding-footer"] = { padding = { 12, 16 }, gap = 8 },
	},
	example = {
		open = true,
		title = "Welcome to Moonframe",
		icon = "sparkles",
		badge = "BETA",
		current = 2,
		pages = {
			{ icon = "terminal", title = "Terminals that remember", text = "Every workspace keeps its panes, tabs and scrollback across restarts." },
			{
				icon = "bot",
				title = "Run agents side by side",
				text = "Start Claude, Codex or Gemini in their own worktrees and watch them from the sidebar.",
			},
			{ icon = "globe", title = "Your machines, one window", text = "Add SSH hosts and cloud devices; Moonframe reconnects when they drop." },
		},
	},

	build = function(props: Props)
		local pages = props.pages or {}
		local count = math.max(#pages, 1)
		local current = math.clamp(props.current or 1, 1, count)
		local first = current == 1
		local last = current == count
		local back = props.back or "back"
		local next = props.next or "next"
		local done = props.done or "done"
		local dismiss = if props.dismiss ~= nil then props.dismiss else done
		local event = props.event or "page"

		local body: { any } = {}
		if #props > 0 then
			for _, child in ipairs(props) do
				body[#body + 1] = child
			end
		elseif pages[current] then
			local page = pages[current]
			if page.icon then
				body[#body + 1] = ui.row { ui.row { class = "onboarding-page-icon", ui.icon { name = page.icon :: any } } }
			end
			if page.title then
				body[#body + 1] = ui.text { page.title, class = "onboarding-page-title" }
			end
			if page.text then
				body[#body + 1] = ui.text { page.text, class = "onboarding-page-text" }
			end
			if page.media ~= nil then
				body[#body + 1] = ui.column { class = "onboarding-media", page.media }
			end
			for _, element in ipairs(page.content or {}) do
				body[#body + 1] = element
			end
		end

		local keys: { [string]: any } = { escape = dismiss, enter = if last then done else next }
		if not first then
			keys.left = back
		end
		if not last then
			keys.right = next
		end

		return ui.overlay {
			open = props.open,
			dismiss = dismiss,
			place = "center",
			backdrop = true,
			width = props.width or 520,
			keys = keys,
			to = props.to,
			class = "onboarding",
			content = ui.column {
				gap = "none",
				ui.row {
					class = "onboarding-header",
					shrink = false,
					align = "center",
					ui.icon { name = (props.icon or "sparkles") :: any, class = "onboarding-logo" },
					ui.text { props.title or "Welcome", class = "onboarding-title" },
					props.badge ~= nil and ui.badge { props.badge, tone = "accent" },
					ui.spacer(),
					ui.row { class = "onboarding-close", msg = dismiss, to = props.to, tooltip = "Close", ui.icon "close" },
				},
				ui.column { class = "onboarding-body", shrink = false, table.unpack(body) },
				ui.row {
					class = "onboarding-footer",
					shrink = false,
					align = "center",
					pager {
						count = count,
						page = current,
						back = back,
						next = if last then done else next,
						next_label = if last then (props.done_label or "Get started") else "Next",
						event = event,
						to = props.to,
					},
				},
			},
		}
	end,
}
