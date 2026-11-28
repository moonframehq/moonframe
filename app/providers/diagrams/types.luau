-- What this provider publishes, as `app.diagrams` (`just provider-types`
-- copies it into types/moonframe.d.luau). published.json is an example the
-- check holds it to.

-- One diagram: an agent's (through `moonframe mcp diagrams`) or a file of
-- the project's `.moonframe/diagrams`.
export type Shown = {
	-- Unique in the list: `agent/<file name>` or `project/<file name>`.
	key: string,
	id: string,
	title: string,
	from: "agent" | "project",
	path: string,
	-- Grows with each change.
	revision: number,
	-- Unix seconds.
	updated: number,
	-- What `ui.diagram`'s `document` takes; nil when it doesn't parse
	-- (`error`) or past the publish's budget (`truncated`).
	document: any?,
	-- Node and edge ids the agent points at, and what it says of them.
	highlight: { string },
	note: string?,
	agent: string?,
	-- The text it came from, `mermaid` or `dot`.
	language: string?,
	text: string?,
	error: string?,
	truncated: boolean?,
}

export type Published = {
	available: boolean,
	-- The folder agents' diagrams are written to.
	agents: string,
	-- The project's `.moonframe/diagrams`, nil without a project.
	project: string?,
	-- Newest first.
	diagrams: { Shown },
}

-- `send "provider:diagrams" { close = key }` removes an agent's diagram: one
-- the MCP server wrote (`agent/<id>.json`), not another file of its folder.
export type Message = { close: string }
