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

export type Published = {
	available: boolean,
	error: string?,
	threads: { Thread },
	total: number,
}

export type Message = "refresh"
