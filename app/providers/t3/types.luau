export type Link = { label: string, title: string, url: string }

export type Thread = {
	id: string,
	title: string,
	project: string,
	projectId: string,
	provider: string?,
	status: string,
	group: string,
	updatedAt: string,
	branch: string?,
	worktree: string?,
	pullRequests: { Link },
	issues: { Link },
}

export type Result = { status: string, error: string?, threadId: string? }
export type Project = { id: string, label: string }

export type Published = {
	available: boolean,
	environmentId: string?,
	projects: { Project },
	writer: string?,
	ticket: string?,
	results: { [string]: Result },
	error: string?,
	threads: { Thread },
	total: number,
}

export type Write = {
	action: "start" | "send",
	target: string,
	text: string,
	environmentId: string,
	ticket: string,
	id: string,
}

export type Message = "refresh" | Write
