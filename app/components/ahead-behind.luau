-- Commits ahead of and behind the upstream, "↑2 ↓1". A side that is zero
-- is hidden, and so is the whole thing when both are, unless `synced`.

type Props = {
	ahead: number?,
	behind: number?,
	synced: boolean?,
	tooltip: string?,
}

return component {
	title = "Ahead / Behind",
	category = "Git",
	icon = "git-commit-horizontal",
	description = "Commits ahead and behind the upstream as ↑2 ↓1, hiding a side that is zero.",
	when = "A branch's sync state beside its name: sidebars, title bars, branch lists. For lines changed use `diff-counts`.",
	props = {
		["ahead"] = "commits to push",
		["behind"] = "commits to pull",
		["synced"] = "when both are zero, show a muted check instead of nothing",
		["tooltip"] = "text under the pointer (default \"2 ahead, 1 behind\")",
	},
	styles = {
		[".ahead-behind"] = { gap = 6, color = "muted-foreground" },
		[".ahead-behind-count"] = { size = "small", font = "mono" },
		[".ahead-behind-part"] = { gap = 1 },
		[".ahead-behind-ahead"] = { color = "success" },
		[".ahead-behind-behind"] = { color = "warning" },
	},
	example = { ahead = 2, behind = 1 },

	build = function(props: Props)
		local ahead = props.ahead or 0
		local behind = props.behind or 0
		if ahead == 0 and behind == 0 then
			if not props.synced then
				return nil
			end
			return ui.row {
				class = "ahead-behind",
				align = "center",
				tooltip = props.tooltip or "Up to date",
				ui.icon { name = "check" },
			}
		end
		local function part(count: number, icon: string, class: string)
			return ui.row {
				class = { "ahead-behind-part", class },
				align = "center",
				ui.icon { name = icon :: any, size = "small" },
				ui.text { tostring(count), class = "ahead-behind-count" },
			}
		end
		return ui.row {
			class = "ahead-behind",
			shrink = false,
			align = "center",
			tooltip = props.tooltip or `{ahead} ahead, {behind} behind`,
			ahead > 0 and part(ahead, "arrow-up", "ahead-behind-ahead"),
			behind > 0 and part(behind, "arrow-down", "ahead-behind-behind"),
		}
	end,
}
