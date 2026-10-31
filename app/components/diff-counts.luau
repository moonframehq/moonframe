-- Lines added and removed, "+38 −16", in the success and danger colours
-- and the mono font, with an optional count of files.

type Props = {
	added: number?,
	removed: number?,
	files: number?,
	size: string?,
	zero: boolean?,
}

return component {
	title = "Diff Counts",
	category = "Git",
	icon = "git-compare",
	description = "Lines added and removed as \"+38 −16\" in green and red mono, with an optional files count.",
	when = "The size of a change beside a file, commit, branch or PR: file lists, rows, title bars. For commits ahead and behind use `ahead-behind`; for a whole diff use `diff-view`.",
	props = {
		["added"] = "lines added",
		["removed"] = "lines removed",
		["files"] = "files changed: shows \"3 files\" first",
		["size"] = "small (default) | normal",
		["zero"] = "show +0 and −0 instead of hiding a zero side",
	},
	styles = {
		[".diff-counts"] = { gap = 6 },
		[".diff-counts-added"] = { color = "success", font = "mono" },
		[".diff-counts-removed"] = { color = "danger", font = "mono" },
		[".diff-counts-files"] = { color = "muted-foreground" },
	},
	example = { added = 38, removed = 16, files = 3, size = "normal" },

	build = function(props: Props)
		local added = props.added or 0
		local removed = props.removed or 0
		local size = (props.size or "small") :: any
		local files = props.files
		return ui.row {
			class = "diff-counts",
			shrink = false,
			align = "center",
			files and ui.text {
				if files == 1 then "1 file" else `{files} files`,
				class = "diff-counts-files",
				size = size,
			},
			(added > 0 or props.zero) and ui.text { `+{added}`, class = "diff-counts-added", size = size },
			(removed > 0 or props.zero) and ui.text { `−{removed}`, class = "diff-counts-removed", size = size },
		}
	end,
}
