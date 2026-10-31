-- The toast stack: every item drawn as a `toast` in one window corner,
-- newest last, floating over the window without a backdrop (clicks around
-- it go through). Each × sends `{ dismiss = id }`.

local toast = use "toast"

type Item = {
	id: any,
	level: string?,
	icon: any,
	source: string?,
	title: string?,
	body: string?,
	full: boolean?,
	action: any,
}

type Props = {
	items: { Item },
	place: string?,
	width: number?,
	-- Most toasts shown; the rest wait (oldest first are dropped from view).
	max: number?,
	event: string?,
	to: string?,
}

return component {
	title = "Toasts",
	category = "Feedback",
	icon = "gallery-vertical-end",
	description = "A stack of toast cards floating in a window corner; each × sends { dismiss = id }.",
	when = "The app's notifications: keep a list in the mod's state, push to it, remove on `dismiss` (or after a timer). For one card inline use `toast`; for a persistent callout in a page use `notice`; for an update offer use `update-pill`.",
	props = {
		["items"] = "list of { id, level, icon, source, title, body, full, action = { label, msg } }",
		["place"] = "overlay corner: `bottom-end` (default), `bottom-start`, `top-end`, `top-start`, `top`, `bottom`",
		["width"] = "card width in pixels; default 340",
		["max"] = "most cards shown at once (the newest); default 4",
		["event"] = "key of the dismiss msg; default `dismiss`",
		["to"] = "mod the × and actions send to, if not the drawing one",
	},
	styles = {
		[".toasts"] = {
			background = "#00000000",
			border = "none",
			shadow = "none",
			padding = 0,
			margin = 16,
		},
		[".toasts-stack"] = { gap = 8 },
	},
	example = {
		items = {
			{ id = 1, level = "success", source = "moonframe", title = "Worktree created", body = "feature/inspect at ~/code/app-inspect" },
			{
				id = 2,
				level = "warning",
				source = "Local",
				title = "Local Git and PR details unavailable",
				body = "The daemon socket directory must be owned by your user and not world-writable.",
				action = { label = "Reconnect", msg = "reconnect" },
			},
			{ id = 3, level = "error", icon = true, source = "devbox", title = "Agent exited with code 1", body = "cargo test failed in crates/moonframe-core" },
		},
	},

	build = function(props: Props)
		local items = props.items or {}
		local first = math.max(1, #items - (props.max or 4) + 1)
		local cards = {}
		for index = first, #items do
			local item = items[index]
			cards[#cards + 1] = toast {
				id = item.id,
				level = item.level,
				icon = item.icon,
				source = item.source,
				title = item.title,
				body = item.body,
				full = item.full,
				action = item.action,
				event = props.event,
				width = props.width,
				to = props.to,
			}
		end
		return ui.overlay {
			to = props.to,
			open = #cards > 0,
			place = (props.place or "bottom-end") :: any,
			class = "toasts",
			content = ui.column { class = "toasts-stack", table.unpack(cards) },
		}
	end,
}
