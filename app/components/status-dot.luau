-- An agent's state as a small dot: working (yellow), blocked (red), done
-- (teal), idle (a hollow green ring) and unknown (a smaller grey dot).
-- `spin` turns working into a loader; `chip` shows a labelled pill instead,
-- "Needs you" when blocked.

type Props = {
	status: string?,
	size: string?,
	spin: boolean?,
	chip: boolean?,
	label: string?,
	tooltip: string?,
}

local LABELS = {
	working = "Working",
	blocked = "Needs you",
	done = "Done",
	idle = "Idle",
	unknown = "Unknown",
}

-- Dot diameters by size, the unknown dot two pixels smaller.
local DIAMETER = { small = 6, normal = 8, large = 10 }

local function dot(status: string, size: string)
	local diameter = DIAMETER[size] or 8
	if status == "unknown" then
		diameter -= 2
	end
	return ui.row {
		class = { "status-dot", `status-dot-{status}` },
		width = diameter,
		height = diameter,
	}
end

return component {
	title = "Status Dot",
	category = "Status",
	icon = "circle-check",
	description = "An agent's state as a coloured dot: working, blocked, done, idle (hollow ring) or unknown; spins or becomes a \"Needs you\" pill.",
	when = "Show the state of an agent, job or session beside its name: in sidebar rows, tabs and lists. Use `chip = true` where the state must catch the eye (a blocked agent). For a count use `count-bubble`; for a PR's state use `pr-badge`; for CI results use `checks`.",
	props = {
		["status"] = "working | blocked | done | idle | unknown (default unknown)",
		["size"] = "small | normal | large (dot diameter 6, 8, 10)",
		["spin"] = "when working, show a spinning loader instead of the dot",
		["chip"] = "show a pill with the dot and a label (\"Needs you\" when blocked)",
		["label"] = "the chip's text instead of the default for the status",
		["tooltip"] = "text under the pointer",
	},
	styles = {
		[".status-dot"] = { radius = 999 },
		[".status-dot-working"] = { background = "warning" },
		[".status-dot-blocked"] = { background = "danger" },
		[".status-dot-done"] = { background = "success" },
		[".status-dot-idle"] = { border = "all", border_color = "success" },
		[".status-dot-unknown"] = { background = "muted-foreground/60" },
		[".status-dot-spin"] = { color = "warning", size = "small" },
		[".status-dot-chip"] = {
			radius = 999,
			padding = { 1, 8 },
			gap = 5,
			background = "muted-foreground/12",
			color = "muted-foreground",
		},
		[".status-dot-chip-label"] = { size = "small", weight = "medium" },
		[".status-dot-chip-working"] = { background = "warning/15", color = "warning" },
		[".status-dot-chip-blocked"] = { background = "danger/15", color = "danger" },
		[".status-dot-chip-done"] = { background = "success/15", color = "success" },
		[".status-dot-chip-idle"] = { background = "success/10", color = "success" },
	},
	example = { status = "blocked", chip = true },
	usage = [[local status_dot = use "status-dot"

ui.row { align = "center", gap = "normal",
	status_dot { status = "working", spin = true },
	status_dot { status = "blocked" },
	status_dot { status = "done" },
	status_dot { status = "idle" },
	status_dot { status = "unknown" },
	status_dot { status = "blocked", chip = true },
}]],

	build = function(props: Props)
		local status = props.status or "unknown"
		if LABELS[status] == nil then
			status = "unknown"
		end
		local size = props.size or "normal"
		local mark = if props.spin and status == "working"
			then ui.icon { name = "loader-circle", spin = true, class = "status-dot-spin" }
			else dot(status, size)
		if props.chip then
			return ui.row {
				class = { "status-dot-chip", `status-dot-chip-{status}` },
				hug = true,
				shrink = false,
				align = "center",
				tooltip = props.tooltip,
				mark,
				ui.text { props.label or LABELS[status], class = "status-dot-chip-label" },
			}
		end
		return ui.row {
			hug = true,
			shrink = false,
			align = "center",
			justify = "center",
			tooltip = props.tooltip,
			mark,
		}
	end,
}
