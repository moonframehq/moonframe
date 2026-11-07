-- A live shell on alacritty_terminal, the pure-Rust emulator. It keeps running across reloads and while hidden.
-- The host runs your login shell; a mod picks where it starts, never what
-- runs. Each `id` is one shell per mod.

type Props = {
	-- Names this shell within the mod; another id starts another shell.
	id: string?,
	-- The folder it starts in: a path, or `~/...`; the home folder if unset.
	cwd: string?,
	-- What keeps it running: "auto" (herdr-core, else tmux, else a pty),
	-- "herdr-core", "tmux" or "pty".
	backend: string?,
	-- Sent once as the shell exits (Ctrl-D), with `exited = true` added.
	exit: any,
	-- Sent as the shell reports something, with `event` and its fields
	-- added: see `shell` on `ui.terminal`.
	shell: any,
	style: any,
}

return component {
	title = "Terminal",
	category = "Terminals",
	icon = "square-terminal",
	-- Offered by tab bars' + menu; a new tab starts in the home folder.
	-- The tab passes `exit`, so Ctrl-D in the shell closes it.
	tab = { title = "Terminal", icon = "terminal", props = {}, report = "exit" },
	description = "A live shell on alacritty_terminal, the pure-Rust emulator. It keeps running across reloads and while hidden.",
	when = "A live shell (alacritty engine) filling its space: a pane, a bottom panel.",
	props = {
		["id"] = "keeps the shell across reloads; one per id",
		["cwd"] = "starting folder",
		["backend"] = "`auto` (default), `herdr-core`, `tmux` or `pty`",
		["exit"] = "message sent once as the shell exits, with `exited = true` added; without it Enter starts another shell",
		["shell"] = "message sent as the shell reports something, with `event` added: `prompt`, `started`, `finished`, `folder`, `title` or `notice`",
		["style"] = "style table for the box",
	},
	example = { id = "shell", cwd = "~" },

	build = function(props: Props)
		return ui.terminal {
			id = props.id or "shell",
			cwd = props.cwd,
			backend = props.backend :: any,
			exit = props.exit,
			shell = props.shell,
			engine = "alacritty",
			style = props.style,
		}
	end,
}
