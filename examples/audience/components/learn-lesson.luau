-- A lesson of Learn's course: the player, the overview, transcript and
-- resources, the steps to the lessons around it, and the syllabus beside
-- it with Mark as complete. Its messages go to the learn mod. Inkwell only.

local heat_grid = use "heat-grid"
local tabs = use "tabs"

-- The learn mod's state (mods/learn/main.luau has its fields).
type State = any

type Props = { state: State, data: any }

local function clock(seconds: number): string
	local whole = math.floor(seconds)
	return string.format("%02d:%02d", whole // 60, whole % 60)
end

return component {
	title = "Learn Lesson",
	category = "Inkwell",
	icon = "circle-play",
	description = "A lesson of the Learn page's course, with its syllabus.",
	when = "The lesson screen of Inkwell's Learn page.",
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
		local course = data.course
		local lessons = course.lessons
		local total = #lessons

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
	end,
}
