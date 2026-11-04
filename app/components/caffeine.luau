-- A cup that keeps the machine awake while it is on, like the one in Herdr
-- UI's status bar. It asks `app:awake` for the drawing mod and shows what
-- `app.awake` says, so it stays right if another mod holds too.

type Awake = {
	held: boolean,
	display: boolean,
	problem: string?,
}

type Props = {
	awake: Awake?,
	scope: string?,
	reason: string?,
	variant: string?,
	size: string?,
}

local icon_button = use "icon-button"

return component {
	title = "Caffeine",
	category = "Controls",
	icon = "coffee",
	description = "A cup that keeps the machine awake while on, so a long job is not cut off by sleep.",
	when = "A scan, render or copy that runs unattended: put it in a status bar or toolbar for the user to turn on. A mod that knows when its job starts and ends should send `app:awake` itself instead.",
	props = {
		["awake"] = "`app.awake`, for it to show how things stand",
		["scope"] = "\"display\" keeps the screen on too (default); \"system\" lets it sleep and lock",
		["reason"] = "why, for the system's list of what is keeping it awake",
		["variant"] = "as `icon-button`",
		["size"] = "as `icon-button`",
	},
	example = { awake = { held = true, display = true }, reason = "Scanning" },
	usage = [[local caffeine = use "caffeine"

caffeine { awake = app.awake, scope = "system", reason = "Scanning" }]],

	build = function(props: Props)
		local awake = props.awake
		local held = awake ~= nil and awake.held
		local system = props.scope == "system"
		local what = if system then "system" else "display"
		local tooltip = if held then `Keeping the {what} awake` else `Keep the {what} awake`
		if awake ~= nil and awake.problem ~= nil then
			tooltip = awake.problem
		end
		return icon_button {
			icon = "coffee",
			tooltip = tooltip,
			active = held,
			variant = props.variant,
			size = props.size,
			to = "app:awake",
			msg = if held
				then { hold = false }
				else { hold = true, display = not system, reason = props.reason },
		}
	end,
}
