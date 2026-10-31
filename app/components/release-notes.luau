-- One release's notes: a version and date header with a badge, highlight
-- cards (icon, title, detail, optional media such as a `ui.image`), and an
-- "Also fixed" bullet list. `collapsed` shows only the header and a count,
-- for older releases in a list.

type Highlight = {
	title: string,
	detail: string?,
	icon: string?,
	media: any,
}

type Props = {
	version: string,
	date: string?,
	name: string?,
	badge: string?,
	summary: string?,
	highlights: { Highlight }?,
	fixes: { string }?,
	fixes_title: string?,
	collapsed: boolean?,
	toggle: any,
	id: any,
	to: string?,
}

return component {
	title = "Release notes",
	category = "Feedback",
	icon = "sparkles",
	description = "One release: version and date header, highlight cards with optional media, an Also fixed list; collapsible for older releases.",
	when = "A What's new page, an update panel or a changelog list (one per release, older ones `collapsed`). For the update offer itself use `update-pill`; for a first-run tour use `onboarding`; for a dated activity feed use `timeline`.",
	props = {
		["version"] = "the version, e.g. \"1.4.0\"",
		["date"] = "release date text",
		["name"] = "app name before the version; default none",
		["badge"] = "a badge after the version (\"Latest\", \"Beta\")",
		["summary"] = "a muted line under the header",
		["highlights"] = "list of { title, detail, icon, media } cards; `media` is an element (a `ui.image`)",
		["fixes"] = "list of strings for the Also fixed bullets",
		["fixes_title"] = "default \"Also fixed\"",
		["collapsed"] = "only the header and a count line",
		["toggle"] = "msg the header sends to expand or collapse; default `{ toggle = id or version }` when set to `true`",
		["id"] = "id for the default toggle msg",
		["to"] = "mod to send the toggle to, if not the drawing one",
	},
	styles = {
		[".release-notes"] = { gap = 12 },
		[".release-notes-head"] = { gap = 8, radius = 8 },
		[".release-notes-head-clickable:hover"] = { background = "list-hover" },
		[".release-notes-chevron"] = { color = "muted-foreground", size = "small" },
		[".release-notes-version"] = { weight = "bold", size = "large" },
		[".release-notes-date"] = { size = "small", color = "muted-foreground" },
		[".release-notes-summary"] = { color = "muted-foreground", size = "small" },
		[".release-notes-highlights"] = { gap = 8 },
		[".release-notes-card"] = {
			background = "muted/50",
			border = "all",
			border_color = "border",
			radius = 10,
			padding = 12,
			gap = 10,
		},
		[".release-notes-card-icon"] = { color = "accent", margin = { top = 2 } },
		[".release-notes-card-title"] = { weight = "medium" },
		[".release-notes-card-detail"] = { size = "small", color = "muted-foreground" },
		[".release-notes-media"] = { radius = 8, border = "all", border_color = "border", margin = { top = 6 } },
		[".release-notes-fixes"] = { gap = 4 },
		[".release-notes-fixes-title"] = { size = "small", weight = "medium", color = "muted-foreground" },
		[".release-notes-fix"] = { gap = 8, size = "small" },
		[".release-notes-bullet"] = { color = "muted-foreground" },
		[".release-notes-count"] = { size = "small", color = "muted-foreground" },
	},
	example = {
		name = "Moonframe",
		version = "1.4.0",
		date = "October 28, 2026",
		badge = "Latest",
		summary = "Settings get a new home, and remote hosts reconnect on their own.",
		highlights = {
			{
				icon = "settings",
				title = "A settings window",
				detail = "Every option in one place, grouped by page, with search and per-host overrides marked Local.",
			},
			{
				icon = "wifi",
				title = "Automatic reconnect",
				detail = "When an SSH host drops, Moonframe retries with backoff and keeps your panes where they were.",
			},
		},
		fixes = {
			"The sidebar no longer jumps when an agent finishes.",
			"Pasting into a terminal with bracketed paste keeps tabs.",
			"Toasts stack upward instead of over each other.",
		},
	},

	build = function(props: Props)
		local id = props.id or props.version
		local toggle = props.toggle
		if toggle == true then
			toggle = { toggle = id }
		end
		local collapsed = props.collapsed == true

		local head = ui.row {
			class = if toggle ~= nil then "release-notes-head release-notes-head-clickable" else "release-notes-head",
			align = "center",
			msg = toggle,
			to = props.to,
			toggle ~= nil and ui.icon {
				name = if collapsed then "chevron-right" else "chevron-down",
				class = "release-notes-chevron",
			},
			ui.text {
				if props.name then props.name .. " " .. props.version else props.version,
				class = "release-notes-version",
			},
			props.badge ~= nil and ui.badge { props.badge, tone = "accent" },
			ui.spacer(),
			props.date ~= nil and ui.text { props.date, class = "release-notes-date" },
		}

		local highlights: { Highlight } = props.highlights or {}
		local fixes: { string } = props.fixes or {}

		if collapsed then
			local parts = {}
			if #highlights > 0 then
				parts[#parts + 1] = #highlights .. (if #highlights == 1 then " highlight" else " highlights")
			end
			if #fixes > 0 then
				parts[#parts + 1] = #fixes .. (if #fixes == 1 then " fix" else " fixes")
			end
			return ui.column {
				class = "release-notes release-notes-collapsed",
				head,
				#parts > 0 and ui.text { table.concat(parts, " · "), class = "release-notes-count" },
			}
		end

		local cards = {}
		for index, item in ipairs(highlights) do
			cards[index] = ui.row {
				class = "release-notes-card",
				shrink = false,
				align = "start",
				item.icon ~= nil and ui.icon { name = item.icon :: any, class = "release-notes-card-icon" },
				ui.column {
					grow = true,
					gap = "none",
					ui.text { item.title, class = "release-notes-card-title" },
					item.detail ~= nil and ui.text { item.detail, class = "release-notes-card-detail" },
					item.media ~= nil and ui.column { class = "release-notes-media", item.media },
				},
			}
		end

		local fix_rows = {}
		for index, fix in ipairs(fixes) do
			fix_rows[index] = ui.row {
				class = "release-notes-fix",
				shrink = false,
				align = "start",
				ui.text { "•", class = "release-notes-bullet" },
				ui.text { fix, size = "small" },
			}
		end

		return ui.column {
			class = "release-notes",
			head,
			props.summary ~= nil and ui.text { props.summary, class = "release-notes-summary" },
			#cards > 0 and ui.column { class = "release-notes-highlights", shrink = false, table.unpack(cards) },
			#fix_rows > 0 and ui.column {
				class = "release-notes-fixes",
				shrink = false,
				ui.text { props.fixes_title or "Also fixed", class = "release-notes-fixes-title" },
				table.unpack(fix_rows),
			},
		}
	end,
}
