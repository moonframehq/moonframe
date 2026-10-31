-- A repository tree: click a chevron to fold a folder, a row to select it,
-- and switch the guide lines on and off.
local tree = use "tree"
local segmented = use "segmented"

-- Nodes mix optional fields; typed loosely for the checker.
local function loose(list: { any }): { any }
	return list
end

return mod {
	init = function(previous)
		return previous
			or {
				selected = "src/ledger.rs",
				guides = "on",
				expanded = { ["ledger-api"] = true, src = true, worktrees = true } :: { [string]: boolean },
			}
	end,

	update = function(state, msg)
		if typeof(msg) ~= "table" then
			return
		end
		if msg.fold then
			state.expanded[msg.fold] = not state.expanded[msg.fold]
		elseif msg.select then
			state.selected = msg.select
		elseif msg.guides then
			state.guides = msg.guides
		end
	end,

	view = function(state)
		local open = state.expanded
		local nodes: { any } = {
			{
				id = "ledger-api",
				label = "ledger-api",
				icon = "folder-git-2",
				detail = "main",
				expanded = open["ledger-api"],
				children = loose {
					{
						id = "src",
						label = "src",
						expanded = open.src,
						children = loose {
							{ id = "src/ledger.rs", label = "ledger.rs", icon = "file-text", tone = "warning", trailing = { ui.badge { "M", tone = "warning" } } },
							{ id = "src/currency.rs", label = "currency.rs", icon = "file-text", tone = "success", trailing = { ui.badge { "A", tone = "success" } } },
							{ id = "src/main.rs", label = "main.rs", icon = "file-text" },
						},
					},
					{ id = "tests", label = "tests", expanded = open.tests, children = loose { { id = "tests/ledger.rs", label = "ledger.rs", icon = "file-text" } } },
					{ id = "Cargo.toml", label = "Cargo.toml", icon = "file" },
				},
			},
			{
				id = "worktrees",
				label = "worktrees",
				icon = "list-tree",
				expanded = open.worktrees,
				children = loose {
					{ id = "wt-multi", label = "feat/multi-currency", detail = "~/w/ledger-multi", icon = "git-branch" },
					{ id = "wt-round", label = "fix/rounding", detail = "~/w/ledger-rounding", icon = "git-branch", trailing = { ui.badge { "needs you", tone = "danger" } } },
				},
			},
		}
		return ui.column {
			gap = "normal",
			width = 380,
			segmented {
				event = "guides",
				selected = state.guides,
				small = true,
				options = { { id = "on", label = "Guides" }, { id = "off", label = "No guides" } },
			},
			tree { nodes = nodes, selected = state.selected, guides = state.guides == "on" },
			ui.text { `Selected: {state.selected}`, size = "small", tone = "muted" },
		}
	end,
}
