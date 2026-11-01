-- The gallery: a card per example app (from the native demos provider), with its
-- screenshot, title and what it is. A click opens the app as its own
-- window, or brings it forward when it is open; Open all opens every one,
-- to compare them side by side.

local WIDTH = 380

local function card(app: any): any
	local open = app.open == true
	return ui.column {
		key = app.name,
		class = if open then { "demo-card", "demo-card-open" } else "demo-card",
		width = WIDTH,
		gap = "none",
		msg = { open = app.name },
		to = "provider:demos",
		tooltip = if open then `Bring {app.title} forward` else `Open {app.title}`,
		if app.picture
			then ui.image { src = app.picture, width = WIDTH, height = 236, fit = "cover", class = "demo-picture" }
			else ui.column { class = "demo-picture demo-blank", width = WIDTH, height = 236, align = "center", justify = "center", ui.icon { "app-window" } },
		ui.column {
			class = "demo-body",
			gap = "small",
			ui.row {
				align = "center",
				ui.text { app.title, weight = "bold", size = "large" },
				ui.spacer(),
				open and ui.row { class = "demo-open", align = "center", gap = "small", ui.row { class = "demo-dot" }, ui.text { "Open", size = "small" } },
			},
			ui.text { app.description ~= "" and app.description or app.folder, tone = "muted", size = "small", lines = 3 },
			ui.row {
				align = "center",
				ui.text { `--profile {app.name}`, tone = "muted", size = "small", class = "demo-profile" },
				ui.spacer(),
				open and ui.button { "Close", msg = { close = app.name }, to = "provider:demos" },
			},
		},
	}
end

return mod {
	region = "main",
	title = "Demos",

	view = function(_, app)
		local demos = app.demos
		local apps: { any } = (demos and demos.apps) or {}
		local open = 0
		for _, entry in apps do
			if entry.open then
				open += 1
			end
		end

		local cards = {}
		for _, entry in apps do
			table.insert(cards, card(entry))
		end

		local summary = if demos == nil
			then "Looking for apps…"
			elseif not demos.available then demos.reason
			else `{#apps} apps · {open} open · profiles in {demos.user}`

		return ui.column {
			grow = true,
			gap = "none",
			class = "demo-page",
			ui.row {
				class = "demo-header",
				drag = true,
				align = "center",
				ui.column {
					gap = "none",
					ui.text { "Moonframe demos", size = "title", weight = "bold" },
					ui.text { summary, tone = "muted", size = "small" },
				},
				ui.spacer(),
				open > 0 and ui.button { "Close all", msg = { close_all = true }, to = "provider:demos" },
				ui.button { "Open all", variant = "primary", msg = { open_all = true }, to = "provider:demos" },
			},
			ui.column {
				grow = true,
				scroll = true,
				class = "demo-grid",
				-- Its full height, not the grid's, so the grid has something to scroll.
				ui.row { wrap = true, gap = "large", shrink = false, table.unpack(cards) },
			},
		}
	end,

	styles = {
		[".demo-page"] = { background = "sidebar" },
		[".demo-header"] = { padding = { 18, 28, 14, 92 } },
		[".demo-grid"] = { padding = { 8, 28, 28, 28 } },
		[".demo-card"] = {
			background = "card",
			border = "all",
			border_color = "border",
			radius = 14,
			shadow = "0 6px 18px -10px #00000040",
			transition = "normal",
		},
		[".demo-card:hover"] = { border_color = "accent", shadow = "0 12px 28px -12px #00000059" },
		[".demo-card-open"] = { border_color = "accent/60" },
		[".demo-picture"] = { radius = 13, background = "muted" },
		[".demo-blank"] = { color = "muted-foreground" },
		[".demo-body"] = { padding = { 12, 16, 14, 16 } },
		[".demo-open"] = { color = "success" },
		[".demo-dot"] = { width = 7, height = 7, radius = 4, background = "success" },
		[".demo-profile"] = { font = "mono" },
	},
}
