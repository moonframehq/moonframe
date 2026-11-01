-- One line made of runs: plain words, bold names, muted verbs, monospace
-- ids and small outlined tags, as an activity log or a notification says
-- what happened: **Order #1213** marked as [Fulfilled]. It is one row, so
-- it truncates rather than wraps; long lines belong in `ui.text`.

type Part = {
	text: string?,
	strong: boolean?,
	muted: boolean?,
	mono: boolean?,
	tag: string?,
	tone: string?,
	icon: string?,
	msg: any,
}

type Props = {
	parts: { any },
	size: string?,
	to: string?,
}

-- The preview's runs, typed loosely: they mix kinds of entries.
local EXAMPLE: { any } = {
	{ text = "Order #1213", strong = true },
	{ text = "marked as", muted = true },
	{ tag = "Fulfilled" },
}

return component {
	title = "Rich Text",
	category = "Data",
	icon = "case-sensitive",
	description = "A line of runs: plain, bold, muted or mono words and small outlined tag pills, side by side; a run can be clickable.",
	when = "A sentence that mixes emphasis and labels in one line: an audit log's activity, a notification, a table cell (\"Order #1213 marked as [Fulfilled]\"). For a paragraph use `ui.text`; for a lone label use `pill` or `ui.badge`; for several labels use `tags`.",
	props = {
		["parts"] = "list of strings (plain) or { text, strong?, muted?, mono?, icon?, msg? } or { tag = \"Fulfilled\", tone? (accent, success, warning, danger) }",
		["size"] = "text size: small or normal (default)",
		["to"] = "mod to send a run's `msg` to, if not the drawing one",
	},
	styles = {
		[".rich-text"] = { gap = 5 },
		[".rich-text-strong"] = { weight = "medium", color = "foreground" },
		[".rich-text-muted"] = { color = "muted-foreground" },
		[".rich-text-mono"] = { font = "mono" },
		[".rich-text-link:hover"] = { color = "accent" },
		[".rich-text-tag"] = { padding = { 1, 7 }, radius = 6, border = "all", border_color = "border", background = "background" },
		[".rich-text-tag-text"] = { size = "small", weight = "medium" },
		[".rich-text-tag-accent"] = { border_color = "accent/40", background = "accent/8", color = "accent" },
		[".rich-text-tag-success"] = { border_color = "success/40", background = "success/8", color = "success" },
		[".rich-text-tag-warning"] = { border_color = "warning/40", background = "warning/8", color = "warning" },
		[".rich-text-tag-danger"] = { border_color = "danger/40", background = "danger/8", color = "danger" },
	},
	example = {
		parts = EXAMPLE,
	},
	usage = [[local rich_text = use "rich-text"

rich_text { parts = {
	{ text = "Product \"Harbor Hoodie\"", strong = true },
	{ text = "updated with", muted = true },
	{ tag = "New variant" },
} }]],

	build = function(props: Props)
		local runs = {}
		for _, entry in props.parts do
			local part: Part = if type(entry) == "string" then { text = entry } else entry
			if part.tag then
				local class = { "rich-text-tag" }
				if part.tone then
					table.insert(class, "rich-text-tag-" .. part.tone)
				end
				table.insert(runs, ui.row {
					class = class,
					shrink = false,
					align = "center",
					msg = part.msg,
					to = props.to,
					ui.text { part.tag, class = "rich-text-tag-text" },
				})
			else
				local class = {}
				if part.strong then
					table.insert(class, "rich-text-strong")
				end
				if part.muted then
					table.insert(class, "rich-text-muted")
				end
				if part.mono then
					table.insert(class, "rich-text-mono")
				end
				if part.msg ~= nil then
					table.insert(class, "rich-text-link")
				end
				table.insert(runs, ui.row {
					gap = "small",
					align = "center",
					msg = part.msg,
					to = props.to,
					part.icon and ui.icon { name = part.icon :: any, class = class },
					ui.text { part.text or "", class = class, size = (props.size or "normal") :: any, truncate = true },
				})
			end
		end
		return ui.row { class = "rich-text", align = "center", table.unpack(runs) }
	end,
}
