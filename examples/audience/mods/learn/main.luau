-- The Learn page and its flow. Home: a header with a guide search (results
-- in a popover as you type) and My progress (a sheet), the featured course,
-- the guides by category with their progress, and on the right your
-- progress, lessons per week and live sessions (Register opens a card).
-- From there: a course's lesson (a player, the overview, transcript and
-- resources, the syllabus beside it, Mark as complete) and a guide's page.
-- The catalogue is `app.inkwell.learn`, sample data from the provider;
-- what you finish, read and register for is this mod's own state.

local course_card = use "course-card"
local guide_card = use "guide-card"
local heat_grid = use "heat-grid"
local segmented = use "segmented"
local dropdown = use "dropdown"
local tabs = use "tabs"
local kbd = use "kbd"
local key_hints = use "key-hints"
local switch = use "switch"
local avatar = use "avatar"
local avatar_stack = use "avatar-stack"
local numbered_list = use "numbered-list"
local sheet = use "sheet"

type State = {
	page: string,
	screen: string, -- "home", "lesson" or "guide"
	title: string?, -- the open course's or guide's name, for the crumbs
	filter: string,
	sort: string,
	sorting: boolean,
	all: boolean,
	query: string,
	searching: boolean,
	hi: number,
	completed: number, -- lessons 1 to `completed` are done
	lesson: number, -- the lesson shown
	watched: number, -- how far into the current lesson, 0 to 1
	playing: boolean,
	tab: string,
	bookmarked: boolean,
	guide: string?,
	read: { [string]: number }, -- guides read since the sample data
	registering: string?,
	registered: { [string]: boolean },
	calendar: boolean,
	recording: boolean,
	progress_open: boolean,
}

local SORTS: { any } = {
	{ id = "popular", label = "Most popular" },
	{ id = "newest", label = "Newest" },
	{ id = "shortest", label = "Shortest" },
}

local function sort_label(id: string): string
	for _, sort in SORTS do
		if sort.id == id then
			return sort.label
		end
	end
	return "Most popular"
end

-- The crumbs the top bar shows: Learn › the open course or guide.
local function crumbs(state: State)
	return { to = "topic:inkwell/learn", msg = { screen = state.screen, title = state.title } }
end

local function clock(seconds: number): string
	local whole = math.floor(seconds)
	return string.format("%02d:%02d", whole // 60, whole % 60)
end

local function category_of(data: any, id: string): any
	for _, category in data.categories do
		if category.id == id then
			return category
		end
	end
	return { id = id, label = id, color = "#7c5cff" }
end

local function guide_by_id(data: any, id: string?): any
	for _, guide in data.guides do
		if guide.id == id then
			return guide
		end
	end
	return nil
end

local function read_of(state: State, guide: any): number
	return state.read[guide.id] or guide.progress or 0
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

return mod {
	region = "main",
	title = "Learn",
	subscribe = { "inkwell/page" },

	styles = {
		[".learn"] = { gap = 0 },
		[".learn-head"] = { padding = { 22, 22, 18, 24 }, gap = 10 },
		[".learn-title"] = { size = "title", weight = "medium" },
		[".muted"] = { color = "muted-foreground" },
		[".small-muted"] = { size = "small", color = "muted-foreground" },
		[".heading"] = { size = "large", weight = "medium" },
		[".eyebrow"] = { size = "small", color = "muted-foreground", case = "upper" },
		[".rule"] = { background = "border" },
		[".pill-button"] = { radius = 999, border = true, border_color = "border", background = "background", padding = { 7, 14 }, gap = 7 },
		[".pill-button:hover"] = { background = "list-hover" },
		[".dark-button"] = { radius = 999, background = "ink", color = "#ffffff", padding = { 8, 16 }, gap = 8, shadow = "0 4px 10px -4px #00000066" },
		[".dark-button:hover"] = { background = "ink/88" },
		[".course-card-primary"] = { background = "ink", color = "#ffffff" },
		[".course-card-primary:hover"] = { background = "ink/88" },
		[".course-card-primary-icon"] = { background = "#ffffff26" },

		-- The search field and its results.
		[".search"] = { radius = 10, border = true, border_color = "border", background = "background", padding = { 0, 6, 0, 10 }, gap = 6 },
		[".search:active"] = { border_color = "accent" },
		[".search-input"] = { background = "#00000000", border = "none", shadow = "none", padding = { 5, 0 } },
		[".search-popover"] = { padding = { 6, 0, 0, 0 } },
		[".search-pop"] = { gap = 0 },
		[".search-group"] = { size = "small", color = "muted-foreground", case = "upper", padding = { 10, 12, 6, 12 } },
		[".search-row"] = { radius = 8, padding = { 7, 8 }, gap = 10, margin = { 0, 6 } },
		[".search-row:hover"] = { background = "list-hover" },
		[".search-row:selected"] = { background = "muted-foreground/10" },
		[".search-tile"] = { radius = 7 },
		[".search-title"] = { color = "foreground" },
		[".search-detail"] = { size = "small", color = "muted-foreground" },
		[".search-match"] = { weight = "bold" },
		[".search-foot"] = { border = "top", border_color = "border", padding = { 6, 12 }, margin = { top = 6 }, background = "muted-foreground/4", radius = { 0, 0, 12, 12 } },

		-- Guides.
		[".body"] = { gap = 0, padding = { top = 26 } },
		[".guides"] = { padding = { 0, 24, 24, 24 }, gap = 16 },
		[".side"] = { border = "left", border_color = "border", padding = { 0, 24, 24, 24 }, gap = 16 },
		[".stat-box"] = { radius = 10, border = true, border_color = "border", padding = { 9, 12 }, gap = 6 },
		[".stat-value"] = { size = "large", weight = "medium" },

		-- Live sessions.
		[".session"] = { padding = { 10, 8 }, gap = 12, radius = 10 },
		[".session:selected"] = { background = "muted-foreground/7" },
		[".date"] = { radius = 10, border = true, border_color = "border", background = "background", padding = { 4, 0 }, gap = 0, shadow = "0 1px 2px #0000000a" },
		[".date-month"] = { size = "small", color = "muted-foreground", case = "upper" },
		[".date-day"] = { size = "large", weight = "medium" },
		[".register"] = { radius = 9, border = true, border_color = "border", background = "background", padding = { 6, 12 }, gap = 6 },
		[".register:hover"] = { background = "list-hover" },
		[".registered"] = { color = "success", border_color = "success/40" },
		[".card-popover"] = { padding = { 0, 0, 0, 0 } },
		[".card-pop"] = { gap = 0 },
		[".card-pop-head"] = { padding = { 16, 18, 12, 18 }, gap = 12 },
		[".card-pop-tile"] = { radius = 9, background = "accent/12", color = "accent" },
		[".card-pop-date"] = { radius = 9, border = true, border_color = "border", background = "muted-foreground/5", padding = { 8, 12 }, gap = 10, margin = { 0, 18 } },
		[".card-pop-host"] = { padding = { 14, 18, 4, 18 }, gap = 10 },
		[".card-pop-people"] = { padding = { 8, 18, 14, 18 }, gap = 10 },
		[".card-pop-option"] = { padding = { 8, 18 }, gap = 10 },
		[".card-pop-foot"] = { border = "top", border_color = "border", padding = { 12, 18 }, gap = 8, margin = { top = 8 } },

		-- The lesson.
		[".lesson-main"] = { padding = 24, gap = 18 },
		[".player"] = { background = "#1d1d21", radius = 16, gap = 0, color = "#ffffff" },
		[".player-text"] = { padding = { 34, 34, 0, 34 }, gap = 8 },
		[".player-eyebrow"] = { size = "small", case = "upper", color = "#ffffff99" },
		[".player-hook"] = { size = "title", weight = "medium", color = "#ffffff" },
		[".player-caption"] = { size = "small", color = "#ffffff99" },
		[".player-play"] = { background = "#ffffff", color = "#1d1d21", radius = 999, shadow = "0 6px 20px #00000055" },
		[".player-play:hover"] = { background = "#ffffffe6" },
		[".player-scrub"] = { padding = { 14, 24, 0, 24 } },
		[".player-knob"] = { background = "#ffffff", radius = 999, shadow = "0 1px 3px #00000066" },
		[".player-bar"] = { padding = { 12, 24, 16, 24 }, gap = 16 },
		[".player-control"] = { color = "#ffffff", radius = 6, padding = 3 },
		[".player-control:hover"] = { background = "#ffffff1a" },
		[".player-time"] = { font = "mono", color = "#ffffff" },
		[".player-cc"] = { border = true, border_color = "#ffffffcc", radius = 4, padding = { 0, 4 }, size = "small", weight = "bold" },
		[".icon-box"] = { radius = 9, border = true, border_color = "border", background = "background", padding = 8 },
		[".icon-box:hover"] = { background = "list-hover" },
		[".icon-box:selected"] = { color = "accent", border_color = "accent/40" },
		[".prose"] = { color = "foreground/80" },
		[".takeaway"] = { gap = 10 },
		[".step-card"] = { radius = 12, border = true, border_color = "border", padding = { 12, 18 }, gap = 12 },
		[".step-card:hover"] = { background = "list-hover" },
		[".transcript-at"] = { font = "mono", color = "accent", size = "small" },
		[".syllabus"] = { border = "left", border_color = "border", gap = 0 },
		[".syllabus-body"] = { padding = { 24, 24, 16, 24 }, gap = 14 },
		[".syllabus-track"] = { background = "muted-foreground/14", radius = 999 },
		[".syllabus-fill"] = { background = "accent", radius = 999 },
		[".lesson-row"] = { radius = 10, padding = { 9, 10 }, gap = 12, border = true, border_color = "#00000000" },
		[".lesson-row:hover"] = { background = "list-hover" },
		[".lesson-row:selected"] = { background = "muted-foreground/6", border_color = "border" },
		[".lesson-mark"] = { radius = 999 },
		[".lesson-done"] = { background = "#14a38b1f", color = "#14a38b" },
		[".lesson-now"] = { background = "accent/15", color = "accent" },
		[".lesson-locked"] = { background = "muted-foreground/12", color = "muted-foreground" },
		[".lesson-now-text"] = { size = "small", color = "accent" },
		[".file-tile"] = { radius = 8, border = true, border_color = "border", background = "background" },
		[".file-row"] = { gap = 12, padding = { 6, 0 } },
		[".file-action"] = { color = "muted-foreground", radius = 6, padding = 4 },
		[".file-action:hover"] = { background = "list-hover", color = "foreground" },
		[".cert"] = { radius = 12, border = true, border_color = "border", padding = 14, gap = 12, background = "muted-foreground/3" },
		[".cert-tile"] = { radius = 9, background = "#14a38b1f", color = "#14a38b" },
		[".syllabus-foot"] = { border = "top", border_color = "border", padding = { 14, 24 }, gap = 10 },

		-- A guide.
		[".guide-band"] = { radius = 16, gap = 0 },
		[".guide-icon"] = { size = "title" },
		[".back"] = { gap = 6, color = "muted-foreground", radius = 6, padding = { 2, 4 } },
		[".back:hover"] = { color = "foreground" },
		[".chip"] = { radius = 999, padding = { 2, 10 }, gap = 6 },
		[".placeholder"] = { padding = 40, gap = 6 },

		-- My progress.
		[".course-row"] = { gap = 8, padding = { 12, 0 }, border = "bottom", border_color = "border" },
	},

	init = function(previous: State?)
		local state: State = previous or {
			page = "audience",
			screen = "home",
			filter = "all",
			sort = "popular",
			sorting = false,
			all = false,
			query = "",
			searching = false,
			hi = 1,
			completed = 3,
			lesson = 4,
			watched = 0.44,
			playing = false,
			tab = "overview",
			bookmarked = false,
			read = {},
			registered = {},
			calendar = true,
			recording = true,
			progress_open = false,
		}
		-- Nothing is left open from the last run.
		state.sorting = false
		state.searching = false
		state.registering = nil
		state.progress_open = false
		state.playing = false
		return state, { crumbs(state) }
	end,

	update = function(state: State, msg)
		local sends = {}
		if type(msg) == "string" then
			if msg == "toggle-sort" then
				state.sorting = not state.sorting
			elseif msg == "close-sort" then
				state.sorting = false
			elseif msg == "browse" then
				state.all = not state.all
			elseif msg == "search-close" then
				state.searching = false
				state.query = ""
			elseif msg == "progress" then
				state.progress_open = true
			elseif msg == "close-progress" then
				state.progress_open = false
			elseif msg == "play" then
				state.playing = not state.playing
				if state.lesson > state.completed and state.watched >= 1 then
					state.watched = 0
				end
			elseif msg == "bookmark" then
				state.bookmarked = not state.bookmarked
			elseif msg == "cancel-register" then
				state.registering = nil
			elseif msg == "toggle-calendar" then
				state.calendar = not state.calendar
			elseif msg == "toggle-recording" then
				state.recording = not state.recording
			elseif msg == "confirm-register" then
				if state.registering then
					state.registered[state.registering] = true
				end
				state.registering = nil
			elseif msg == "complete" then
				-- Finish the lesson shown and move to the next one.
				if state.lesson == state.completed + 1 and state.completed < 8 then
					state.completed += 1
				end
				if state.lesson < 8 then
					state.lesson += 1
					state.watched = 0
					state.tab = "overview"
				end
				state.playing = false
			end
			return state
		end
		if type(msg) ~= "table" then
			return state
		end
		if msg.topic == "inkwell/page" then
			state.page = msg.value
		elseif msg.screen then
			state.screen = msg.screen
			state.title = nil
			state.progress_open = false
			table.insert(sends, crumbs(state))
		elseif msg.lesson then
			-- Open the course at a lesson (from Continue, the syllabus, search).
			if msg.lesson ~= state.lesson then
				state.lesson = msg.lesson
				state.watched = if msg.lesson <= state.completed then 1 else 0
				state.tab = "overview"
				state.playing = false
			end
			state.screen = "lesson"
			state.title = msg.title or state.title
			state.searching = false
			state.query = ""
			state.progress_open = false
			table.insert(sends, crumbs(state))
		elseif msg.guide then
			state.screen = "guide"
			state.guide = msg.guide
			state.title = msg.title
			state.searching = false
			state.query = ""
			table.insert(sends, crumbs(state))
		elseif msg.read_more then
			state.read[msg.read_more] = math.min(1, (msg.from or 0) + 0.25)
		elseif msg.read_all then
			state.read[msg.read_all] = 1
		elseif msg.filter then
			state.filter = msg.filter
		elseif msg.sort then
			state.sort = msg.sort
			state.sorting = false
		elseif msg.tab then
			state.tab = msg.tab
		elseif msg.seek then
			state.watched = msg.seek
		elseif msg.register then
			state.registering = if state.registering == msg.register then nil else msg.register
		elseif msg.msg == "search" then
			state.query = msg.value
			state.searching = msg.value ~= ""
			state.hi = 1
		elseif msg.msg == "search-close" then
			state.searching = false
			state.query = ""
		elseif msg.msg == "search-up" then
			state.hi = math.max(1, state.hi - 1)
		elseif msg.msg == "search-down" then
			state.hi += 1
		elseif msg.open then
			-- A search result, clicked or chosen with Enter: open what it names.
			state.searching = false
			state.query = ""
			return state, { { to = "learn", msg = msg.open } }
		end
		return state, sends
	end,

	view = function(state: State, app)
		if state.page ~= "learn" then
			return ui.column {}
		end
		local all = app.inkwell
		if not all or not all.learn then
			return ui.column { class = "placeholder", ui.text { "Loading the sample courses…", class = "muted" } }
		end
		local data = all.learn
		local course = data.course
		local lessons = course.lessons
		local total = #lessons
		local next_lesson = math.min(state.completed + 1, total)

		local function stat_box(icon: string, color: string, label: string, value: string, unit: string?)
			return ui.column {
				class = "stat-box",
				grow = true,
				ui.row {
					align = "center",
					gap = "small",
					ui.icon { name = icon :: any, size = "small", style = { color = color } },
					ui.text { label, class = "small-muted" },
				},
				ui.row {
					align = "end",
					style = { gap = 2 },
					ui.text { value, class = "stat-value" },
					unit and ui.text { unit, class = "small-muted" },
				},
			}
		end

		local function weeks_grid(size: number)
			local tips = {}
			for index, count in data.weeks do
				tips[index] = `{count} lesson{if count == 1 then "" else "s"}`
			end
			return heat_grid { rows = 3, size = size, spread = true, columns = data.weeks, tooltips = tips, labels = { data.weeks_from, "This week" } }
		end

		local function stats_row()
			local stats = data.stats
			return ui.row {
				gap = "normal",
				stat_box("book-open", "#7c5cff", "Lessons", `{stats.lessons - 3 + state.completed}`, `/{stats.lessons_total}`),
				stat_box("zap", "#f08a24", "Streak", `{stats.streak}`, "weeks"),
				stat_box("shield", "#14a38b", "Certificates", `{stats.certificates + (if state.completed >= total then 1 else 0)}`),
			}
		end

		-- My progress: a sheet with the figures, the weeks and every course.
		local function progress_sheet()
			local rows = {}
			for index, enrolled in data.enrolled do
				local done = if index == 1 then state.completed else enrolled.done
				local finished = done >= enrolled.total
				table.insert(rows, ui.column {
					class = "course-row",
					ui.row {
						align = "center",
						ui.text { enrolled.title, weight = "medium" },
						ui.spacer(),
						finished and ui.row {
							class = "chip",
							align = "center",
							style = { background = "#14a38b1f", color = "#14a38b" },
							ui.icon { name = "shield", size = "small" },
							ui.text { "Certificate", size = "small" },
						},
					},
					ui.row {
						align = "center",
						gap = "normal",
						heat_grid {
							levels = 2,
							size = 12,
							gap = 3,
							color = enrolled.color,
							columns = (function()
								local cells = {}
								for step = 1, enrolled.total do
									cells[step] = { if step <= done then 2 else 0 }
								end
								return cells
							end)(),
						},
						ui.spacer(),
						ui.text { if finished then enrolled.certificate or "Done" else `{done} of {enrolled.total} lessons`, class = "small-muted" },
					},
				})
			end
			return sheet {
				open = state.progress_open,
				dismiss = "close-progress",
				title = "My progress",
				subtitle = `Across {data.stats.courses} enrolled courses.`,
				icon = "list-checks",
				width = 420,
				footer = ui.row {
					ui.spacer(),
					ui.row {
						class = "dark-button",
						align = "center",
						msg = { lesson = next_lesson, title = course.title },
						ui.icon { name = "play", size = "small" },
						ui.text { if state.completed >= total then "Review the course" else `Continue lesson {next_lesson}`, weight = "medium" },
					},
				},
				ui.column {
					gap = "large",
					stats_row(),
					ui.column {
						gap = "normal",
						ui.row { ui.text { "Lessons per week", weight = "medium" }, ui.spacer(), ui.text { "Last 12 weeks", class = "small-muted" } },
						weeks_grid(18),
					},
					ui.column { gap = "none", ui.text { "Courses", class = "eyebrow" }, table.unpack(rows) },
				},
			}
		end

		------------------------------------------------------------------
		-- Home
		------------------------------------------------------------------
		local function home()
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

			local head = ui.row {
				class = "learn-head",
				align = "start",
				ui.column {
					gap = "small",
					ui.text { "Learn", class = "learn-title" },
					ui.text { "Guides, courses and live sessions to grow your newsletter.", class = "muted" },
				},
				ui.spacer(),
				search_field,
				ui.row {
					class = "pill-button",
					height = 38,
					align = "center",
					style = { radius = 10 },
					msg = "progress",
					ui.icon { name = "list-checks", size = "small" },
					ui.text { "My progress" },
				},
			}

			local finished = state.completed >= total
			local featured = ui.column {
				style = { padding = { 0, 24 } },
				course_card {
					eyebrow = "Featured course",
					title = course.title,
					meta = course.meta,
					primary = {
						label = if finished then "Review the course" else `Continue lesson {next_lesson}`,
						icon = "play",
						msg = { lesson = next_lesson, title = course.title },
					},
					secondary = { label = "View syllabus", icon = "book-open", msg = { lesson = state.lesson, title = course.title } },
					progress = state.completed / total,
					label = `{state.completed} of {total} lessons`,
					bar = 250,
					aside = heat_grid { rows = 6, size = 15, columns = course.activity },
				},
			}

			-- Guides: the category filter, the order, then three to a row.
			local options: { any } = { { id = "all", label = "All" } }
			for _, category in data.categories do
				table.insert(options, { id = category.id, label = category.label })
			end
			local shown = {}
			for _, guide in data.guides do
				if state.filter == "all" or guide.category == state.filter then
					table.insert(shown, guide)
				end
			end
			local matching = #shown
			table.sort(shown, function(a, b)
				if state.sort == "newest" then
					if (a.new or false) ~= (b.new or false) then
						return a.new == true
					end
				elseif state.sort == "shortest" and a.minutes ~= b.minutes then
					return a.minutes < b.minutes
				end
				return a.rank < b.rank
			end)
			local limit = if state.all then #shown else math.min(6, #shown)
			local grid = {}
			local row = {}
			for index = 1, limit do
				local guide = shown[index]
				local category = category_of(data, guide.category)
				local read = read_of(state, guide)
				table.insert(row, guide_card {
					title = guide.title,
					category = category.label,
					color = category.color,
					icon = guide.icon,
					read = `{guide.minutes} min read`,
					done = read >= 1,
					new = guide.new and read == 0,
					progress = read,
					msg = { guide = guide.id, title = guide.title },
				})
				if #row == 3 or index == limit then
					while #row < 3 do
						table.insert(row, ui.column { grow = true })
					end
					table.insert(grid, ui.row { gap = "large", table.unpack(row) })
					row = {}
				end
			end

			local guides = ui.column {
				class = "guides",
				grow = true,
				ui.column {
					gap = "small",
					ui.text { "Guides", class = "heading" },
					ui.text { "Short, practical reads you can finish between sends.", class = "muted" },
				},
				ui.row {
					align = "center",
					segmented { options = options, selected = state.filter, event = "filter" },
					ui.spacer(),
					dropdown {
						value = sort_label(state.sort),
						options = SORTS,
						selected = state.sort,
						open = state.sorting,
						toggle = "toggle-sort",
						dismiss = "close-sort",
						event = "sort",
						align = "end",
						width = 180,
					},
				},
				#grid == 0 and ui.column {
					class = "placeholder",
					align = "center",
					ui.text { "No guides in this category yet.", class = "muted" },
				},
				ui.column { gap = "large", table.unpack(grid) },
				ui.row {
					align = "center",
					style = { margin = { top = 6 } },
					ui.text { `Showing {limit} of {matching} guides`, class = "muted" },
					ui.spacer(),
					matching > 6 and ui.row {
						class = "pill-button",
						style = { radius = 10 },
						msg = "browse",
						ui.text { if state.all then "Show fewer" else "Browse all guides", weight = "medium" },
					},
				},
			}

			-- Upcoming live sessions, each with Register and its card.
			local sessions = {}
			for index, session in data.sessions do
				if index > 1 then
					table.insert(sessions, ui.row { class = "rule", height = 1 })
				end
				local registered = state.registered[session.id]
				local card = ui.column {
					class = "card-pop",
					ui.row {
						class = "card-pop-head",
						align = "center",
						ui.row { class = "card-pop-tile", width = 38, height = 38, align = "center", justify = "center", ui.icon { name = "monitor" } },
						ui.column {
							gap = "none",
							ui.text { session.title, weight = "medium" },
							ui.text { session.where, class = "small-muted" },
						},
					},
					ui.row {
						class = "card-pop-date",
						align = "center",
						ui.icon { name = "calendar", size = "small", class = "muted" },
						ui.text { session.date, weight = "medium" },
					},
					ui.row {
						class = "card-pop-host",
						align = "center",
						avatar { name = session.host, shape = "circle", size = "large" },
						ui.column {
							gap = "none",
							ui.text { session.host },
							ui.text { `{session.role} · Host`, class = "small-muted" },
						},
					},
					ui.row {
						class = "card-pop-people",
						align = "center",
						avatar_stack {
							people = (function()
								local people = {}
								for _, name in session.people do
									table.insert(people, { name = name })
								end
								return people
							end)(),
							count = session.registered,
							most = 4,
						},
						ui.text { "writers registered", class = "small-muted" },
					},
					ui.row { class = "rule", height = 1 },
					ui.row {
						class = "card-pop-option",
						align = "center",
						style = { margin = { top = 6 } },
						ui.column {
							grow = true,
							gap = "none",
							ui.text { "Add to my calendar" },
							ui.text { "A calendar invite with the video link", class = "small-muted" },
						},
						switch { on = state.calendar, msg = "toggle-calendar" },
					},
					ui.row {
						class = "card-pop-option",
						align = "center",
						ui.column {
							grow = true,
							gap = "none",
							ui.text { "Email me the recording" },
							ui.text { "Sent within 24 hours of the session", class = "small-muted" },
						},
						switch { on = state.recording, msg = "toggle-recording" },
					},
					ui.row {
						class = "card-pop-foot",
						align = "center",
						ui.text { `Free · {session.length}`, class = "muted" },
						ui.spacer(),
						ui.row { class = "pill-button", msg = "cancel-register", ui.text { "Cancel" } },
						ui.row {
							class = "dark-button",
							align = "center",
							msg = "confirm-register",
							ui.icon { name = "calendar", size = "small" },
							ui.text { "Register", weight = "medium" },
						},
					},
				}
				table.insert(sessions, ui.popover {
					open = state.registering == session.id,
					dismiss = "cancel-register",
					side = "left",
					align = "end",
					gap = 12,
					width = 420,
					class = "card-popover",
					content = card,
					ui.row {
						class = "session",
						align = "center",
						selected = state.registering == session.id,
						ui.column {
							class = "date",
							width = 46,
							shrink = false,
							align = "center",
							ui.text { session.month, class = "date-month" },
							ui.text { session.day, class = "date-day" },
						},
						ui.column {
							grow = true,
							gap = "none",
							ui.text { session.title, weight = "medium", truncate = true },
							ui.text { `{session.when} · Host: {session.host}`, class = "small-muted", truncate = true },
						},
						ui.row {
							class = if registered then { "register", "registered" } else "register",
							align = "center",
							shrink = false,
							msg = if registered then nil else { register = session.id },
							tooltip = if registered then "You're registered; the invite is in your inbox" else nil,
							registered and ui.icon { name = "check", size = "small" },
							ui.text { if registered then "Registered" else "Register", weight = "medium" },
						},
					},
				})
			end

			local side = ui.column {
				class = "side",
				width = 400,
				shrink = false,
				ui.column {
					gap = "small",
					ui.text { "Your progress", class = "heading" },
					ui.text { `Across {data.stats.courses} enrolled courses.`, class = "muted" },
				},
				stats_row(),
				ui.column {
					gap = "normal",
					style = { margin = { top = 4 } },
					ui.row {
						align = "center",
						ui.text { "Lessons per week", weight = "medium" },
						ui.spacer(),
						ui.text { "Last 12 weeks", class = "small-muted" },
					},
					weeks_grid(16),
				},
				ui.row { class = "rule", height = 1, style = { margin = { 10, 0 } } },
				ui.column {
					gap = "small",
					ui.text { "Upcoming live sessions", class = "heading" },
					ui.text { "Join live, ask questions, get the recording.", class = "muted" },
				},
				ui.column { gap = "none", table.unpack(sessions) },
			}

			return ui.column {
				class = "learn",
				grow = true,
				scroll = true,
				ui.column {
					gap = "none",
					shrink = false,
					head,
					featured,
					ui.row { class = "body", guides, side },
				},
				progress_sheet(),
			}
		end

		------------------------------------------------------------------
		-- A lesson of the course
		------------------------------------------------------------------
		local function lesson_screen()
			local index = math.clamp(state.lesson, 1, total)
			local lesson = lessons[index]
			local done = index <= state.completed
			local watched = if done and state.watched == 0 then 1 else state.watched

			-- The scrubber: thin steps across the width, watched, buffered and
			-- ahead, each a place to seek to; the knob rides the last watched.
			local steps = 120
			local here = math.floor(watched * steps + 0.5)
			local buffered = math.min(steps, here + 28)
			local scrub = {}
			for step = 1, steps do
				local fill = if step <= here then "accent" elseif step <= buffered then "#ffffff59" else "#ffffff26"
				table.insert(scrub, ui.row {
					grow = true,
					height = 4,
					msg = { seek = step / steps },
					style = { background = fill },
					step == math.max(here, 1) and ui.row {
						class = "player-knob",
						width = 14,
						height = 14,
						float = { top = -5, right = -7 },
					},
				})
			end

			local player = ui.column {
				class = "player",
				height = 380,
				shrink = false,
				ui.column {
					class = "player-text",
					ui.text { course.title, class = "player-eyebrow" },
					ui.row { max_width = 380, ui.text { lesson.hook, class = "player-hook", lines = 2 } },
					ui.text { lesson.chart, class = "player-caption" },
				},
				ui.row {
					float = { at = "top", inset = 150 },
					justify = "center",
					ui.row {
						class = "player-play",
						width = 66,
						height = 66,
						align = "center",
						justify = "center",
						msg = "play",
						tooltip = if state.playing then "Pause" else "Play",
						ui.icon { name = if state.playing then "pause" else "play", size = "large" },
					},
				},
				ui.spacer(),
				ui.row {
					style = { padding = { 0, 34 } },
					ui.spacer(),
					heat_grid {
						rows = 5,
						size = 19,
						gap = 6,
						color = "#8b6bff",
						empty = "#ffffff14",
						columns = { 0, 0, 1, 1, 2, 2, 3, 3, 4, 5 },
					},
				},
				ui.row { class = "player-scrub", gap = "none", align = "center", table.unpack(scrub) },
				ui.row {
					class = "player-bar",
					align = "center",
					ui.row { class = "player-control", msg = "play", ui.icon { name = if state.playing then "pause" else "play" } },
					ui.text { `{clock(lesson.seconds * watched)} / {lesson.time}`, class = "player-time" },
					ui.spacer(),
					ui.text { "1×", weight = "medium" },
					ui.text { "CC", class = "player-cc" },
					ui.row { class = "player-control", tooltip = "Settings", ui.icon { name = "settings" } },
					ui.row { class = "player-control", tooltip = "Full screen", ui.icon { name = "maximize" } },
				},
			}

			-- Overview, transcript or resources under the player.
			local body
			local resources = {}
			for _, file in course.resources do
				table.insert(resources, ui.row {
					class = "file-row",
					align = "center",
					ui.row {
						class = "file-tile",
						width = 34,
						height = 34,
						shrink = false,
						align = "center",
						justify = "center",
						ui.icon { name = file.icon :: any, size = "small", style = { color = file.color } },
					},
					ui.column {
						grow = true,
						gap = "none",
						ui.text { file.title, weight = "medium" },
						ui.text { file.detail, class = "small-muted" },
					},
					ui.row {
						class = "file-action",
						tooltip = if file.action == "download" then "Download" else "Open",
						ui.icon { name = file.action :: any, size = "small" },
					},
				})
			end
			if state.tab == "transcript" then
				local lines = {}
				for _, line in lesson.transcript or {} do
					table.insert(lines, ui.row {
						gap = "large",
						ui.row { width = 44, shrink = false, ui.text { line.at, class = "transcript-at" } },
						ui.text { line.text, class = "prose" },
					})
				end
				body = ui.column {
					gap = "normal",
					#lines == 0 and ui.text { "The transcript is published with the video.", class = "muted" },
					table.unpack(lines),
				}
			elseif state.tab == "resources" then
				body = ui.column { gap = "small", max_width = 520, table.unpack(resources) }
			else
				local takeaways = {}
				for _, line in lesson.takeaways or {} do
					table.insert(takeaways, ui.row {
						class = "takeaway",
						align = "center",
						ui.icon { name = "check", size = "small", tone = "accent" },
						ui.text { line },
					})
				end
				body = ui.column {
					gap = "large",
					ui.text { lesson.summary, class = "prose" },
					lesson.more and ui.text { lesson.more, class = "prose" },
					#takeaways > 0 and ui.column {
						gap = "normal",
						ui.text { "Key takeaways", weight = "medium" },
						table.unpack(takeaways),
					},
				}
			end

			local function step_card(target: number, label: string, forward: boolean)
				local other = lessons[target]
				return ui.row {
					class = "step-card",
					grow = true,
					align = "center",
					msg = { lesson = target, title = course.title },
					not forward and ui.icon { name = "chevron-left", size = "small", class = "muted" },
					ui.column {
						grow = true,
						gap = "none",
						align = if forward then "end" else "start",
						ui.text { `{label} · Lesson {target}`, class = "small-muted" },
						ui.text { other.title, weight = "medium" },
					},
					forward and ui.icon { name = "chevron-right", size = "small", class = "muted" },
				}
			end

			local main = ui.column {
				grow = true,
				scroll = true,
				ui.column {
					class = "lesson-main",
					shrink = false,
					player,
					ui.row {
						align = "end",
						style = { margin = { top = 6 } },
						ui.column {
							gap = "small",
							ui.text { `Lesson {index} of {total}`, class = "eyebrow" },
							ui.text { lesson.title, class = "learn-title" },
						},
						ui.spacer(),
						ui.row {
							class = "icon-box",
							selected = state.bookmarked,
							msg = "bookmark",
							tooltip = if state.bookmarked then "Saved" else "Save for later",
							ui.icon { name = "bookmark", size = "small" },
						},
						ui.row { class = "icon-box", tooltip = "Copy a link to this lesson", copy = `inkwell://learn/lesson/{index}`, ui.icon { name = "link", size = "small" } },
					},
					tabs {
						variant = "underline",
						selected = state.tab,
						event = "tab",
						tabs = {
							{ id = "overview", label = "Overview", closable = false },
							{ id = "transcript", label = "Transcript", closable = false },
							{ id = "resources", label = "Resources", badge = #course.resources, closable = false },
						},
					},
					body,
					ui.row { height = 12 },
					ui.row {
						gap = "large",
						if index > 1 then step_card(index - 1, "Previous", false) else ui.row { grow = true },
						if index < total then step_card(index + 1, "Up next", true) else ui.row { grow = true },
					},
				},
			}

			-- The syllabus on the right.
			local rows = {}
			for number, item in lessons do
				local is_done = number <= state.completed
				local is_next = number == state.completed + 1
				local open = is_done or is_next
				local mark, icon, detail
				if is_done then
					mark, icon, detail = "lesson-done", "check", `{item.kind} · {item.completed or "Completed"}`
				elseif is_next then
					mark, icon = "lesson-now", "play"
					detail = if number == state.lesson and watched > 0 then `{item.kind} · {math.floor(watched * 100)}% watched` else item.kind
				else
					mark, icon, detail = "lesson-locked", "lock", item.kind
				end
				table.insert(rows, ui.row {
					class = "lesson-row",
					align = "center",
					selected = number == index,
					msg = if open then { lesson = number, title = course.title } else nil,
					tooltip = if open then nil else `Finish lesson {state.completed + 1} to unlock`,
					ui.row {
						class = { "lesson-mark", mark },
						width = 22,
						height = 22,
						shrink = false,
						align = "center",
						justify = "center",
						ui.icon { name = icon :: any, size = "small" },
					},
					ui.column {
						grow = true,
						gap = "none",
						ui.text { `{number}. {item.title}`, truncate = true },
						ui.text { detail, class = if is_next then "lesson-now-text" else "small-muted", truncate = true },
					},
					ui.text { item.time, class = if number == index then nil else "muted" },
				})
			end
			local left = total - state.completed
			local cert_cells = {}
			for number = 1, total do
				cert_cells[number] = { if number <= state.completed then 2 elseif number == state.completed + 1 then 1 else 0 }
			end
			local bar = 332
			local syllabus = ui.column {
				class = "syllabus",
				width = 380,
				shrink = false,
				ui.column {
					grow = true,
					scroll = true,
					ui.column {
						class = "syllabus-body",
						shrink = false,
						ui.column {
							gap = "small",
							ui.text { course.title, class = "heading" },
							ui.text { course.meta, class = "muted" },
						},
						ui.column {
							gap = "small",
							ui.row {
								ui.text { `{state.completed} of {total} lessons complete`, weight = "medium" },
								ui.spacer(),
								ui.text { `{math.floor(state.completed / total * 100 + 0.5)}%`, class = "muted" },
							},
							ui.row {
								class = "syllabus-track",
								width = bar,
								height = 6,
								state.completed > 0 and ui.row { class = "syllabus-fill", width = bar * state.completed / total, height = 6 },
							},
						},
						ui.column { gap = "none", table.unpack(rows) },
						ui.row { class = "rule", height = 1, style = { margin = { 4, 0 } } },
						ui.row {
							align = "center",
							ui.text { "Lesson resources", weight = "medium" },
							ui.spacer(),
							ui.text { `{#course.resources} files`, class = "small-muted" },
						},
						ui.column { gap = "none", table.unpack(resources) },
						ui.row {
							class = "cert",
							align = "center",
							ui.row { class = "cert-tile", width = 38, height = 38, shrink = false, align = "center", justify = "center", ui.icon { name = "shield" } },
							ui.column {
								gap = "normal",
								ui.text {
									if left == 0
										then "Certificate earned. Well done."
										else `Finish {left} more lesson{if left == 1 then "" else "s"} to earn your certificate`,
									weight = "medium",
								},
								heat_grid { levels = 2, size = 14, gap = 4, columns = cert_cells },
							},
						},
					},
				},
				ui.row {
					class = "syllabus-foot",
					align = "center",
					ui.column {
						gap = "none",
						ui.text { if left == 0 then "Course complete" else "Finish to unlock", class = "small-muted" },
						ui.text {
							if left == 0 then "Certificate earned"
								elseif index < total then `Lesson {index + 1} · {lessons[index + 1].time}`
								else "Your certificate",
							weight = "medium",
						},
					},
					ui.spacer(),
					ui.row {
						class = "dark-button",
						align = "center",
						msg = if index == total and done then { screen = "home" } else "complete",
						ui.icon { name = if done then "arrow-right" else "circle-check", size = "small" },
						ui.text {
							if not done then "Mark as complete" elseif index < total then "Next lesson" else "Back to Learn",
							weight = "medium",
						},
					},
				},
			}

			return ui.row { grow = true, gap = "none", main, syllabus }
		end

		------------------------------------------------------------------
		-- A guide
		------------------------------------------------------------------
		local function guide_screen()
			local guide = guide_by_id(data, state.guide)
			if not guide then
				return ui.column {
					class = "placeholder",
					align = "center",
					ui.text { "That guide has moved.", class = "muted" },
					ui.button { "Back to Learn", msg = { screen = "home" } },
				}
			end
			local category = category_of(data, guide.category)
			local read = read_of(state, guide)
			local done = read >= 1
			-- Two more to read: the same topic first, then the most popular.
			local picks = {}
			for _, other in data.guides do
				if other.id ~= guide.id and other.category == guide.category and #picks < 2 then
					table.insert(picks, other)
				end
			end
			for _, other in data.guides do
				if other.id ~= guide.id and other.category ~= guide.category and #picks < 2 then
					table.insert(picks, other)
				end
			end
			local related = {}
			for _, other in picks do
				do
					local other_read = read_of(state, other)
					local other_category = category_of(data, other.category)
					table.insert(related, ui.row {
						height = 236,
						guide_card {
							title = other.title,
							category = other_category.label,
							color = other_category.color,
							icon = other.icon,
							read = `{other.minutes} min read`,
							done = other_read >= 1,
							new = other.new and other_read == 0,
							progress = other_read,
							msg = { guide = other.id, title = other.title },
						},
					})
				end
			end
			local bar = 120
			local article = ui.column {
				grow = true,
				scroll = true,
				ui.column {
					class = "lesson-main",
					shrink = false,
					max_width = 820,
					ui.row {
						class = "back",
						hug = true,
						align = "center",
						msg = { screen = "home" },
						ui.icon { name = "arrow-left", size = "small" },
						ui.text { "All guides" },
					},
					ui.row {
						class = "guide-band",
						height = 150,
						align = "center",
						justify = "center",
						style = { background = `{category.color}/10` },
						ui.icon { name = guide.icon :: any, size = "title", style = { color = category.color } },
					},
					ui.text { category.label, class = "eyebrow", style = { color = category.color } },
					ui.text { guide.title, class = "learn-title" },
					ui.row {
						align = "center",
						gap = "large",
						ui.row { align = "center", gap = "small", ui.icon { name = "clock", size = "small", class = "muted" }, ui.text { `{guide.minutes} min read`, class = "muted" } },
						ui.row {
							align = "center",
							gap = "normal",
							ui.row {
								width = bar,
								height = 4,
								style = { background = "muted-foreground/14", radius = 999 },
								read > 0 and ui.row { width = math.max(4, bar * read), height = 4, style = { background = if done then "success" else "accent", radius = 999 } },
							},
							ui.text { if done then "Done" else `{math.floor(read * 100 + 0.5)}% read`, class = if done then nil else "muted", style = if done then { color = "success" } else nil },
						},
					},
					ui.text { guide.summary, class = "prose", size = "large" },
					ui.column {
						gap = "normal",
						style = { margin = { top = 8 } },
						ui.text { "In this guide", weight = "medium" },
						numbered_list { items = guide.steps },
					},
					ui.row {
						class = "cert",
						align = "center",
						style = { margin = { top = 4 } },
						ui.row {
							class = "cert-tile",
							width = 38,
							height = 38,
							shrink = false,
							align = "center",
							justify = "center",
							style = { background = `{category.color}/14`, color = category.color },
							ui.icon { name = "sparkles" },
						},
						ui.column {
							gap = "none",
							ui.text { "Try it before your next send", weight = "medium" },
							ui.text { guide.steps[1], class = "muted" },
						},
					},
					ui.row {
						gap = "normal",
						style = { margin = { top = 10 } },
						ui.row {
							class = "dark-button",
							align = "center",
							msg = if done then { screen = "home" } else { read_all = guide.id },
							ui.icon { name = if done then "arrow-left" else "circle-check", size = "small" },
							ui.text { if done then "Back to guides" else "Mark as read", weight = "medium" },
						},
						not done and ui.row {
							class = "pill-button",
							align = "center",
							msg = { read_more = guide.id, from = read },
							ui.icon { name = "book-open", size = "small" },
							ui.text { "Keep reading" },
						},
					},
				},
			}
			local side = ui.column {
				class = "side",
				width = 380,
				shrink = false,
				style = { padding = 24 },
				ui.column {
					gap = "small",
					ui.text { `More in {category.label}`, class = "heading" },
					ui.text { "Pick up where this one leaves off.", class = "muted" },
				},
				ui.column { gap = "large", table.unpack(related) },
			}
			return ui.row { grow = true, gap = "none", article, side }
		end

		if state.screen == "lesson" then
			return lesson_screen()
		elseif state.screen == "guide" then
			return guide_screen()
		end
		return home()
	end,
}
