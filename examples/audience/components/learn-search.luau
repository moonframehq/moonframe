-- Learn's guide search, in the page's header: the field, and as you type
-- its results in a popover (guides, then lessons, then help articles),
-- with up, down, Enter and Escape. Its messages go to the learn mod.
-- Inkwell only.

local kbd = use "kbd"
local key_hints = use "key-hints"

-- The learn mod's state (mods/learn/main.luau has its fields).
type State = any

type Props = { state: State, data: any }

local function category_of(data: any, id: string): any
	for _, category in data.categories do
		if category.id == id then
			return category
		end
	end
	return { id = id, label = id, color = "#7c5cff" }
end

-- The text with its first match of `query` in bold: a search result's line.
local function marked(text: string, query: string, class: string)
	local first, last = string.find(string.lower(text), string.lower(query), 1, true)
	if query == "" or not first then
		return ui.text { text, class = class, truncate = true }
	end
	return ui.row {
		gap = "none",
		ui.text { string.sub(text, 1, first - 1), class = class },
		ui.text { string.sub(text, first, last), class = { class, "search-match" } },
		ui.text { string.sub(text, (last :: number) + 1), class = class, truncate = true },
	}
end

local function matches(text: string?, query: string): boolean
	return text ~= nil and string.find(string.lower(text), string.lower(query), 1, true) ~= nil
end

-- What the search finds: guides, then lessons, then help articles.
local function search(data: any, query: string): { any }
	local results = {}
	if query == "" then
		return results
	end
	for _, guide in data.guides do
		if matches(guide.title, query) or matches(guide.summary, query) then
			table.insert(results, { kind = "guide", id = guide.id, title = guide.title, detail = guide.summary, icon = guide.icon, color = category_of(data, guide.category).color, right = `{guide.minutes} min` })
		end
	end
	for index, lesson in data.course.lessons do
		if matches(lesson.title, query) or matches(lesson.summary, query) then
			table.insert(results, { kind = "lesson", id = index, title = `Lesson {index} · {lesson.title}`, icon = "circle-play", color = "#7c5cff", right = lesson.time })
		end
	end
	for _, article in data.help do
		if matches(article.title, query) then
			table.insert(results, { kind = "help", title = article.title, icon = "circle-help", color = "#82828a", right = article.where })
		end
	end
	return results
end

-- The message that opens a search result.
local function open_result(result: any, data: any): any
	if result.kind == "guide" then
		return { guide = result.id, title = result.title }
	elseif result.kind == "lesson" then
		return { lesson = result.id, title = data.course.title }
	end
	return "search-close"
end

return component {
	title = "Learn Search",
	category = "Inkwell",
	icon = "search",
	description = "The Learn page's guide search, with its results in a popover.",
	when = "The header of Inkwell's Learn page. Elsewhere use `search-field`.",
	props = {
		["state"] = "the learn mod's state",
		["data"] = "`app.inkwell.learn`, the catalogue",
	},
	example = {
		state = { query = "", hi = 1, searching = false, filter = "all", sort = "popular", sorting = false, all = false, completed = 1, lesson = 1, watched = 0.5, playing = false, tab = "overview", bookmarked = false, guide = "welcome", read = {}, registered = {}, calendar = true, recording = true, progress_open = false },
		data = {
			categories = { { id = "growth", label = "Growth", color = "#7c5cff" } },
			guides = { { id = "welcome", title = "Your welcome email", summary = "Greet new readers.", category = "growth", icon = "sparkles", minutes = 5, rank = 1, steps = { "Say hello", "Set expectations" } } },
			help = {},
			course = { title = "Newsletter foundations", meta = "8 lessons", activity = { 1, 2, 3 }, resources = { { title = "Checklist", detail = "PDF", icon = "file-text", color = "#e5484d", action = "download" } }, lessons = { { title = "Write a welcome email", hook = "First impressions", chart = "Opens by day", seconds = 300, time = "5:00", kind = "Video", summary = "Why the first email matters.", takeaways = { "Say what to expect" } } } },
			stats = { lessons = 3, lessons_total = 8, streak = 2, certificates = 0, courses = 1 },
			weeks = { 1, 0, 2 },
			weeks_from = "Feb",
			enrolled = { { title = "Newsletter foundations", total = 8, done = 1, color = "#7c5cff" } },
			sessions = { { id = "live-1", title = "Grow with referrals", where = "Live on Zoom", date = "Thursday, 14 May", month = "May", day = "14", when = "Thu 6pm", host = "Ada Park", role = "Editor", people = { "Ben", "Cy" }, registered = 12, length = "45 min" } },
		},
	},

	build = function(props: Props)
		local state = props.state
		local data = props.data

		local results = search(data, state.query)
		local hi = math.clamp(state.hi, 1, math.max(#results, 1))
		local open_msg = if results[hi] then { open = open_result(results[hi], data) } else "search-close"

		local found = {}
		local last_kind
		local headings = { guide = "Guides", lesson = "Lessons", help = "Help articles" }
		for index, result in results do
			if result.kind ~= last_kind then
				last_kind = result.kind
				table.insert(found, ui.text { headings[result.kind], class = "search-group" })
			end
			local here = index == hi
			table.insert(found, ui.row {
				class = "search-row",
				align = "center",
				selected = here,
				msg = { open = open_result(result, data) },
				ui.row {
					class = "search-tile",
					width = 30,
					height = 30,
					shrink = false,
					align = "center",
					justify = "center",
					style = { background = `{result.color}/14`, color = result.color },
					ui.icon { name = result.icon :: any, size = "small" },
				},
				ui.column {
					grow = true,
					gap = "none",
					marked(result.title, state.query, "search-title"),
					result.detail and marked(result.detail, state.query, "search-detail"),
				},
				ui.text { result.right, class = "small-muted" },
				if result.kind == "help" then ui.icon { name = "external-link", size = "small", class = "muted" } elseif here then kbd { keys = "enter", small = true } else nil,
			})
		end
		local popover_body = ui.column {
			class = "search-pop",
			#results == 0 and ui.column {
				class = "placeholder",
				align = "center",
				ui.icon { name = "search", tone = "muted" },
				ui.text { `Nothing matches “{state.query}”`, class = "muted" },
			},
			table.unpack(found),
		}
		local search_field = ui.popover {
			open = state.searching,
			dismiss = "search-close",
			side = "bottom",
			align = "end",
			width = 440,
			class = "search-popover",
			content = ui.column {
				gap = "none",
				popover_body,
				ui.row {
					class = "search-foot",
					align = "center",
					key_hints {
						caps = true,
						separator = "",
						hints = { { keys = "up down", label = "to navigate" }, { keys = "enter", label = "to open" }, { keys = "escape", label = "to close" } },
					},
					ui.spacer(),
					ui.text { `{#results} result{if #results == 1 then "" else "s"}`, class = "small-muted" },
				},
			},
			ui.row {
				class = "search",
				state = if state.searching then "active" else nil,
				width = if state.searching then 440 else 300,
				height = 38,
				align = "center",
				ui.icon { name = "search", size = "small", class = "muted" },
				ui.input {
					id = "guides",
					value = state.query,
					placeholder = "Search guides",
					change = "search",
					grow = true,
					class = "search-input",
					-- Enter opens the highlighted result: its message is built here.
					keys = { up = "search-up", down = "search-down", escape = "search-close", enter = open_msg },
				},
				if state.query ~= ""
					then ui.row { class = "file-action", msg = "search-close", tooltip = "Clear", ui.icon { name = "close", size = "small" } }
					else kbd { keys = { "/" }, small = true },
			},
		}
		return search_field
	end,
}
