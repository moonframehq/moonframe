-- The status bar under the terminals: the work tree's folder, its branch,
-- its changes (which open source control), Commit (to the message field)
-- and Create PR (to the pull requests), and on the right where the shells
-- live: herdr-core or tmux keep them when the window closes.

type Item = { icon: string?, text: string, tone: string?, msg: any, to: string?, tooltip: string?, chevron: boolean? }

return mod {
	title = "Status",

	styles = {
		[".tstatus"] = { background = "background", border = "top", border_color = "border", color = "muted-foreground" },
		[".tstatus-item"] = { padding = { 0, 10 }, gap = 6, border = "right", border_color = "border" },
		[".tstatus-item:hover"] = { background = "list-hover", color = "foreground" },
		[".tstatus-end"] = { padding = { 0, 12 }, gap = 6, border = "left", border_color = "border" },
		[".tstatus-warning"] = { color = "warning" },
		[".tstatus-dot"] = { radius = 999, background = "accent" },
		[".tstatus-dot-off"] = { background = "muted-foreground" },
	},

	view = function(_state, app)
		local projects = app.projects
		local current = projects and projects.current
		local herdr = app.herdr

		local function item(spec: Item)
			return ui.row {
				class = if spec.tone then { "tstatus-item", `tstatus-{spec.tone}` } else "tstatus-item",
				align = "center",
				shrink = false,
				msg = spec.msg,
				to = spec.to,
				tooltip = spec.tooltip,
				spec.icon and ui.icon { name = spec.icon :: any, size = "small" },
				ui.text { spec.text, size = "small", truncate = true },
				spec.chevron and ui.icon { name = "chevron-down", size = "small" },
			}
		end

		local items = {}
		if current then
			table.insert(items, item { icon = "folder", text = `{current.home}/{current.name}`, tooltip = current.path })
			table.insert(items, item { icon = "git-merge", text = current.branch })
			table.insert(items, item {
				icon = "arrow-up-down",
				text = `{current.count} {if current.count == 1 then "Change" else "Changes"}`,
				tone = if current.count > 0 then "warning" else nil,
				msg = { tab = "branch" },
				to = "source-control",
				tooltip = `+{current.added} −{current.deleted}`,
			})
			table.insert(items, item { icon = "circle-dot", text = "Commit", chevron = true, msg = "focus-message", to = "source-control" })
			table.insert(items, item { icon = "git-pull-request", text = "Create PR", chevron = true, msg = { tab = "prs" }, to = "source-control" })
		else
			table.insert(items, item { icon = "folder", text = "~" })
		end

		local kept = herdr and (herdr.installed or herdr.tmux)
		local server = if herdr == nil
			then "Terminals"
			elseif herdr.installed then "herdr-core · Connected"
			elseif herdr.tmux then "tmux · Connected"
			else "Local shells"

		return ui.row {
			class = "tstatus",
			grow = true,
			height = 28,
			shrink = false,
			align = "stretch",
			gap = "none",
			ui.row { align = "stretch", gap = "none", table.unpack(items) },
			ui.spacer(),
			ui.row {
				class = "tstatus-end",
				align = "center",
				shrink = false,
				tooltip = if kept then "Shells keep running when the window closes" else "Shells end with the window; install herdr-core or tmux to keep them",
				ui.row { class = if kept then "tstatus-dot" else { "tstatus-dot", "tstatus-dot-off" }, width = 6, height = 6 },
				ui.text { server, size = "small" },
			},
		}
	end,
}
