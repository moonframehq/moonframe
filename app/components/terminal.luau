-- A live shell on alacritty_terminal, the pure-Rust emulator. It keeps running across reloads and while hidden.
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
	title = "Terminal",
	category = "Terminals",
	icon = "square-terminal",
	-- Offered by tab bars' + menu; a new tab starts in the home folder.
	tab = { title = "Terminal", icon = "terminal", props = { cwd = "~" } },
	description = "A live shell on alacritty_terminal, the pure-Rust emulator. It keeps running across reloads and while hidden.",
	when = "A live shell (alacritty engine) filling its space: a pane, a bottom panel.",
	props = {
		["id"] = "keeps the shell across reloads; one per id",
		["cwd"] = "starting folder",
		["style"] = "style table for the box",
	},
	example = { id = "shell", cwd = "~" },

	build = function(props: Props)
		return ui.terminal {
			id = props.id or "shell",
			cwd = props.cwd,
			engine = "alacritty",
			style = props.style,
		}
	end,
}
