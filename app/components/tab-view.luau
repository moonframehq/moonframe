-- One tab's content, from its record: a component kind is drawn with the
-- tab's props and id (so a terminal or browser is the tab's own), a mod kind
-- is mounted as an instance of its own. `tabbed` draws it under its strip;
-- with the strip elsewhere (`tabbed { show = "bar" }`, as in the title bar),
-- a mod draws it from the `tabs/<id>` topic. What a kind reports (a browser's
-- page) arrives at the drawing mod as `{ tab_report = <tab id>, url, title, ... }`;
-- publish it on `tabs/<id>/report` and the tab keeps it.

type Props = {
	-- { id, kind, title?, props }, as `tabbed` keeps it.
	tab: any,
	-- Its entry in `app.kinds`, for the kind's own props and report.
	kind: any,
}

return component {
	title = "Tab view",
	category = "Navigation",
	icon = "panel-bottom-open",
	description = "One tab's content from its record: its component with the tab's props, or an instance of its mod.",
	when = "Drawing the selected tab of a `tabbed` bar somewhere other than under it, from the `tabs/<id>` topic. `tabbed` uses it for its own content.",
	props = {
		["tab"] = "the tab: { id, kind, title?, props }, as `tabbed` publishes it",
		["kind"] = "its entry in `app.kinds`",
	},
	usage = [[local tab_view = use "tab-view"

-- in a mod that subscribes to "tabs/main":
tab_view { tab = state.tab, kind = kind_of(app.kinds, state.tab.kind) }]],

	example = {
		tab = { id = "example", kind = "component:empty-state", props = { title = "A tab's content", icon = "panel-bottom-open" } },
	},

	build = function(props: Props)
		local tab, kind = props and props.tab, props and props.kind
		if not tab then
			return ui.column {
				grow = true,
				align = "center",
				justify = "center",
				ui.text { "No tab selected", tone = "muted" },
			}
		end
		local merged = table.clone(kind and kind.props or {})
		for key, value in tab.props or {} do
			merged[key] = value
		end
		local of, name = string.match(tab.kind, "^(%a+):(.+)$")
		if of == "mod" and kind == nil then
			-- Its mod is gone (a mockup deleted while its tab was open).
			return ui.column {
				grow = true,
				align = "center",
				justify = "center",
				ui.text { `{name} is no longer installed; close this tab.`, tone = "muted" },
			}
		end
		if of == "mod" then
			-- The tab frames it, so a mod that draws its own panel elsewhere
			-- (the explorer in the sidebar) should draw its content bare.
			merged.framed = true
			merged.id = tab.id
			return ui.mount { name :: any, id = tab.id, props = merged }
		end
		if kind and kind.report then
			merged[kind.report] = { tab_report = tab.id }
			-- A terminal reports what its shell does the same way.
			if kind.report == "exit" then
				merged.shell = merged.exit
			end
		end
		merged.id = tab.id
		return use(name :: any)(merged)
	end,
}
