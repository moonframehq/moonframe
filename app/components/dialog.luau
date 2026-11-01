-- A modal dialog: a card centred over a dimmed window, with a header (icon,
-- title, subtitle, an Esc hint and a close button), a body (its children)
-- and a footer of right-aligned buttons. Stateless: the mod keeps `open`
-- and closes it when `dismiss` arrives (Escape, a backdrop click, ×).

type Button = {
	label: string,
	msg: any,
	variant: string?,
	disabled: boolean?,
}

type Props = {
	open: boolean?,
	dismiss: any,
	title: string?,
	subtitle: string?,
	icon: string?,
	tone: string?,
	size: string?,
	width: number?,
	height: number?,
	place: string?,
	backdrop: boolean?,
	-- The Esc hint and the × in the header; both on by default.
	hint: boolean?,
	-- No rules between header, body and footer: an alert rather than a form.
	plain: boolean?,
	close: boolean?,
	buttons: { Button }?,
	actions: { any }?,
	-- Muted text on the footer's left, such as "Changes apply at once".
	note: string?,
	-- Sent by ⌘↩ (Ctrl+Enter elsewhere).
	submit: any,
	keys: { [string]: any }?,
	to: string?,
	class: string?,
	[number]: any,
}

local WIDTHS = { small = 360, medium = 480, large = 640 }

return component {
	title = "Dialog",
	category = "Overlays",
	icon = "frame",
	description = "A modal card over a dimmed window: header with title, Esc hint and ×, a body, and right-aligned footer buttons.",
	when = "Any modal that asks for attention: a form, a choice, details. For a yes/no question use `confirm`; for a panel sliding from an edge use `sheet`; for a non-blocking message use `toast` or `notice`; for content beside a control use `ui.popover`.",
	props = {
		["open"] = "show it",
		["dismiss"] = "msg sent by Escape, a backdrop click or the ×",
		["title"] = "header title",
		["subtitle"] = "muted line under the title",
		["icon"] = "icon before the title",
		["tone"] = "icon color: `accent` (default), `success`, `warning` or `danger`",
		["size"] = "`small` (360), `medium` (480, default) or `large` (640)",
		["width"] = "pixels, instead of `size`",
		["height"] = "pixels for the body, which then scrolls",
		["place"] = "overlay placement, default `center` (`top` for a palette-like dialog)",
		["backdrop"] = "dim the window behind; default true",
		["plain"] = "no lines between header, body and footer (alerts, confirmations)",
		["hint"] = "show the Esc key hint in the header; default true",
		["close"] = "show the × button; default true",
		["buttons"] = "footer buttons { { label, msg, variant, disabled } }, right-aligned",
		["actions"] = "footer elements instead of `buttons`",
		["note"] = "muted text on the footer's left",
		["submit"] = "msg sent by secondary-enter (⌘↩ / Ctrl+Enter)",
		["keys"] = "more keys for the overlay, merged with escape and submit",
		["to"] = "mod the buttons and × send to, if not the drawing one",
		["class"] = "extra class on the card",
		["[children]"] = "the body",
	},
	styles = {
		[".dialog"] = {
			background = "background",
			border = "all",
			border_color = "border",
			radius = 12,
			padding = 0,
		},
		[".dialog-header"] = { padding = { 14, 16, 12, 16 }, gap = 10, border = "bottom", border_color = "border" },
		[".dialog-header-plain"] = { border = "none", padding = { 16, 16, 0, 16 } },
		[".dialog-footer-plain"] = { border = "none", background = "#00000000", padding = { 4, 16, 14, 16 } },
		[".dialog-icon"] = { color = "accent", size = "large" },
		[".dialog-icon-success"] = { color = "success" },
		[".dialog-icon-warning"] = { color = "warning" },
		[".dialog-icon-danger"] = { color = "danger" },
		[".dialog-title"] = { weight = "semibold", size = "large" },
		[".dialog-subtitle"] = { size = "small", color = "muted-foreground" },
		[".dialog-hint"] = {
			size = "small",
			color = "muted-foreground",
			border = "all",
			border_color = "border",
			radius = 5,
			padding = { 0, 6 },
		},
		[".dialog-close"] = { padding = 4, radius = 6, color = "muted-foreground" },
		[".dialog-close:hover"] = { background = "list-hover", color = "foreground" },
		[".dialog-body"] = { padding = 16, gap = 10 },
		[".dialog-footer"] = { padding = { 10, 16 }, gap = 8, border = "top", border_color = "border", background = "muted/40" },
		[".dialog-note"] = { size = "small", color = "muted-foreground" },
	},
	usage = [[local dialog = use "dialog"

dialog { open = state.renaming, dismiss = "close", title = "Rename workspace",
	subtitle = "moonframe · ~/code/moonframe", icon = "pencil", size = "small", submit = "save",
	buttons = { { label = "Cancel", msg = "close", variant = "ghost" }, { label = "Rename", msg = "save", variant = "primary" } },
	ui.text "The new name shows in the sidebar and the window title.",
}]],
	example = {
		open = true,
		dismiss = "close",
		title = "Remove worktree",
		subtitle = "feature/inspect · ~/code/app-inspect",
		icon = "folder-git-2",
		submit = "remove",
		note = "⌘↩ to confirm",
		buttons = {
			{ label = "Cancel", msg = "close", variant = "ghost" },
			{ label = "Remove worktree", msg = "remove", variant = "danger" },
		},
		ui.text { "The worktree folder and its untracked files are deleted. The branch stays.", size = "small" },
		ui.text { "3 files changed, 1 untracked", size = "small", tone = "muted" },
	},

	build = function(props: Props)
		local keys: { [string]: any } = {}
		local extra: { [string]: any } = props.keys or {}
		for name, msg in pairs(extra) do
			keys[name] = msg
		end
		if props.submit ~= nil and keys["secondary-enter"] == nil then
			keys["secondary-enter"] = props.submit
		end
		if props.dismiss ~= nil and keys.escape == nil then
			keys.escape = props.dismiss
		end

		local header: any = false
		if props.title or props.icon then
			header = ui.row {
				class = props.plain and "dialog-header dialog-header-plain" or "dialog-header",
				shrink = false,
				align = "center",
				props.icon ~= nil and ui.icon {
					name = props.icon :: any,
					class = props.tone and ("dialog-icon dialog-icon-" .. props.tone) or "dialog-icon",
				},
				ui.column {
					grow = true,
					gap = "none",
					props.title ~= nil and ui.text { props.title, class = "dialog-title", truncate = true },
					props.subtitle ~= nil and ui.text { props.subtitle, class = "dialog-subtitle", truncate = true },
				},
				props.hint ~= false and props.dismiss ~= nil and ui.row { class = "dialog-hint", ui.text "esc" },
				props.close ~= false and props.dismiss ~= nil and ui.row {
					class = "dialog-close",
					msg = props.dismiss,
					to = props.to,
					tooltip = "Close",
					ui.icon "close",
				},
			}
		end

		local footer_items = {}
		for _, button in ipairs(props.buttons or {}) do
			footer_items[#footer_items + 1] = ui.button {
				button.label,
				msg = button.msg,
				to = props.to,
				variant = (button.variant or "normal") :: any,
				disabled = button.disabled,
			}
		end
		for _, action in ipairs(props.actions or {}) do
			footer_items[#footer_items + 1] = action
		end
		local footer: any = false
		if #footer_items > 0 or props.note then
			footer = ui.row {
				class = props.plain and "dialog-footer dialog-footer-plain" or "dialog-footer",
				shrink = false,
				align = "center",
				props.note ~= nil and ui.text { props.note, class = "dialog-note" },
				ui.spacer(),
				table.unpack(footer_items),
			}
		end

		local body: any = false
		if #props > 0 then
			body = ui.column {
				class = "dialog-body",
				height = props.height,
				scroll = props.height ~= nil,
				table.unpack(props),
			}
		end

		return ui.overlay {
			open = props.open,
			dismiss = props.dismiss,
			place = (props.place or "center") :: any,
			backdrop = props.backdrop ~= false,
			width = props.width or WIDTHS[props.size or "medium"] or 480,
			keys = keys,
			to = props.to,
			class = props.class and ("dialog " .. props.class) or "dialog",
			content = ui.column { gap = "none", header, body, footer },
		}
	end,
}
