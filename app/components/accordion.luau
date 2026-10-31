-- Several disclosures stacked in one frame, divided by rules. Stateless: the
-- mod keeps which are open, either one id (`open = "checks"`) or a set
-- (`open = { checks = true, files = true }`) when several may be open.
-- Clicking a header sends `{ toggle = id }`.

local collapsible = use "collapsible"

type Item = {
	id: string,
	title: string,
	summary: string?,
	icon: string?,
	-- The body: one element or a list of them.
	body: any,
}

type Props = {
	items: { Item },
	open: any,
	event: string?,
	to: string?,
}

local function is_open(open: any, id: string): boolean
	if typeof(open) == "table" then
		return open[id] == true
	end
	return open == id
end

return component {
	title = "Accordion",
	category = "Containers",
	icon = "gallery-vertical-end",
	description = "Several foldable sections in one frame, one or many open at a time; sends { toggle = id }.",
	when = "A stack of foldable sections of the same kind: a merge request's checks, files and activity, or groups of settings. The mod decides whether opening one closes the others. For one foldable block use `collapsible`; for sidebar groups use `sidebar-section`; for switching between views use `tabs`.",
	props = {
		["items"] = "list of { id, title, summary?, icon?, body (an element or a list of elements) }",
		["open"] = "the open item's id, or a set `{ [id] = true }` when several may be open",
		["event"] = "key of the msg sent on a header click; default `toggle`, sending `{ toggle = id }`",
		["to"] = "mod to send the msg to, if not the drawing one",
	},
	styles = {
		[".accordion"] = { background = "background", border = "all", border_color = "border", radius = 10, padding = 4 },
		[".accordion-divider"] = { background = "border" },
	},
	usage = [[local accordion = use "accordion"

accordion { open = state.open, items = {
	{ id = "checks", title = "Checks", summary = "3 of 4 passed", icon = "circle-check",
		body = ui.text { "lint, test and build passed; deploy is running.", tone = "muted" } },
	{ id = "files", title = "Files changed", summary = "2", icon = "file",
		body = { ui.text "src/ledger.rs", ui.text "src/currency.rs" } },
} }
-- update: one open at a time: state.open = state.open ~= msg.toggle and msg.toggle or nil]],
	example = {
		open = "checks",
		items = {
			{
				id = "checks",
				title = "Checks",
				summary = "3 of 4 passed",
				icon = "circle-check",
				body = {
					ui.row { gap = "small", align = "center", ui.icon { name = "check", tone = "success" }, ui.text { "lint · cargo clippy", size = "small" } },
					ui.row { gap = "small", align = "center", ui.icon { name = "check", tone = "success" }, ui.text { "test · cargo nextest", size = "small" } },
					ui.row { gap = "small", align = "center", ui.icon { name = "loader-circle", spin = true, tone = "warning" }, ui.text { "deploy · preview", size = "small" } },
				},
			},
			{ id = "files", title = "Files changed", summary = "2", icon = "file-text", body = ui.text "src/ledger.rs" },
			{ id = "activity", title = "Activity", summary = "updated 3d ago", icon = "calendar", body = ui.text "penso opened 7d ago" },
		},
	},

	build = function(props: Props)
		local children = {}
		for index, item in props.items do
			if index > 1 then
				children[#children + 1] = ui.row { class = "accordion-divider", height = 1 }
			end
			local body = item.body
			local parts = if typeof(body) == "table" and body.type == nil then body else { body }
			children[#children + 1] = collapsible {
				title = item.title,
				summary = item.summary,
				icon = item.icon,
				open = is_open(props.open, item.id),
				toggle = { [props.event or "toggle"] = item.id },
				to = props.to,
				flat = true,
				table.unpack(parts),
			}
		end
		return ui.column { class = "accordion", gap = "none", table.unpack(children) }
	end,
}
