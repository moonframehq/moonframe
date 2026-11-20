-- What this provider publishes, as `app.agents` (`just provider-types`
-- copies it into types/moonframe.d.luau).

export type Agent = {
	pid: number,
	ppid: number,
	-- "claude", "codex", "gemini", "cursor", "opencode", "aider", "amp",
	-- "copilot", "droid", "kiro", "qwen", "goose", "crush", ...
	name: string,
	-- Its command line, cut to 200 characters.
	command: string,
	cwd: string?,
	tty: string?,
	-- Unix seconds.
	started: number,
	elapsed_seconds: number,
	-- Percent of one core.
	cpu: number,
	-- Always "running": what it is doing is not visible from outside.
	status: string,
}

export type Published = {
	available: boolean,
	reason: string?,
	agents: { Agent },
	count: number,
}
