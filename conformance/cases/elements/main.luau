-- Every shared element once, with its primary value given each way.
return mod {
	view = function()
		return ui.column {
			gap = "normal",
			padding = "large",
			grow = true,
			ui.panel {
				title = "Elements",
				actions = { ui.button { icon = "plus", tooltip = "Add", msg = "add" } },
				ui.row {
					gap = "small",
					align = "center",
					justify = "between",
					ui.icon "git-branch",
					ui.text { "main", tone = "muted", size = "small", weight = "semibold" },
					ui.badge { "3", tone = "accent" },
				},
				ui.item { "Inbox", detail = "12", badge = "new", selected = true, msg = { open = "inbox" } },
				ui.button { "Save", variant = "primary", msg = "save" },
				ui.button { label = "Later", variant = "ghost", small = true, disabled = true },
				ui.input { id = "q", value = "", placeholder = "Search", change = "q", submit = "go" },
				ui.input { id = "notes", value = "one\ntwo", multiline = true, change = "notes" },
				ui.markdown "# Title",
				ui.text "plain",
				ui.icon { name = "trash-2", tone = "danger" },
				ui.divider(),
				ui.spacer(),
			},
		}
	end,
}
