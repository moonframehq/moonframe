-- Page dots for an onboarding flow or a carousel, the current one
-- stretched into a bar, with optional Back and Next buttons around them.

type Props = {
	count: number,
	page: number,
	back: any,
	next: any,
	back_label: string?,
	next_label: string?,
	event: string?,
	to: string?,
}

return component {
	title = "Pager",
	category = "Navigation",
	icon = "dot",
	description = "Dots for pages with the current one stretched, and optional Back and Next buttons; sends `back`, `next` or { page = n }.",
	when = "Steps of an onboarding or welcome flow, or pages of a carousel. For history back/forward use `nav-buttons`; for switching views use `tabs` or `segmented`.",
	props = {
		["count"] = "number of pages",
		["page"] = "current page, from 1",
		["back"] = "message the Back button sends; no button when unset (hidden on the first page)",
		["next"] = "message the Next button sends; no button when unset",
		["back_label"] = "Back button text (default \"Back\")",
		["next_label"] = "Next button text (default \"Next\"; e.g. \"Get started\" on the last page)",
		["event"] = "key of the message a dot sends, default \"page\": { page = n }",
		["to"] = "mod to send messages to, if not the drawing one",
	},
	styles = {
		[".pager"] = { gap = 12 },
		[".pager-dots"] = { gap = 6 },
		[".pager-dot"] = { radius = 999, background = "muted-foreground/30" },
		[".pager-dot:hover"] = { background = "muted-foreground/60" },
		[".pager-dot:selected"] = { background = "accent" },
	},
	example = { count = 4, page = 2, back = "back", next = "next" },

	build = function(props: Props)
		local event = props.event or "page"
		local dots = {}
		for index = 1, props.count do
			local current = index == props.page
			dots[index] = ui.row {
				class = "pager-dot",
				width = if current then 18 else 6,
				height = 6,
				selected = current,
				msg = { [event] = index },
				to = props.to,
			}
		end
		local strip = ui.row { class = "pager-dots", align = "center", table.unpack(dots) }
		if props.back == nil and props.next == nil then
			return ui.row { class = "pager", align = "center", strip }
		end
		local first = props.page <= 1
		return ui.row {
			class = "pager",
			align = "center",
			justify = "between",
			grow = true,
			ui.row {
				width = 96,
				props.back ~= nil and not first and ui.button {
					props.back_label or "Back",
					variant = "ghost",
					msg = props.back,
					to = props.to,
				},
			},
			strip,
			ui.row {
				width = 96,
				justify = "end",
				props.next ~= nil and ui.button {
					props.next_label or "Next",
					variant = "primary",
					msg = props.next,
					to = props.to,
				},
			},
		}
	end,
}
