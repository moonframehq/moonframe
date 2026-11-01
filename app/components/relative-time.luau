-- How long ago something happened, from seconds elapsed: "just now",
-- "5 min ago", "yesterday", "3 days ago", or compact "5m", "2h", "3d".
-- Mods compute the elapsed seconds themselves, from a provider's
-- timestamp and the current time, and redraw as it grows.

type Props = {
	seconds: number,
	compact: boolean?,
	prefix: string?,
	tooltip: string?,
	size: string?,
}

local MINUTE, HOUR, DAY, WEEK, MONTH, YEAR = 60, 3600, 86400, 604800, 2629800, 31557600

local function long(seconds: number): string
	if seconds < 45 then
		return "just now"
	end
	-- Rounded before the unit is picked, so 59.5 minutes is an hour, not
	-- "60 min ago", and 23.6 hours is "yesterday", not "24 hours ago".
	local minutes = math.max(1, math.floor(seconds / MINUTE + 0.5))
	if minutes < 60 then
		return `{minutes} min ago`
	end
	local hours = math.floor(seconds / HOUR + 0.5)
	if hours < 24 then
		return if hours == 1 then "an hour ago" else `{hours} hours ago`
	elseif seconds < 2 * DAY then
		return "yesterday"
	elseif seconds < WEEK then
		return `{math.floor(seconds / DAY)} days ago`
	elseif seconds < MONTH then
		local weeks = math.floor(seconds / WEEK)
		return if weeks == 1 then "last week" else `{weeks} weeks ago`
	elseif seconds < YEAR then
		local months = math.floor(seconds / MONTH)
		return if months == 1 then "last month" else `{months} months ago`
	end
	local years = math.floor(seconds / YEAR)
	return if years == 1 then "last year" else `{years} years ago`
end

local function short(seconds: number): string
	if seconds < MINUTE then
		return "now"
	elseif seconds < HOUR then
		return `{math.floor(seconds / MINUTE)}m`
	elseif seconds < DAY then
		return `{math.floor(seconds / HOUR)}h`
	elseif seconds < WEEK then
		return `{math.floor(seconds / DAY)}d`
	elseif seconds < YEAR then
		return `{math.floor(seconds / WEEK)}w`
	end
	return `{math.floor(seconds / YEAR)}y`
end

return component {
	title = "Relative Time",
	category = "Data",
	icon = "calendar",
	description = "Elapsed seconds as \"just now\", \"5 min ago\", \"yesterday\", \"3 days ago\", or compact \"5m\", \"2h\", \"3d\".",
	when = "An age or last-updated time in rows, cards and headers. Pass elapsed `seconds`: the mod computes them from a provider's timestamp and the current time. Use `compact` in tight columns (PR lists, tables). For a dated list of events use `timeline`.",
	props = {
		["seconds"] = "seconds elapsed (the mod computes them from a provider's timestamp)",
		["compact"] = "\"5m\", \"2h\", \"3d\", \"4w\", \"1y\" instead of words",
		["prefix"] = "words before it, e.g. \"updated\"",
		["size"] = "text size (default small)",
		["tooltip"] = "text under the pointer, e.g. the exact date",
	},
	styles = {
		[".relative-time"] = { color = "muted-foreground" },
	},
	example = { seconds = 3 * 86400 + 7200, prefix = "updated", tooltip = "2026-10-28 14:02" },

	build = function(props: Props)
		local seconds = math.max(0, props.seconds or 0)
		local text = if props.compact then short(seconds) else long(seconds)
		if props.prefix then
			text = `{props.prefix} {text}`
		end
		return ui.row {
			shrink = false,
			tooltip = props.tooltip,
			ui.text { text, class = "relative-time", size = (props.size or "small") :: any },
		}
	end,
}
