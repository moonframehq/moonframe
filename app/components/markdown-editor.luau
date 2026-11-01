-- A markdown file to read and edit: its source in a text area, the rendered
-- document beside it (or either alone), and Save. It reads and saves through
-- the `docs` provider, which only takes saves from the mods in its `writers`
-- (`editor`, as shipped). Saving never overwrites a file that changed on
-- disk since it was opened: the user picks Overwrite or Reload. A change on
-- disk with nothing edited simply reloads. Unsaved edits are kept across
-- launches. Links go to the drawing mod as { open_link = url, from = path }.

local segmented = use "segmented"
local markdown = use "markdown"
local notice = use "notice"
local loading = use "loading"
local empty_state = use "empty-state"
local error_state = use "error-state"

type Mode = "view" | "edit" | "split"

type State = {
	path: string?,
	-- The text being edited, and the version of the file it started from.
	text: string?,
	version: string?,
	-- Edited since it was read or saved.
	dirty: boolean,
	mode: Mode,
	-- Waiting for the file.
	loading: boolean,
	-- The text a save is writing, until it answers.
	saving: string?,
	-- A save was refused: the file changed on disk since it was read.
	conflict: boolean,
	-- It changed on disk while there are unsaved edits.
	outside: boolean,
	-- It was deleted or moved away.
	missing: boolean,
	error: string?,
	-- "Saved", until the next edit.
	note: string?,
}

type Props = {
	-- The file, relative to the project's root.
	path: string?,
	-- Without a path: a scratch note to start from, never saved.
	text: string?,
	-- The mode it opens in: "split" (default), "edit" or "view".
	mode: string?,
	-- The instance's id; a tab's own.
	id: string?,
}

local MODES = {
	{ id = "view", icon = "eye", tooltip = "Read" },
	{ id = "edit", icon = "pencil", tooltip = "Edit the source" },
	{ id = "split", icon = "columns-2", tooltip = "Source and preview side by side" },
}

local function read(path: string, id: string)
	return { to = "provider:docs", msg = { read = path, id = id } }
end

local function save(state: State, force: boolean?)
	state.saving = state.text
	state.note = nil
	state.error = nil
	return state,
		{
			{
				to = "provider:docs",
				msg = { write = state.path, text = state.text, version = state.version, force = force, id = "save" },
			},
		}
end

-- The file's name, after its folders.
local function name(path: string?): string
	local whole = path or "Scratch note"
	return string.match(whole, "([^/]+)$") or whole
end

return component {
	title = "Markdown Editor",
	category = "Content",
	icon = "book-open",
	description = "A markdown file's source and its rendered preview, side by side or alone, with Save; it never overwrites a file that changed on disk.",
	when = "Editing a markdown file of the project in a tab or a pane. For showing markdown without editing use `markdown`; for code read-only use `code-view`; for a short text field use `ui.input`.",
	props = {
		["path"] = "the file, relative to the project's root (as the explorer names it)",
		["text"] = "without `path`: a scratch note to start from, edited in place and never saved",
		["mode"] = "how it opens: \"split\" (default), \"edit\" or \"view\"",
		["id"] = "keeps the edits apart from another editor's; a tab passes its own",
	},
	tab = { title = "Markdown", icon = "book-open", props = {} },
	styles = {
		[".md-editor-bar"] = {
			padding = { 6, 10 },
			gap = 8,
			border = "bottom",
			border_color = "border",
		},
		[".md-editor-path"] = { font = "mono", size = "small" },
		[".md-editor-dirty"] = { color = "warning", size = "small" },
		[".md-editor-status"] = { size = "small", color = "muted-foreground" },
		[".md-editor-source"] = {
			border = "none",
			radius = 0,
			padding = { 12, 16 },
			background = "background",
		},
		[".md-editor-notice"] = { margin = { 8, 10, 0, 10 } },
	},
	example = {
		id = "example",
		mode = "split",
		text = "# Scratch note\n\nType on the left; the **preview** follows on the right.\n\n- [x] Source and preview\n- [ ] Save (a file needs a `path`)\n",
	},
	usage = [[local markdown_editor = use "markdown-editor"

-- In a pane that fills its space; Save needs the drawing mod in
-- providers/docs's `writers` (as shipped, `editor`).
markdown_editor { id = "readme", path = "README.md" }
-- update: msg.open_link is a clicked link's address, msg.from the file it is in]],

	init = function(previous: State?, props: Props?)
		local path = props and props.path
		local state: State
		if previous and previous.path == path then
			state = previous
		else
			local mode = (props and props.mode) or (previous and previous.mode) or "split"
			state = {
				path = path,
				dirty = false,
				mode = mode :: Mode,
				loading = false,
				conflict = false,
				outside = false,
				missing = false,
			}
		end
		-- Nothing is on its way at a launch.
		state.saving = nil
		state.note = nil
		state.error = nil
		state.loading = path ~= nil and state.text == nil
		if path then
			return state, { read(path, "load") }
		end
		-- A scratch note: what it was given, until it is edited.
		if state.text == nil then
			state.text = props and props.text
		end
		return state
	end,

	update = function(state: State, msg)
		if type(msg) ~= "table" then
			if msg == "save" and state.path and state.text and not state.saving then
				return save(state)
			elseif msg == "overwrite" and state.path and state.text and not state.saving then
				return save(state, true)
			elseif msg == "reload" and state.path then
				state.loading = state.text == nil
				return state, { read(state.path, "reload") }
			elseif msg == "keep" then
				-- Keep editing; a save will then ask before overwriting.
				state.outside = false
			elseif msg == "retry" and state.path then
				state.error = nil
				state.loading = true
				return state, { read(state.path, "load") }
			end
			return
		end

		if msg.msg == "edit" then
			state.text = msg.value
			state.dirty = true
			state.note = nil
		elseif msg.mode then
			state.mode = msg.mode
		elseif msg.link then
			return state, { { to = "parent", msg = { open_link = msg.url, from = state.path } } }
		elseif msg.changed and msg.changed == state.path then
			if msg.missing then
				state.missing = true
			elseif state.dirty then
				state.outside = true
			else
				return state, { read(state.path :: string, "load") }
			end
		elseif msg.reply == "load" or msg.reply == "reload" then
			state.loading = false
			if not msg.ok then
				state.error = msg.error
				return
			end
			state.missing = false
			if msg.reply == "reload" or not state.dirty or state.text == nil then
				state.text = msg.text
				state.version = msg.version
				state.dirty = false
				state.outside = false
				state.conflict = false
			elseif msg.version ~= state.version then
				-- Unsaved edits over a file that has moved on: the user decides.
				state.outside = true
			end
		elseif msg.reply == "save" then
			if msg.ok then
				state.version = msg.version
				state.dirty = state.text ~= state.saving
				state.conflict = false
				state.outside = false
				state.missing = false
				state.note = "Saved"
			elseif msg.conflict then
				state.conflict = true
			else
				state.error = msg.error
			end
			state.saving = nil
		end
	end,

	build = function(props: Props, kept: any)
		local state: State = kept
		if not state.path and state.text == nil then
			return ui.column {
				grow = true,
				align = "center",
				justify = "center",
				empty_state {
					icon = "book-open",
					title = "No file open",
					message = "Open a markdown file from the Files panel.",
				},
			}
		end

		local status
		if state.saving then
			status = ui.row {
				align = "center",
				gap = "small",
				ui.icon { "loader-circle", spin = true, class = "md-editor-status" },
				ui.text { "Saving…", class = "md-editor-status" },
			}
		elseif state.note then
			status = ui.text { state.note, class = "md-editor-status" }
		end

		local bar = ui.row {
			class = "md-editor-bar",
			align = "center",
			shrink = false,
			ui.icon { "book-open", tone = "muted" },
			ui.text { state.path or "Scratch note", class = "md-editor-path", truncate = true, tooltip = state.path },
			state.path and state.dirty and ui.text { "●", class = "md-editor-dirty", tooltip = "Unsaved changes" },
			ui.spacer(),
			status,
			segmented { options = MODES, selected = state.mode, event = "mode", small = true },
			state.path and ui.button {
				"Save",
				small = true,
				variant = if state.dirty then "primary" else "ghost",
				-- A deleted file is written again even unedited.
				disabled = (not state.dirty and not state.missing) or state.saving ~= nil or state.text == nil,
				msg = "save",
				tooltip = "Save (⌘S while editing)",
			},
		}

		local warning
		if state.conflict then
			warning = notice {
				tone = "warning",
				title = `{name(state.path)} changed on disk`,
				text = "It was changed elsewhere since you opened it. Overwrite it with your version, or reload it and lose your edits.",
				action = { label = "Overwrite", msg = "overwrite", variant = "outline" },
				ui.row { ui.button { "Reload from disk", small = true, variant = "ghost", msg = "reload" } },
			}
		elseif state.outside then
			warning = notice {
				tone = "warning",
				title = `{name(state.path)} changed on disk`,
				text = "You have unsaved edits. Reload to take the new version, or keep editing yours.",
				action = { label = "Reload", msg = "reload", variant = "outline" },
				dismiss = "keep",
			}
		elseif state.missing then
			warning = notice {
				tone = "warning",
				title = `{name(state.path)} is gone`,
				text = "It was deleted or moved. Save to write it again.",
			}
		elseif state.error and state.text then
			warning = notice { tone = "danger", title = "Couldn't save", text = state.error }
		end

		local body
		if state.loading then
			body = loading { block = true, text = `Opening {name(state.path)}…` }
		elseif state.text == nil then
			body = error_state {
				block = true,
				title = `Couldn't open {name(state.path)}`,
				message = state.error or "The file could not be read.",
				retry = "retry",
			}
		else
			local source = ui.input {
				id = "source",
				class = "md-editor-source",
				value = state.text,
				change = "edit",
				multiline = true,
				rows = 8,
				grow = true,
				mono = true,
				keys = { ["secondary-s"] = "save" },
			}
			local preview = markdown {
				text = state.text,
				link = { link = true },
				grow = true,
				scroll = true,
			}
			if state.mode == "view" then
				body = preview
			elseif state.mode == "edit" then
				body = source
			else
				body = ui.hsplit {
					id = "markdown-split",
					ui.pane { min = 200, gap = "none", source },
					ui.pane { min = 200, gap = "none", preview },
				}
			end
		end

		return ui.column {
			grow = true,
			gap = "none",
			bar,
			warning and ui.column { class = "md-editor-notice", shrink = false, warning },
			ui.column { grow = true, gap = "none", body },
		}
	end,
}
