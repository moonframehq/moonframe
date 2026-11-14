-- The project's bar across the card: its name and star, a search field in
-- the middle, then who is in the project, Share and more. Share offers the
-- sheet as it stands, as tab-separated text, to the system's share menu
-- (`app:share`): Mail, Notes, Messages.

local avatar = use "avatar"
local icon_button = use "icon-button"
local kbd = use "kbd"

type State = { query: string, focus: boolean, starred: boolean, shared: string? }

local KEYS = { "apy", "fee", "deposit", "rating" }

-- The sheet as far as the agent has written it, as tab-separated text:
-- what Share hands the menu. Nil until it has found an app.
local function sheet_text(data: any): string?
	local run = data and data.run
	if not run or run.found == 0 then
		return nil
	end
	local lines = { "Company\tType\tAPY\tMonthly fee\tMin. deposit\tApp rating\tSource" }
	for index = 1, run.found do
		local item = data.apps[index]
		local cells = { item.name, item.type }
		for c, key in KEYS do
			local written = run.step > 2 or (run.step == 2 and (index < run.row or (index == run.row and c <= run.cell)))
			table.insert(cells, if written then tostring(item[key]) else "")
		end
		table.insert(cells, item.source)
		table.insert(lines, table.concat(cells, "\t"))
	end
	return table.concat(lines, "\n")
end

return mod {
	region = "top",
	title = "Top bar",

	styles = {
		[".bar"] = { padding = { 0, 14, 0, 16 }, gap = 8 },
		[".bar-name"] = { weight = "medium" },
		[".bar-star"] = { color = "muted-foreground" },
		[".bar-star-on"] = { color = "warning" },
		[".bar-search"] = { background = "secondary", radius = 10, padding = { 0, 12 }, gap = 8 },
		[".bar-agent"] = { background = "accent", color = "#ffffff", radius = 6 },
		[".bar-face"] = { background = "background", radius = 13, padding = 1 },
		[".bar-share"] = { border = true, border_color = "border", radius = 17, padding = { 0, 14 }, gap = 6 },
		[".bar-share:hover"] = { background = "secondary" },
	},

	init = function(previous: State?): State
		return { query = previous and previous.query or "", focus = false, starred = previous ~= nil and previous.starred or false }
	end,

	update = function(state: State, msg)
		if msg == "focus" then
			state.focus = true
		elseif msg == "star" then
			state.starred = not state.starred
		elseif type(msg) == "table" and msg.msg == "query" then
			state.query = msg.value
			state.focus = false
		elseif type(msg) == "table" and msg.msg == "clear" then
			state.query = ""
		elseif type(msg) == "table" and msg.shared then
			state.shared = msg.service
		end
	end,

	view = function(state: State, app)
		local data = app.research
		local sheet = sheet_text(data)
		return ui.row {
			class = "bar",
			drag = true,
			grow = true,
			height = 54,
			align = "center",
			ui.row {
				width = 320,
				align = "center",
				gap = "normal",
				ui.icon { name = "folder", class = "quiet" },
				ui.text { if data then data.project else "Project", class = "bar-name", truncate = true },
				ui.row {
					msg = "star",
					tooltip = if state.starred then "Starred" else "Star",
					ui.icon { name = if state.starred then "star-fill" else "star", class = if state.starred then "bar-star-on" else "bar-star" },
				},
			},
			ui.spacer(),
			ui.row {
				class = "bar-search",
				width = 290,
				height = 34,
				align = "center",
				ui.icon { name = "search", size = "small", class = "quiet" },
				ui.input {
					id = "search",
					value = state.query,
					placeholder = "Search in project",
					change = "query",
					keys = { escape = "clear" },
					focus = state.focus,
					grow = true,
					class = "bare-input",
				},
				kbd { keys = "cmd-k", small = true, plain = true },
			},
			ui.spacer(),
			ui.row {
				width = 320,
				align = "center",
				gap = "normal",
				justify = "end",
				ui.row {
					class = "bar-agent",
					width = 20,
					height = 20,
					align = "center",
					justify = "center",
					tooltip = "The agent is in this project",
					ui.icon { name = "bot", size = "small" },
				},
				ui.row {
					width = 60,
					height = 26,
					tooltip = "Noor, Theo and Ines are in this project",
					ui.row { float = { top = 0, left = 0 }, class = "bar-face", avatar { name = "Noor Haddad", initial = "N", shape = "circle", color = "#c08457" } },
					ui.row { float = { top = 0, left = 17 }, class = "bar-face", avatar { name = "Theo Marsh", initial = "T", shape = "circle", color = "#4b5563" } },
					ui.row { float = { top = 0, left = 34 }, class = "bar-face", avatar { name = "Ines Okafor", initial = "I", shape = "circle", color = "#7c5cff" } },
				},
				ui.row {
					class = "bar-share",
					height = 34,
					align = "center",
					to = if sheet then "app:share" else nil,
					msg = if sheet then { text = sheet, reply = { shared = true } } else nil,
					tooltip = if not sheet
						then "Nothing to share yet"
						elseif state.shared then `Share the sheet (last with {state.shared})`
						else "Share the sheet as text",
					ui.icon { name = "share", size = "small" },
					ui.text { "Share", weight = "medium" },
				},
				icon_button { icon = "ellipsis", tooltip = "More", msg = "star" },
			},
		}
	end,
}
