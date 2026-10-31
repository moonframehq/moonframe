-- A live shell on libghostty-vt, Ghostty's own emulator. It keeps running across reloads and while hidden.
-- The host runs your login shell; a mod picks where it starts, never what
-- runs. Each `id` is one shell per mod.

type Props = {
	-- Names this shell within the mod; another id starts another shell.
	id: string?,
	-- The folder it starts in: a path, or `~/...`; the home folder if unset.
	cwd: string?,
	style: any,
}

return component {
	title = "Ghostty terminal",
	category = "Terminals",
	icon = "square-terminal",
	description = "A live shell on libghostty-vt, Ghostty's own emulator. It keeps running across reloads and while hidden.",
	example = { id = "shell", cwd = "~" },

	build = function(props: Props)
		return ui.terminal {
			id = props.id or "shell",
			cwd = props.cwd,
			engine = "ghostty",
			style = props.style,
		}
	end,
}
