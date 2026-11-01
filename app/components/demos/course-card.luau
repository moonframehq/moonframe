-- A featured course: Continue finishes the current lesson and moves on, and
-- the grid on the right fills as lessons are done.
local course_card = use "course-card"
local heat_grid = use "heat-grid"

return mod {
	init = function(previous)
		return previous or { done = 3, total = 8 }
	end,

	update = function(state, msg)
		if msg == "continue" then
			state.done = if state.done >= state.total then 0 else state.done + 1
		end
	end,

	view = function(state)
		local columns = {}
		for week = 1, 12 do
			columns[week] = math.min(6, math.floor(week * state.done / 8 + (week % 3) / 2))
		end
		local finished = state.done >= state.total
		return course_card {
			eyebrow = "Featured course",
			title = "The Reader Growth Playbook",
			meta = `{state.total} lessons · 46 min · Beginner`,
			primary = {
				label = if finished then "Start over" else `Continue lesson {state.done + 1}`,
				icon = if finished then "refresh-cw" else "play",
				msg = "continue",
			},
			secondary = { label = "View syllabus", icon = "book-open", msg = "syllabus" },
			progress = state.done / state.total,
			label = `{state.done} of {state.total} lessons`,
			bar = 200,
			aside = heat_grid { rows = 6, columns = columns },
		}
	end,
}
