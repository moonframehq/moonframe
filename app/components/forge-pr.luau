-- A branch's pull (or merge) request from `app.github` or `app.gitlab`:
-- its number tinted by where it stands, a checks dot and its lines added
-- and removed; a click opens it in the browser. With no request it shows
-- the checkout's own uncommitted lines instead, when given them.

local pr_badge = use "pr-badge"
local diff_counts = use "diff-counts"

type Props = {
	forge: any,
	branch: string?,
	pr: any,
	changes: any,
	brand: boolean?,
	open: boolean?,
}

-- The checks dot: the worst outcome there is.
local function checks(pr: any): string?
	local counts = pr.checks
	if not counts then
		return nil
	elseif (counts.failed or 0) > 0 then
		return "failed"
	elseif (counts.pending or 0) > 0 then
		return "pending"
	elseif (counts.passed or 0) > 0 then
		return "passed"
	end
	return nil
end

return component {
	title = "Forge PR",
	category = "Git",
	icon = "git-pull-request",
	description = "A branch's GitHub pull request or GitLab merge request, read from app.github or app.gitlab: number by state, checks dot, +/− lines; a click opens it.",
	when = "Wherever a branch shows its pull request: the title bar (the current branch, falling back to uncommitted lines), beside each worktree or branch in a sidebar (`branch = worktree.branch`). It reads the forge provider's data; for a number you already have use `pr-badge`, for a whole list row `pr-row`.",
	props = {
		["forge"] = "`app.github` or `app.gitlab` (nil while loading: draws nothing)",
		["branch"] = "the branch whose request to show, from `forge.branches`; default the current branch's (`forge.current_pr`)",
		["pr"] = "a pull request record to show instead of looking one up",
		["changes"] = "`{ added, deleted }` shown when there is no request (e.g. `app.git`), so the slot still says work is waiting",
		["brand"] = "show the forge's logo before the number",
		["open"] = "open the request in the browser on a click (default true)",
	},
	styles = {
		[".forge-pr"] = { gap = 6, padding = { 2, 6 }, radius = 6 },
		[".forge-pr-link:hover"] = { background = "foreground/8" },
		[".forge-pr-brand"] = { color = "muted-foreground" },
	},
	example = {
		forge = {
			available = true,
			host = "github.com",
			auth = { status = "signed_in", host = "github.com", device = true, cli = false },
			current_pr = {
				number = 850,
				title = "Add forge building blocks",
				tone = "ready",
				url = "https://github.com/moonframehq/moonframe/pull/850",
				additions = 1204,
				deletions = 96,
				checks = { total = 3, passed = 3, failed = 0, pending = 0, skipped = 0 },
			},
		},
		brand = true,
	},

	build = function(props: Props)
		local forge = props.forge
		local gitlab = forge ~= nil and forge.host ~= nil and forge.host ~= "github.com"
		local pr = props.pr
		if not pr and forge and forge.available then
			if props.branch then
				pr = forge.branches and forge.branches[props.branch]
			else
				pr = forge.current_pr
			end
		end
		if not pr then
			local changes = props.changes
			if changes and ((changes.added or 0) > 0 or (changes.deleted or 0) > 0) then
				return ui.row {
					class = "forge-pr",
					hug = true,
					align = "center",
					tooltip = "Uncommitted changes",
					diff_counts { added = changes.added, removed = changes.deleted },
				}
			end
			return ui.row { class = "forge-pr", hug = true }
		end
		local link = props.open ~= false and pr.url ~= nil
		return ui.row {
			class = if link then "forge-pr forge-pr-link" else "forge-pr",
			hug = true,
			shrink = false,
			align = "center",
			tooltip = pr.title,
			to = if link then "app:open" else nil,
			msg = if link then pr.url else nil,
			props.brand and ui.icon {
				name = (if gitlab then "brand-gitlab" else "brand-github") :: any,
				size = "small",
				class = "forge-pr-brand",
			},
			pr_badge {
				number = pr.number,
				kind = if gitlab then "mr" else "pr",
				tone = pr.tone,
				checks = checks(pr),
				tooltip = pr.title,
			},
			(pr.additions or pr.deletions) and diff_counts { added = pr.additions, removed = pr.deletions },
		}
	end,
}
