-- An update offer: a rounded pill reading "Moonframe 1.4 is ready", an accent
-- Restart button, a "What changed" link and a round ×. Inline by default
-- (a status bar, a title bar); `floating` puts it in a window corner.

type Props = {
	name: string?,
	version: string?,
	text: string?,
	icon: string?,
	restart: any,
	restart_label: string?,
	notes: any,
	notes_label: string?,
	dismiss: any,
	floating: boolean?,
	place: string?,
	-- A download still running: a spinner and no Restart button.
	downloading: boolean?,
	to: string?,
}

return component {
	title = "Update pill",
	category = "Feedback",
	icon = "rocket",
	description = "A pill offering an app update: \"Moonframe 1.4 is ready\", an accent Restart button, a What changed link and a round ×.",
	when = "Tell the user an update is downloaded and ready to install, without interrupting. For the changelog itself use `release-notes`; for other notifications use `toast`/`toasts`; for a lasting in-page message use `notice`.",
	props = {
		["name"] = "app name; default \"Moonframe\"",
		["version"] = "the new version",
		["text"] = "the whole message instead of \"<name> <version> is ready\"",
		["icon"] = "icon before the text; default `rocket`",
		["restart"] = "msg for the Restart button; default `\"restart\"`",
		["restart_label"] = "default \"Restart\"",
		["notes"] = "msg for the What changed link (hidden when unset)",
		["notes_label"] = "default \"What changed\"",
		["dismiss"] = "msg for the round × (hidden when unset)",
		["downloading"] = "a spinner and \"Downloading…\" instead of Restart",
		["floating"] = "float it over the window in a corner instead of drawing it inline",
		["place"] = "corner when floating; default `bottom-end`",
		["to"] = "mod the buttons send to, if not the drawing one",
	},
	styles = {
		[".update-pill"] = {
			background = "background",
			border = "all",
			border_color = "accent/40",
			radius = 999,
			padding = { 4, 4, 4, 12 },
			gap = 10,
			shadow = "0 6px 18px -6px #00000040",
		},
		[".update-pill-floating"] = { background = "#00000000", border = "none", shadow = "none", padding = 0, margin = 16 },
		[".update-pill-icon"] = { color = "accent", size = "small" },
		[".update-pill-text"] = { size = "small", weight = "medium" },
		[".update-pill-link"] = { size = "small", color = "muted-foreground", padding = { 2, 4 }, radius = 6 },
		[".update-pill-link:hover"] = { color = "accent" },
		[".update-pill-restart"] = { radius = 999 },
		[".update-pill-close"] = { padding = 4, radius = 999, color = "muted-foreground", size = "small" },
		[".update-pill-close:hover"] = { background = "list-hover", color = "foreground" },
	},
	example = { name = "Moonframe", version = "1.4", notes = "show-notes", dismiss = "later" },

	build = function(props: Props)
		local label = string.format("%s %s", props.name or "Moonframe", props.version or "")
		local text = props.text or (if props.downloading then "Downloading " .. label .. "…" else label .. " is ready")
		local pill = ui.row {
			class = "update-pill",
			align = "center",
			props.downloading == true and ui.icon { name = "loader-circle", spin = true, class = "update-pill-icon" }
				or ui.icon { name = (props.icon or "rocket") :: any, class = "update-pill-icon" },
			ui.text { text, class = "update-pill-text" },
			props.notes ~= nil and ui.row {
				class = "update-pill-link",
				msg = props.notes,
				to = props.to,
				ui.text { props.notes_label or "What changed" },
			},
			props.downloading ~= true and ui.button {
				props.restart_label or "Restart",
				class = "update-pill-restart",
				variant = "primary",
				small = true,
				msg = props.restart or "restart",
				to = props.to,
			},
			props.dismiss ~= nil and ui.row {
				class = "update-pill-close",
				msg = props.dismiss,
				to = props.to,
				tooltip = "Later",
				ui.icon "close",
			},
		}
		if not props.floating then
			return ui.row { pill }
		end
		return ui.overlay {
			to = props.to,
			open = true,
			place = (props.place or "bottom-end") :: any,
			class = "update-pill-floating",
			content = pill,
		}
	end,
}
