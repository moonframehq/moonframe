-- A CI or status box: a summary header ("3 of 4 passed") over one row per
-- check, each with an icon by state (passed check, failed cross, pending
-- spinner, skipped dash), its name and a detail such as the duration.
-- A row with `msg`, or the `event` prop, makes rows clickable.

type Check = {
	id: any,
	name: string,
	state: string?,
	detail: string?,
	msg: any,
}

type Props = {
	checks: { Check },
	title: string?,
	event: string?,
	to: string?,
	collapsed: boolean?,
	toggle: any,
}

local ICONS = {
	passed = "circle-check",
	failed = "circle-x",
	pending = "loader-circle",
	skipped = "circle-dashed",
	cancelled = "ban",
}

return component {
	title = "Checks",
	category = "Git",
	icon = "list-checks",
	description = "A CI/status box: \"3 of 4 passed\" header with an overall icon, then one row per check with a state icon (spinning when pending), name and detail.",
	when = "CI runs or readiness factors for a pull request or branch: a PR detail pane, a merge dialog. For a single summary dot use `pr-badge` or `pr-row` with `checks`; for an agent's state use `status-dot`.",
	props = {
		["checks"] = "list of { id, name, state = passed|failed|pending|skipped|cancelled, detail, msg }",
		["title"] = "header text instead of the summary (\"3 of 4 passed\" stays beside it)",
		["event"] = "row click message key: sends { [event] = id or name } (rows are not clickable without it or a row `msg`)",
		["to"] = "the mod to send it to",
		["collapsed"] = "show only the header",
		["toggle"] = "message the header sends on a click (to collapse or expand)",
	},
	styles = {
		[".checks"] = { gap = 0, border = "all", border_color = "border", radius = 8 },
		[".checks-header"] = { gap = 8, padding = { 8, 12 } },
		[".checks-header:hover"] = { background = "list-hover" },
		[".checks-title"] = { weight = "semibold", size = "small" },
		[".checks-summary"] = { size = "small", color = "muted-foreground" },
		[".checks-row"] = { gap = 8, padding = { 6, 12 }, border = "top", border_color = "border/70" },
		[".checks-row:hover"] = { background = "list-hover" },
		[".checks-name"] = { size = "small" },
		[".checks-detail"] = { size = "small", color = "muted-foreground" },
		[".checks-icon"] = { size = "small", color = "muted-foreground" },
		[".checks-passed"] = { color = "success" },
		[".checks-failed"] = { color = "danger" },
		[".checks-pending"] = { color = "warning" },
	},
	example = {
		checks = {
			{ name = "ci / test (ubuntu)", state = "passed", detail = "4m 12s" },
			{ name = "ci / test (macos)", state = "passed", detail = "6m 40s" },
			{ name = "ci / clippy", state = "failed", detail = "1 warning denied" },
			{ name = "ci / build release", state = "pending", detail = "running 2m" },
			{ name = "Review", state = "passed", detail = "Approved by alice" },
		},
	},

	build = function(props: Props)
		local counts = { passed = 0, failed = 0, pending = 0 }
		local counted = 0
		for _, check in props.checks do
			local state = check.state or "pending"
			if counts[state] ~= nil then
				counts[state] += 1
			end
			if state ~= "skipped" and state ~= "cancelled" then
				counted += 1
			end
		end
		local overall = if counts.failed > 0 then "failed" elseif counts.pending > 0 then "pending" else "passed"
		local summary = `{counts.passed} of {counted} passed`
		if counts.failed > 0 then
			summary ..= ` · {counts.failed} failed`
		elseif counts.pending > 0 then
			summary ..= ` · {counts.pending} running`
		end

		local function icon(state: string)
			return ui.icon {
				name = (ICONS[state] or "circle-dashed") :: any,
				spin = state == "pending",
				class = { "checks-icon", `checks-{state}` },
			}
		end

		local rows = {}
		if not props.collapsed then
			for index, check in props.checks do
				local state = check.state or "pending"
				local id = if check.id == nil then check.name else check.id
				rows[index] = ui.row {
					class = "checks-row",
					shrink = false,
					align = "center",
					msg = check.msg or (if props.event then { [props.event] = id } else nil),
					to = props.to,
					icon(state),
					ui.text { check.name, class = "checks-name", truncate = true },
					ui.spacer(),
					check.detail and ui.text { check.detail, class = "checks-detail" },
				}
			end
		end

		return ui.column {
			class = "checks",
			ui.row {
				class = "checks-header",
				shrink = false,
				align = "center",
				msg = props.toggle,
				to = props.to,
				icon(overall),
				ui.text { props.title or summary, class = "checks-title" },
				props.title and ui.text { summary, class = "checks-summary" },
				ui.spacer(),
				props.toggle ~= nil and ui.icon {
					name = if props.collapsed then "chevron-down" else "chevron-up",
					class = "checks-icon",
				},
			},
			table.unpack(rows),
		}
	end,
}
