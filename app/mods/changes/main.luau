-- The right sidebar: what changed in the repository the app runs from, as
-- the built-in git provider (providers/git) sees it. Files sit in their
-- folders; choosing one asks git for its diff, shown under the list.
local change_tree = use "change-tree"
local diff_view = use "diff-view"
local empty_state = use "empty-state"

type State = {
	-- The file whose diff shows, if any.
	selected: string?,
	-- Folders folded shut, by path.
	folded: { [string]: boolean },
}

-- Asks git for `path`'s diff, or to stop sending one.
local function ask(path: string?): { any }
	return { { to = "provider:git", msg = { diff = if path then path else false } } }
end

return mod {
	region = "right",
	title = "Changes",

	styles = {
		[".changes-summary"] = { padding = { 0, 8, 6, 8 } },
		[".changes-list"] = { padding = { 0, 4 } },
	},

	init = function(previous: any): (State, { any }?)
		local state: State = { selected = previous and previous.selected, folded = previous and previous.folded or {} }
		-- A diff chosen before a reload or a relaunch is asked for again.
		return state, if state.selected then ask(state.selected) else nil
	end,

	update = function(state: State, msg)
		if type(msg) ~= "table" then
			if msg == "close" then
				state.selected = nil
				return state, ask(nil)
			end
			return nil
		end
		if msg.pick then
			-- Choosing the open file again closes its diff.
			state.selected = if state.selected == msg.pick then nil else msg.pick
			return state, ask(state.selected)
		elseif msg.fold then
			state.folded[msg.fold] = (if state.folded[msg.fold] then nil else true) :: any
		end
		return nil
	end,

	view = function(state: State, app)
		local git = app.git
		local body: any
		local actions: { any } = {
			ui.button { icon = "refresh-cw", variant = "ghost", small = true, msg = "refresh", to = "provider:git", tooltip = "Look again" },
		}
		if git == nil then
			body = ui.text { "Loading…", tone = "muted", size = "small" }
		elseif not git.available or not git.repository then
			body = empty_state {
				icon = "git-branch",
				title = "Not a git repository",
				message = git.reason or "Run the app from a repository to see its changes here.",
			}
		elseif git.changed == 0 then
			body = empty_state { icon = "circle-check", title = "Nothing changed", message = `{git.branch} is clean.` }
		else
			-- The lines change-tree reads, and each file's status for the diff.
			local lines: { string } = {}
			local status: { [string]: string } = {}
			for _, file in git.files do
				table.insert(lines, `{file.status}\t{file.added}\t{file.deleted}\t{file.path}`)
				status[file.path] = file.status
			end
			local more: any = if git.files_truncated then ui.text { "and more…", tone = "muted", size = "small" } else false
			local list = ui.column {
				gap = "none",
				ui.row {
					class = "changes-summary",
					gap = "small",
					align = "center",
					ui.text { `{git.changed} {if git.changed == 1 then "file" else "files"}`, size = "small", tone = "muted" },
					ui.text { `+{git.added}`, size = "small", tone = "success" },
					ui.text { `−{git.deleted}`, size = "small", tone = "danger" },
				},
				ui.column {
					class = "changes-list",
					gap = "none",
					change_tree { files = lines, selected = state.selected, folded = state.folded },
					more,
				},
			}
			local diff = git.diff
			if state.selected and status[state.selected] then
				local shown: any = if diff and diff.path == state.selected
					then diff_view {
						path = diff.path,
						status = status[diff.path],
						hunks = diff.hunks,
						grow = true,
						actions = { ui.button { icon = "close", variant = "ghost", small = true, msg = "close", tooltip = "Close the diff" } },
					}
					else ui.text { "Reading the diff…", tone = "muted", size = "small" }
				body = ui.vsplit {
					id = "changes",
					ui.pane { scroll = true, list },
					ui.pane { size = 360, min = 120, shown },
				}
			else
				body = ui.column { grow = true, scroll = true, gap = "none", list }
			end
		end
		return ui.panel { title = "Changes", grow = true, actions = actions, body }
	end,
}
