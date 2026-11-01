-- A live shell on libghostty-vt, Ghostty's own emulator. It keeps running across reloads and while hidden.
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
	style: any,
}

return component {
	title = "Ghostty terminal",
	category = "Terminals",
	icon = "square-terminal",
	-- Offered by tab bars' + menu; a new tab starts in the home folder.
	tab = { title = "Ghostty terminal", icon = "square-terminal", props = { cwd = "~" } },
	description = "A live shell on libghostty-vt, Ghostty's own emulator. It keeps running across reloads and while hidden.",
	when = "A shell drawn by Ghostty's emulator; same as `terminal` with the other engine.",
	props = {
		["id"] = "keeps the shell across reloads; one per id",
		["cwd"] = "starting folder",
		["backend"] = "`auto` (default), `herdr-core`, `tmux` or `pty`",
		["style"] = "style table for the box",
	},
	example = { id = "shell", cwd = "~" },

	build = function(props: Props)
		return ui.terminal {
			id = props.id or "shell",
			cwd = props.cwd,
			backend = props.backend :: any,
			engine = "ghostty",
			style = props.style,
		}
	end,
}
