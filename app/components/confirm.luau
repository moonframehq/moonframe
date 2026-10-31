-- A yes/no question in a small modal: a title, a message, an optional
-- scrolling detail and a list of what is affected, and Cancel / Confirm.
-- `danger` turns the confirm button red for destructive actions.

local dialog = use "dialog"

type Props = {
	open: boolean?,
	title: string,
	message: string?,
	detail: string?,
	items: { any }?,
	items_title: string?,
	confirm: any,
	cancel: any,
	confirm_label: string?,
	cancel_label: string?,
	danger: boolean?,
	icon: string?,
	to: string?,
}

return component {
	title = "Confirm",
	category = "Overlays",
	icon = "circle-help",
	description = "A small modal question with a message, optional detail and affected items, and Cancel / Confirm buttons.",
	when = "Ask before doing something, especially destructive (close a window with running agents, delete a branch). For a modal with a form or custom body use `dialog`; for an undo-able action prefer doing it and showing a `toast` with an Undo action.",
	props = {
		["open"] = "show it",
		["title"] = "the question",
		["message"] = "one or two lines under the title",
		["detail"] = "longer muted text in a scrolling box (an error, a path list)",
		["items"] = "affected things, listed: strings or { label, detail, icon }",
		["items_title"] = "heading over `items`, e.g. \"Agents that will be interrupted\"",
		["confirm"] = "msg sent by the confirm button and ⌘↩; default `\"confirm\"`",
		["cancel"] = "msg sent by Cancel, Escape and the backdrop; default `\"cancel\"`",
		["confirm_label"] = "confirm button text; default \"Confirm\"",
		["cancel_label"] = "cancel button text; default \"Cancel\"",
		["danger"] = "red confirm button and a warning icon",
		["icon"] = "icon before the title",
		["to"] = "mod the buttons send to, if not the drawing one",
	},
	styles = {
		[".confirm-message"] = {},
		[".confirm-detail"] = {
			background = "muted/60",
			border = "all",
			border_color = "border",
			radius = 8,
			padding = { 8, 10 },
			size = "small",
			color = "muted-foreground",
			font = "mono",
		},
		[".confirm-items-title"] = { size = "small", weight = "medium", color = "muted-foreground" },
		[".confirm-items"] = { border = "all", border_color = "border", radius = 8, padding = 4, gap = 0 },
		[".confirm-item"] = { padding = { 3, 8 }, gap = 8, radius = 6 },
		[".confirm-item-icon"] = { color = "muted-foreground", size = "small" },
		[".confirm-item-detail"] = { size = "small", color = "muted-foreground" },
	},
	example = {
		open = true,
		title = "Close this window?",
		message = "Closing it interrupts the agents still running here.",
		items_title = "Agents that will be interrupted",
		items = {
			{ label = "Claude", detail = "feature/inspect · writing tests", icon = "agent-claude" },
			{ label = "Codex", detail = "fix/menus · running cargo test", icon = "agent-codex" },
		},
		confirm_label = "Close window",
		cancel_label = "Keep open",
		danger = true,
	},

	build = function(props: Props)
		local confirm = props.confirm or "confirm"
		local cancel = props.cancel or "cancel"

		local list: any = false
		if props.items and #props.items > 0 then
			local rows = {}
			for index, item in props.items do
				local entry = if type(item) == "table" then item else { label = tostring(item) }
				rows[index] = ui.row {
					class = "confirm-item",
					shrink = false,
					align = "center",
					entry.icon ~= nil and ui.icon { name = entry.icon :: any, class = "confirm-item-icon" },
					ui.text { entry.label, weight = "medium", truncate = true },
					entry.detail ~= nil and ui.text { entry.detail, class = "confirm-item-detail", truncate = true },
				}
			end
			list = ui.column {
				gap = "small",
				props.items_title ~= nil and ui.text { props.items_title, class = "confirm-items-title" },
				ui.column { class = "confirm-items", table.unpack(rows) },
			}
		end

		local detail: any = false
		if props.detail then
			detail = ui.column {
				class = "confirm-detail",
				height = 96,
				scroll = true,
				ui.text { props.detail },
			}
		end

		local body = {}
		for _, child in { props.message ~= nil and ui.text { props.message, class = "confirm-message" } or false, detail or false, list or false } do
			if child then
				body[#body + 1] = child
			end
		end

		return dialog {
			open = props.open,
			dismiss = cancel,
			title = props.title,
			icon = props.icon or (props.danger and "triangle-alert" or nil),
			size = "small",
			width = 420,
			hint = false,
			plain = true,
			close = false,
			submit = confirm,
			class = props.danger and "confirm confirm-danger" or "confirm",
			to = props.to,
			buttons = {
				{ label = props.cancel_label or "Cancel", msg = cancel, variant = "ghost" },
				{ label = props.confirm_label or "Confirm", msg = confirm, variant = props.danger and "danger" or "primary" },
			},
			tone = props.danger and "danger" or nil,
			table.unpack(body),
		}
	end,
}
