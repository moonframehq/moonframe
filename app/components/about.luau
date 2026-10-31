-- An About box: the app's icon (or an image), name, version and tagline,
-- build details as aligned key-values, and a row of links. A small centred
-- modal by default; `inline` draws just the card.

type Link = {
	label: string,
	msg: any,
	icon: string?,
}

type Props = {
	open: boolean?,
	dismiss: any,
	name: string,
	version: string?,
	tagline: string?,
	icon: string?,
	-- A picture instead of the icon: a path for `ui.image`.
	image: string?,
	info: { { string } }?,
	links: { Link }?,
	copyright: string?,
	-- Copied by a "Copy" button: the version and build info for bug reports.
	copy: string?,
	inline: boolean?,
	width: number?,
	to: string?,
}

return component {
	title = "About",
	category = "Overlays",
	icon = "info",
	description = "An About box: app icon, name, version and tagline, build info key-values, links and a copyright line.",
	when = "Help > About, or the top of a settings General page (`inline`). For the changes in a version use `release-notes`; for other properties use `key-values`; for a first-run welcome use `onboarding`.",
	props = {
		["open"] = "show it",
		["dismiss"] = "msg for Escape, the × and backdrop clicks",
		["name"] = "the app's name",
		["version"] = "version text",
		["tagline"] = "a muted line under the version",
		["icon"] = "the app icon (an icon name); default `square-terminal`",
		["image"] = "a picture path for `ui.image` instead of the icon",
		["info"] = "build details as { label, value } pairs (Commit, Built, Channel)",
		["links"] = "list of { label, msg, icon }: Website, Release notes, Report an issue",
		["copyright"] = "small muted line at the bottom",
		["copy"] = "text a Copy button puts on the clipboard (version and build, for bug reports)",
		["inline"] = "draw only the card, without the modal",
		["width"] = "pixels; default 360",
		["to"] = "mod the links send to, if not the drawing one",
	},
	styles = {
		[".about"] = { background = "background", border = "all", border_color = "border", radius = 14, padding = 0 },
		[".about-card"] = { padding = { 24, 24, 18, 24 }, gap = 14 },
		[".about-close"] = { padding = 4, radius = 6, color = "muted-foreground" },
		[".about-close:hover"] = { background = "list-hover", color = "foreground" },
		[".about-logo"] = { background = "accent/12", color = "accent", radius = 18, padding = 16, size = "title" },
		[".about-image"] = { radius = 16 },
		[".about-name"] = { size = "title", weight = "bold" },
		[".about-version"] = { color = "muted-foreground", font = "mono", size = "small" },
		[".about-tagline"] = { color = "muted-foreground", size = "small" },
		[".about-info"] = {
			background = "muted/50",
			border = "all",
			border_color = "border",
			radius = 10,
			padding = { 10, 12 },
			gap = 4,
		},
		[".about-info-label"] = { size = "small", color = "muted-foreground" },
		[".about-info-value"] = { size = "small", font = "mono" },
		[".about-links"] = { gap = 4 },
		[".about-link"] = { padding = { 3, 8 }, radius = 6, gap = 6, color = "accent", size = "small" },
		[".about-link:hover"] = { background = "accent/10" },
		[".about-copyright"] = { size = "small", color = "muted-foreground" },
	},
	example = {
		open = true,
		dismiss = "close",
		name = "Moonframe",
		version = "1.4.0 (beta 2)",
		tagline = "Terminals, agents and worktrees in one window.",
		icon = "square-terminal",
		info = {
			{ "Commit", "6731aec" },
			{ "Built", "2026-10-28 14:02" },
			{ "Channel", "beta" },
			{ "Platform", "macOS 26 · arm64" },
		},
		links = {
			{ label = "Website", icon = "globe", msg = "open-website" },
			{ label = "Release notes", icon = "file-text", msg = "open-notes" },
			{ label = "Report an issue", icon = "bug", msg = "report" },
		},
		copy = "Moonframe 1.4.0 (beta 2) 6731aec macOS 26 arm64",
		copyright = "© 2026 Fabien Penso",
	},

	build = function(props: Props)
		local logo
		if props.image then
			logo = ui.image { src = props.image, width = 72, height = 72, fit = "cover", class = "about-image" }
		else
			logo = ui.row { class = "about-logo", ui.icon { name = (props.icon or "square-terminal") :: any } }
		end

		local info: any = false
		if props.info and #props.info > 0 then
			local rows = {}
			for index, pair in props.info do
				rows[index] = ui.row {
					gap = "small",
					ui.row { width = 72, ui.text { pair[1], class = "about-info-label" } },
					ui.text { pair[2] or "", class = "about-info-value", truncate = true },
				}
			end
			info = ui.column { class = "about-info", self_align = "stretch", table.unpack(rows) }
		end

		local links: any = false
		if props.links and #props.links > 0 then
			local items = {}
			for index, link in props.links do
				items[index] = ui.row {
					class = "about-link",
					align = "center",
					msg = link.msg,
					to = props.to,
					link.icon ~= nil and ui.icon { name = link.icon :: any },
					ui.text { link.label },
				}
			end
			links = ui.row { class = "about-links", wrap = true, justify = "center", table.unpack(items) }
		end

		local card = ui.column {
			class = "about-card",
			align = "center",
			logo,
			ui.column {
				align = "center",
				gap = "none",
				ui.text { props.name, class = "about-name" },
				props.version ~= nil and ui.row {
					gap = "small",
					align = "center",
					ui.text { "Version " .. props.version, class = "about-version" },
					props.copy ~= nil and ui.button { "Copy", small = true, variant = "ghost", copy = props.copy, tooltip = "Copy version and build" },
				},
				props.tagline ~= nil and ui.text { props.tagline, class = "about-tagline" },
			},
			info,
			links,
			props.copyright ~= nil and ui.text { props.copyright, class = "about-copyright" },
		}

		if props.inline then
			return ui.column { class = "about", width = props.width or 360, card }
		end
		return ui.overlay {
			open = props.open,
			dismiss = props.dismiss,
			to = props.to,
			place = "center",
			backdrop = true,
			width = props.width or 360,
			class = "about",
			content = ui.column {
				gap = "none",
				card,
				props.dismiss ~= nil and ui.row {
					class = "about-close",
					float = { top = 10, right = 10 },
					msg = props.dismiss,
					to = props.to,
					tooltip = "Close",
					ui.icon "close",
				},
			},
		}
	end,
}
