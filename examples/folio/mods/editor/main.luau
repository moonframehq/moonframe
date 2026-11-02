-- The page: the open document in a quiet column. Focus mode (⌘D) keeps the
-- sentence, then the paragraph, being written in full ink; typewriter mode
-- (⌘T) keeps its line in the middle; ⌘R reads it as it will look. The
-- footer shows one figure of its length, a click steps to the next.
--
-- The text is the provider's copy as it was opened; each edit goes back to
-- it to be written once typing stops. The field is keyed by the opening,
-- so another document (or the same one opened again) starts a field of its
-- own, and an edit never comes back to the field late.

local prose_editor = use "prose-editor"
local markdown = use "markdown"
local writing_stats = use "writing-stats"
local icon_button = use "icon-button"
local empty_state = use "empty-state"
local kbd = use "kbd"

type State = {
	dim: ("sentence" | "paragraph")?,
	typewriter: boolean,
	preview: boolean,
	mono: boolean,
	show: string,
	lead: boolean,
	-- What was typed in the opening `opened`, newer than the provider's copy.
	text: string?,
	opened: string?,
}

local DIM_NEXT = { sentence = "paragraph", paragraph = "off", off = "sentence" }
local DIM_LABEL = { sentence = "Sentence", paragraph = "Paragraph" }

-- The keys the field takes while it has focus, which would otherwise go to
-- the editor engine before the menus.
local FIELD_KEYS = {
	["secondary-d"] = "cycle-dim",
	["secondary-t"] = "typewriter",
	["secondary-r"] = "preview",
	["secondary-shift-f"] = "font",
	["escape"] = "cycle-dim-off",
}

-- A field's message comes as { msg = name, value = text }; a menu's as the name.
local function name(msg): any
	if type(msg) == "table" and msg.msg ~= nil then
		return msg.msg
	end
	return msg
end

return mod {
	region = "main",
	title = "Page",

	styles = {
		[".page-head"] = { height = 52, padding = { 0, 14, 0, 14 }, gap = 4 },
		[".page-head-lead"] = { padding = { 0, 14, 0, 84 } },
		[".page-title"] = { size = "small", color = "muted-foreground" },
		-- Room on the right for the feedback button in the window's corner.
		[".page-foot"] = { height = 40, padding = { 0, 64, 0, 16 }, gap = 10 },
		[".page-mode"] = { size = "small", color = "accent" },
		[".page-preview"] = { padding = { 28, 32, 40, 32 } },
		[".page-chrome"] = { opacity = 0.55, transition = "fade normal" },
		[".page-chrome:hover"] = { opacity = 1 },
	},

	init = function(previous: State?): State
		return previous or { typewriter = false, preview = false, mono = true, show = "words", lead = false }
	end,

	update = function(state: State, msg)
		local what = name(msg)
		if type(msg) == "table" and msg.typed then
			state.text = msg.value
			state.opened = msg.opened
			return state, { { to = "provider:library", msg = { save = { id = msg.typed, text = msg.value } } } }
		elseif type(msg) == "table" and msg.new then
			return state, { { to = "provider:library", msg = { new = true } } }
		elseif what == "cycle-dim" then
			local next = DIM_NEXT[state.dim or "off"]
			state.dim = if next == "off" then nil else next
		elseif what == "cycle-dim-off" then
			state.dim = nil
		elseif what == "typewriter" then
			state.typewriter = not state.typewriter
		elseif what == "preview" then
			state.preview = not state.preview
		elseif what == "font" then
			state.mono = not state.mono
		elseif type(msg) == "table" and msg.lead ~= nil then
			state.lead = msg.lead
		elseif type(msg) == "table" and msg.show then
			state.show = msg.show
		end
		return state
	end,

	view = function(state: State, app)
		local current = app.library and app.library.current
		if not current then
			return ui.column {
				grow = true,
				ui.row { class = "page-head", drag = true, shrink = false },
				ui.column {
					grow = true,
					justify = "center",
					empty_state {
						icon = "square-pen",
						title = "No document open",
						message = "Pick one from the list, or start a new one.",
						action = "New Document",
						msg = { new = true },
					},
				},
			}
		end
		if current.too_large then
			return ui.column {
				grow = true,
				ui.row { class = "page-head", drag = true, shrink = false },
				ui.column {
					grow = true,
					justify = "center",
					empty_state {
						icon = "file-text",
						title = `{current.id} is too large to edit here`,
						message = "Folio opens documents up to 256 KB. Open the file in another editor, or split it.",
					},
				},
			}
		end
		local text = if state.opened == current.opened and state.text then state.text else current.text
		local title = string.match(text, "^%s*#*%s*([^\n]+)") or current.id
		local page = if state.preview
			then ui.column {
				class = "page-preview",
				grow = true,
				scroll = true,
				ui.row { justify = "center", shrink = false, markdown { text = text, max_width = 680 } },
			}
			else prose_editor {
				id = `doc-{current.opened}`,
				value = current.text,
				change = { typed = current.id, opened = current.opened },
				keys = FIELD_KEYS,
				dim = state.dim,
				typewriter = state.typewriter,
				mono = state.mono,
				focus = true,
			}
		local modes: { string } = {}
		if state.dim then
			table.insert(modes, `Focus: {DIM_LABEL[state.dim]}`)
		end
		if state.typewriter then
			table.insert(modes, "Typewriter")
		end
		if state.preview then
			table.insert(modes, "Preview")
		end
		return ui.column {
			grow = true,
			gap = "none",
			ui.row {
				class = if state.lead then "page-head page-head-lead" else "page-head",
				align = "center",
				drag = true,
				shrink = false,
				ui.row {
					class = "page-chrome",
					gap = "small",
					icon_button { icon = "panel-left", tooltip = "Show or hide the library and documents", msg = "focus-page", to = "shell" },
				},
				ui.spacer(),
				ui.text { title, class = "page-title", truncate = true },
				ui.spacer(),
				ui.row {
					class = "page-chrome",
					gap = "small",
					icon_button { icon = "scan", tooltip = "Focus mode (⌘D)", msg = "cycle-dim", active = state.dim ~= nil },
					icon_button { icon = "keyboard", tooltip = "Typewriter mode (⌘T)", msg = "typewriter", active = state.typewriter },
					icon_button { icon = "case-sensitive", tooltip = "Switch font (⇧⌘F)", msg = "font", active = not state.mono },
					icon_button { icon = if state.preview then "pencil" else "eye", tooltip = if state.preview then "Write (⌘R)" else "Preview (⌘R)", msg = "preview", active = state.preview },
				},
			},
			page,
			ui.row {
				class = "page-foot",
				align = "center",
				shrink = false,
				ui.text { table.concat(modes, " · "), class = "page-mode" },
				ui.spacer(),
				#modes == 0 and ui.row { class = "page-chrome", kbd { keys = "cmd-d", small = true, plain = true } },
				writing_stats { text = text, show = state.show :: any, msg = { show = true } },
			},
		}
	end,
}
