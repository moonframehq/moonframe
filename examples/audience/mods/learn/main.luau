-- The Learn page and its flow. Home: a header with a guide search (results
-- in a popover as you type) and My progress (a sheet), the featured course,
-- the guides by category with their progress, and on the right your
-- progress, lessons per week and live sessions (Register opens a card).
-- From there: a course's lesson (a player, the overview, transcript and
-- resources, the syllabus beside it, Mark as complete) and a guide's page.
-- The catalogue is `app.inkwell.learn`, sample data from the provider;
-- what you finish, read and register for is this mod's own state.
-- Its screens are components of the app's: learn-home (with learn-search
-- and learn-sessions), learn-lesson and learn-guide.

local learn_home = use "learn-home"
local learn_lesson = use "learn-lesson"
local learn_guide = use "learn-guide"

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

-- The crumbs the top bar shows: Learn › the open course or guide.
local function crumbs(state: State)
	return { to = "topic:inkwell/learn", msg = { screen = state.screen, title = state.title } }
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

		if state.screen == "lesson" then
			return learn_lesson { state = state, data = data }
		elseif state.screen == "guide" then
			return learn_guide { state = state, data = data }
		end
		return learn_home { state = state, data = data }
	end,
}
