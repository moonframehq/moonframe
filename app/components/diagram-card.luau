-- A diagram an agent shows (from `app.diagrams`), as a card in its chat:
-- the title, who shows it and what it points at, over the diagram drawn
-- by `ui.diagram`. As the agent patches it, the same card follows; it
-- fits itself again with each revision, and scrolls the chat past it
-- until a click inside lets it pan and zoom.

type Props = {
	-- One of `app.diagrams.diagrams`.
	diagram: any,
	height: number?,
}

-- The id its hosted view keeps it by, from the diagram's key: letters,
-- digits, `-` and `_`, at most 64 (the host's limit). A key too long for
-- that keeps its start and ends in a hash of the whole key, so each card
-- keeps an id of its own.
local function view_id(key: string): string
	local id = "card-" .. string.gsub(key, "[^%w_-]", "-")
	if #id <= 64 then
		return id
	end
	local hash = 5381
	for i = 1, #key do
		hash = bit32.band(hash * 33 + string.byte(key, i), 0xffffffff)
	end
	return string.sub(id, 1, 55) .. string.format("-%08x", hash)
end

return component {
	title = "Diagram Card",
	category = "Agents",
	icon = "workflow",
	description = "A diagram an agent shows through `moonframe mcp diagrams`, drawn in its chat and kept up to date as it patches it.",
	when = "In a chat or an agent run, for the `app.diagrams` entry its `diagram_*` tool calls showed (`tool-group` and `agent-run` draw it from their `diagrams` prop). For a diagram the mod owns use `ui.diagram`; for Mermaid in an answer a ```mermaid fence draws by itself.",
	props = {
		diagram = "one of `app.diagrams.diagrams`: { key, title, agent?, note?, document?, highlight, revision, error? }",
		height = "the diagram's height in pixels (default 300)",
	},
	styles = {
		[".diagram-card"] = { border = true, border_color = "border", radius = 10, gap = 0, clip = true },
		[".diagram-card-head"] = { padding = { 6, 10 }, gap = 8, border = "bottom", border_color = "border" },
		[".diagram-card-title"] = { size = "small", weight = "medium" },
		[".diagram-card-detail"] = { size = "small", color = "muted-foreground" },
		[".diagram-card-note"] = { size = "small", color = "accent" },
		[".diagram-card-problem"] = { padding = { 8, 10 }, size = "small", color = "muted-foreground" },
	},
	example = {
		diagram = {
			key = "agent/example",
			title = "What I am changing",
			agent = "claude",
			note = "Working on the parser",
			revision = 1,
			highlight = { "parse" },
			document = {
				direction = "right",
				nodes = { { id = "md", label = "Markdown" }, { id = "parse", label = "Parser" }, { id = "view", label = "View" } },
				edges = { { from = "md", to = "parse" }, { from = "parse", to = "view" } },
			},
		},
	},

	build = function(props: Props)
		local d = props.diagram
		if type(d) ~= "table" then
			return ui.column {}
		end
		local body = if d.error
			then ui.text { `Not drawn: {d.error}`, class = "diagram-card-problem" }
			elseif not d.document then ui.text { "Too big to share here.", class = "diagram-card-problem" }
			else ui.column {
				height = props.height or 300,
				gap = "none",
				ui.diagram {
					id = view_id(d.key or "diagram"),
					document = d.document,
					highlight = d.highlight,
					scroll = "focus",
					camera = { fit = true, seq = d.revision },
				},
			}
		return ui.column {
			class = "diagram-card",
			ui.row {
				class = "diagram-card-head",
				align = "center",
				ui.icon { name = "workflow", size = "small", tone = "accent" },
				ui.text { d.title or "Diagram", class = "diagram-card-title", truncate = true },
				d.agent and ui.text { d.agent, class = "diagram-card-detail" },
				ui.spacer(),
				d.note and ui.text { d.note, class = "diagram-card-note", truncate = true },
			},
			body,
		}
	end,
}
