-- A few avatars overlapping, then how many there are: who is in a group
-- (investors, reviewers, people on a thread) without listing them.

local avatar = use "avatar"

type Props = {
	people: { any },
	count: number?,
	most: number?,
	size: string?,
}

return component {
	title = "Avatar Stack",
	category = "Data",
	icon = "user",
	description = "Up to a few overlapping round avatars, then the group's size.",
	when = "Who is in a group, in a card or a row (38 investors, 3 reviewers), when their names matter less than how many. For one person use `avatar`; for a list of people use `list` or `table`.",
	props = {
		["people"] = "list of { name, src? }: the avatars drawn",
		["count"] = "the group's size, shown after them (default the number of people)",
		["most"] = "the most avatars drawn (default 3)",
		["size"] = "avatar size: small (default) or normal",
	},
	styles = {
		[".avatar-stack"] = { gap = 6 },
		[".avatar-stack-faces"] = { gap = 0 },
		[".avatar-stack-face"] = { radius = 12, border = true, border_color = "background" },
		[".avatar-stack-count"] = { size = "small", color = "muted-foreground" },
	},
	example = {
		people = { { name = "Cole Bennett" }, { name = "Imani Bennett" }, { name = "Ray Ortiz" }, { name = "Mia Chen" } },
		count = 38,
	},

	build = function(props: Props)
		local side = if props.size == "normal" then 24 else 18
		local step = side - 6
		local faces = {}
		for index, person in props.people do
			if index > (props.most or 3) then
				break
			end
			-- Each over the one before, floated along the row.
			table.insert(faces, ui.row {
				class = "avatar-stack-face",
				float = { top = 0, left = (index - 1) * step },
				avatar { name = person.name, src = person.src, shape = "circle", size = props.size or "small" },
			})
		end
		return ui.row {
			class = "avatar-stack",
			align = "center",
			hug = true,
			ui.row {
				class = "avatar-stack-faces",
				width = side + (#faces - 1) * step + 2,
				height = side + 2,
				shrink = false,
				table.unpack(faces),
			},
			ui.text { tostring(props.count or #props.people), class = "avatar-stack-count" },
		}
	end,
}
