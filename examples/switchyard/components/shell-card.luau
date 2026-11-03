-- One shell command from the `shell` provider, as a card: what ran, its
-- status and time, its output in colour, and actions under it (copy the
-- output, run it again, open a terminal there, add it to the chat).
--
-- The provider marks colours in each line as `\1FG;FLAGS\2`; see
-- crates/moonframe-shell/src/ansi.rs. Long output scrolls in a list, so
-- only the lines on screen are built.

local MARK = string.char(1)
local END = string.char(2)
-- Lines shown before the output scrolls, and each line's height.
local VISIBLE = 18
local LINE = 19

local NAMES = {
	"black", "red", "green", "yellow", "blue", "magenta", "cyan", "white",
	"bright-black", "bright-red", "bright-green", "bright-yellow",
	"bright-blue", "bright-magenta", "bright-cyan", "bright-white",
}

local STYLES: { [string]: any } = {
	[".shell-card"] = { background = "secondary", radius = 12, border = true, border_color = "border", gap = 0 },
	[".shell-head"] = { padding = { 8, 14 }, gap = 8, border = "bottom", border_color = "border" },
	[".shell-title"] = { color = "muted-foreground" },
	[".shell-status"] = { color = "muted-foreground", size = "small" },
	[".shell-body"] = { padding = { 10, 16, 12, 16 }, gap = 0 },
	[".shell-line"] = { font = "mono", height = LINE },
	[".shell-prompt"] = { font = "mono", color = "ansi-magenta", weight = "bold" },
	[".shell-command"] = { font = "mono", color = "ansi-green" },
	[".shell-text"] = { font = "mono" },
	[".shell-dim"] = { opacity = 0.65 },
	[".shell-hidden"] = { font = "mono", color = "muted-foreground", size = "small" },
	[".shell-error"] = { color = "danger" },
	[".shell-actions"] = { padding = { 6, 4, 2, 4 }, gap = 2 },
	[".shell-add"] = { color = "muted-foreground", padding = { 4, 8 }, radius = 8, gap = 4 },
	[".shell-add:hover"] = { background = "list-hover", color = "foreground" },
	[".shell-stop"] = { background = "card", radius = 7, padding = { 2, 9 }, border = true, border_color = "border" },
	[".shell-stop:hover"] = { background = "list-hover" },
}
for _, name in NAMES do
	STYLES[`.ansi-{name}`] = { color = `ansi-{name}` }
end

-- "0.6s", "12s", "2m 5s".
local function duration(ms: number): string
	local seconds = ms / 1000
	if seconds < 10 then
		return string.format("%.1fs", seconds)
	elseif seconds < 60 then
		return `{math.floor(seconds)}s`
	end
	return `{math.floor(seconds / 60)}m {math.floor(seconds % 60)}s`
end

-- A line's pieces: { style, text }, the style "FG;FLAGS" or "".
local function pieces(line: string): { { string } }
	local out = {}
	local at, style = 1, ""
	while at <= #line do
		local mark = string.find(line, MARK, at, true)
		if not mark then
			table.insert(out, { style, string.sub(line, at) })
			break
		end
		if mark > at then
			table.insert(out, { style, string.sub(line, at, mark - 1) })
		end
		local close = string.find(line, END, mark + 1, true)
		if not close then
			break
		end
		style = string.sub(line, mark + 1, close - 1)
		at = close + 1
	end
	return out
end

local function draw_line(line: string): any
	local texts = {}
	for _, piece in pieces(line) do
		local style, text = piece[1], piece[2]
		local fg, flags = string.match(style, "^([^;]*);?(.*)$")
		local classes = { "shell-text" }
		local inline = nil
		if fg and fg ~= "" then
			if string.sub(fg, 1, 1) == "#" then
				inline = { color = fg }
			else
				table.insert(classes, `ansi-{fg}`)
			end
		end
		if flags and string.find(flags, "d", 1, true) then
			table.insert(classes, "shell-dim")
		end
		local bold = flags and string.find(flags, "b", 1, true) ~= nil
		table.insert(texts, ui.text { text, class = classes, style = inline, weight = if bold then "bold" else nil, wrap = false })
	end
	if #texts == 0 then
		table.insert(texts, ui.text { " ", class = "shell-text", wrap = false })
	end
	return ui.row { class = "shell-line", gap = "none", align = "center", table.unpack(texts) }
end

local function status_row(run: any, stop: any): any
	local time = duration(run.elapsed_ms or 0)
	if run.status == "running" then
		return ui.row { align = "center", gap = "small",
			ui.icon { name = "loader", size = "small", spin = true, class = "shell-status" },
			ui.text { `Running {math.floor((run.elapsed_ms or 0) / 1000)}s`, class = "shell-status" },
			stop and ui.row { class = "shell-stop", msg = stop, tooltip = "Stop it (Esc in the composer)", ui.text { "Stop", size = "small" } },
		}
	end
	local tone, words = "success", "finished"
	if run.status == "stopped" then
		tone, words = "warning", "stopped"
	elseif run.status == "failed" then
		tone, words = "danger", if run.code then `exit {run.code}` else "failed"
	end
	return ui.row { align = "center", gap = "small",
		ui.text { "●", tone = tone :: any, size = "small" },
		ui.text { `{words} · {time}`, class = "shell-status" },
	}
end

return component {
	title = "Shell Card",
	category = "Data",
	icon = "square-terminal",
	description = "A shell command's run: the command, its status and time, its coloured output, and copy, rerun, terminal and add-to-chat actions.",
	when = "A command the user or an agent ran, from the `shell` provider (`app.shell.runs`), in a chat or a console. For a live shell the user types in use `terminal`; for a file's source use `code-view`.",
	props = {
		run = "the run, as `app.shell` shares it",
		stop = "message the Stop button sends while it runs",
		rerun = "message the rerun button sends",
		terminal = "message the terminal button sends (the command is copied too)",
		add = "message the Add to chat button sends; no button without it",
		added = "it is already in the chat's next prompt",
		title = "the header's text (default \"Shell command\")",
	},
	styles = STYLES,
	example = {
		run = {
			id = "1", command = "git log --oneline -3", status = "finished", elapsed_ms = 600,
			output = "\1yellow;\2" .. "2c89568 \1;\2Fix Linux builds\n\1yellow;\2" .. "8d790cb \1;\2Usage popover",
			text = "2c89568 Fix Linux builds\n8d790cb Usage popover", lines = 2, hidden = 0,
		},
		rerun = "rerun", terminal = "terminal", add = "add",
	},

	build = function(props)
		local run = props.run
		local lines: { string } = if run.output and run.output ~= "" then string.split(run.output, "\n") else {}
		local output: any = nil
		if #lines > VISIBLE then
			output = ui.list {
				id = `out-{run.id}`,
				count = #lines,
				row_height = LINE,
				height = VISIBLE * LINE,
				row = function(index: number)
					return draw_line(lines[index])
				end,
			}
		else
			local drawn = {}
			for _, line in lines do
				table.insert(drawn, draw_line(line))
			end
			if #drawn > 0 then
				output = ui.column { gap = "none", table.unpack(drawn) }
			end
		end
		return ui.column { gap = "none",
			ui.column { class = "shell-card",
				ui.row { class = "shell-head", align = "center",
					ui.icon { name = "square-terminal", size = "small", class = "shell-title" },
					ui.text { props.title or "Shell command", class = "shell-title" },
					ui.spacer(),
					status_row(run, props.stop),
				},
				ui.column { class = "shell-body", scroll = "x",
					ui.row { class = "shell-line", gap = "small", align = "center",
						ui.text { "❯", class = "shell-prompt" },
						ui.text { run.command, class = "shell-command", wrap = false },
					},
					(run.hidden or 0) > 0 and ui.text { `… {run.hidden} earlier lines`, class = "shell-hidden" },
					output,
					run.error and ui.text { run.error, class = "shell-error", wrap = true },
				},
			},
			run.status ~= "running" and ui.row { class = "shell-actions", align = "center",
				ui.button { icon = "copy", variant = "ghost", small = true, copy = run.text or "", tooltip = "Copy the output" },
				props.rerun and ui.button { icon = "rotate-cw", variant = "ghost", small = true, msg = props.rerun, tooltip = "Run it again" },
				props.terminal and ui.button { icon = "terminal", variant = "ghost", small = true, msg = props.terminal, copy = run.command,
					tooltip = "Open a terminal here, with the command copied to paste" },
				ui.spacer(),
				props.add and ui.row { class = "shell-add", align = "center", msg = props.add,
					tooltip = "Show the output to the agent with your next message",
					ui.icon { name = if props.added then "check" else "plus", size = "small" },
					ui.text { if props.added then "Added to chat" else "Add to chat", size = "small" },
				},
			},
		}
	end,
}
