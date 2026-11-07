-- Settings. Providers write pi's own auth.json and models.json, so the CLI and
-- this app share their sign-ins. A browser sign-in is pi's own `/login`, which
-- runs in a terminal; the app sees the new credential by itself.

local dialog = use "dialog"
local dropdown = use "dropdown"
local kbd = use "kbd"
local segmented = use "segmented"
local settings_row = use "settings-row"
local switch = use "switch"

type State = {
	page: string,
	key_for: { id: string, name: string, env: string? }?,
	key: string,
	endpoint: { id: string, url: string, key: string, models: string }?,
	login: string?,
	theme_open: boolean,
	problem: string?,
}

local PAGES = {
	{ id = "providers", label = "Providers", icon = "key-round" },
	{ id = "general", label = "General", icon = "settings" },
	{ id = "appearance", label = "Appearance", icon = "palette" },
	{ id = "shortcuts", label = "Shortcuts", icon = "keyboard" },
	{ id = "about", label = "About", icon = "info" },
}

local SHORTCUTS = {
	{ "cmd-k", "Search chats, workspaces and actions" },
	{ "cmd-p", "Open any file in the checkout" },
	{ "cmd-n", "New thread" },
	{ "ctrl-tab", "Switch between recent threads" },
	{ "cmd-1", "Jump to a thread in the sidebar (1 to 9)" },
	{ "cmd-j", "Toggle the terminal" },
	{ "cmd-r", "Toggle changes" },
	{ "cmd-e", "Toggle files" },
	{ "cmd-alt-b", "Show or hide the workbench" },
	{ "cmd-b", "Show or hide the sidebar" },
	{ "cmd-f", "Find in thread" },
	{ "cmd-shift-m", "Switch model" },
	{ "cmd-.", "Stop the run" },
	{ "enter", "Send, or queue a follow-up during a run" },
	{ "cmd-enter", "Steer the current run" },
	{ "shift-enter", "A new line" },
}

local function heading(title: string, about: string?): any
	return ui.column { gap = "none", ui.text { title, class = "page-title" }, about and ui.text { about, tone = "muted", wrap = true } }
end

local function provider_row(item: any, last: boolean): any
	return settings_row {
		title = item.name,
		border = not last,
		description = if item.connected then item.via elseif item.env then item.env else "Sign in with your account",
		ui.row {
			align = "center",
			gap = "small",
			item.connected and ui.row { class = "settings-on" },
			item.oauth and not item.connected and ui.button { "Sign in", small = true, msg = { login = item.id } },
			not item.keyless and not item.connected and ui.button { "Add key", small = true, msg = { add_key = item.id, name = item.name, env = item.env } },
			item.removable and ui.button { "Disconnect", small = true, variant = "ghost", msg = { forget_key = item.id }, to = "provider:pi" },
		},
	}
end

local function providers_page(pi: any): { any }
	local rows: { any } = { heading("Providers", "Where pi gets its models. Keys stay in pi's auth.json, shared with the pi CLI.") }
	local known: { any } = {}
	for index, item in pi.providers.known do
		table.insert(known, provider_row(item, index == #pi.providers.known))
	end
	table.insert(rows, ui.column { class = "page-card", gap = "none", table.unpack(known) })
	local custom: { any } = {}
	for _, endpoint in pi.providers.endpoints do
		table.insert(custom, settings_row {
			title = endpoint.id,
			description = `{endpoint.url} · {endpoint.models}`,
			ui.button { "Remove", small = true, variant = "ghost", msg = { remove_endpoint = endpoint.id }, to = "provider:pi" },
		})
	end
	table.insert(rows, ui.column {
		class = "page-card",
		gap = "none",
		ui.row { align = "center", ui.text { "Custom endpoints", weight = "medium" }, ui.spacer(), ui.button { "Add endpoint", small = true, msg = "add-endpoint" } },
		#custom == 0 and ui.text { "Any OpenAI-compatible server: Ollama, LM Studio, vLLM.", tone = "muted", size = "small" },
		table.unpack(custom),
	})
	if pi.config_error then
		table.insert(rows, ui.text { pi.config_error, tone = "danger" })
	end
	return rows
end

local function general_page(pi: any): { any }
	local prefs = pi.prefs
	return {
		heading("General"),
		ui.column {
			class = "page-card",
			gap = "none",
			settings_row {
				title = "Notify when a thread needs you",
				description = "When a background thread finishes, fails or asks something.",
				switch { on = prefs.notifications, msg = { set_pref = { notifications = not prefs.notifications } }, to = "provider:pi" },
			},
			settings_row {
				title = "Play a sound",
				switch { on = prefs.sounds, msg = { set_pref = { sounds = not prefs.sounds } }, to = "provider:pi" },
			},
			settings_row {
				title = "Name threads from their first prompt",
				description = "Asks the thread's model for a short title, which costs a few tokens.",
				border = false,
				switch { on = prefs.auto_title, msg = { set_pref = { auto_title = not prefs.auto_title } }, to = "provider:pi" },
			},
		},
	}
end

local function appearance_page(state: State, app: any): { any }
	local appearance = app.appearance
	local options: { any } = {}
	for _, theme in appearance.themes do
		if theme.dark == appearance.dark then
			table.insert(options, { id = theme.name, label = theme.name, dot = theme.accent })
		end
	end
	return {
		heading("Appearance"),
		ui.column {
			class = "page-card",
			gap = "none",
			settings_row {
				title = "Mode",
				segmented {
					selected = appearance.mode,
					event = "appearance",
					options = { { id = "system", label = "System" }, { id = "light", label = "Light" }, { id = "dark", label = "Dark" } },
				},
			},
			settings_row {
				title = "Theme",
				description = "One for light windows and one for dark.",
				border = false,
				dropdown { value = appearance.theme, open = state.theme_open, toggle = "toggle-theme", dismiss = "close-theme", options = options, event = if appearance.dark then "dark_theme" else "light_theme", width = 260, align = "end" },
			},
		},
	}
end

local function shortcuts_page(): { any }
	local rows: { any } = {}
	for index, shortcut in SHORTCUTS do
		table.insert(rows, settings_row { title = shortcut[2], border = index < #SHORTCUTS, kbd { keys = shortcut[1] } })
	end
	return { heading("Shortcuts"), ui.column { class = "page-card", gap = "none", table.unpack(rows) } }
end

local function about_page(pi: any): { any }
	local runtime = pi.runtime
	return {
		heading("About"),
		ui.column {
			class = "page-card",
			gap = "none",
			settings_row { title = "pi", description = runtime.path or "Not found", ui.text { runtime.version or "", tone = "muted" } },
			settings_row {
				title = "Sessions",
				description = "Threads are pi's own session files, so they carry over to the CLI.",
				border = false,
				ui.button { "pi on GitHub", small = true, variant = "ghost", to = "app:open", msg = "https://github.com/earendil-works/pi" },
			},
		},
	}
end

return mod {
	styles = {
		[".settings-side"] = { padding = { 0, 10 }, gap = 2, border = "right" },
		[".settings-link"] = { padding = { 7, 10 }, radius = 8, gap = 10 },
		[".settings-link:hover"] = { background = "list-hover" },
		[".settings-link:selected"] = { background = "list-active" },
		[".settings-on"] = { width = 8, height = 8, radius = 4, background = "success" },
		[".login-command"] = { font = "mono", background = "muted", radius = 6, padding = { 2, 6 } },
	},

	init = function(previous: State?): State
		local state: State = previous or { page = "providers", key = "", theme_open = false }
		state.key_for = nil
		state.endpoint = nil
		state.login = nil
		state.key = ""
		state.theme_open = false
		return state
	end,

	update = function(state: State, msg: any)
		if msg == "cancel" then
			state.key_for = nil
			state.endpoint = nil
			state.login = nil
			state.key = ""
			state.problem = nil
		elseif msg == "save-key" then
			local target = state.key_for
			local key = string.match(state.key, "^%s*(.-)%s*$") or ""
			state.key_for = nil
			state.key = ""
			if target and key ~= "" then
				return state, { { to = "provider:pi", msg = { save_key = { provider = target.id, key = key } } } }
			end
		elseif msg == "add-endpoint" then
			state.endpoint = { id = "", url = "", key = "", models = "" }
			state.problem = nil
		elseif msg == "save-endpoint" then
			local endpoint = state.endpoint
			if endpoint then
				local models: { string } = {}
				for model in string.gmatch(endpoint.models, "[^,%s]+") do
					table.insert(models, model)
				end
				if not string.match(endpoint.id, "^[%w%-_]+$") or not string.match(endpoint.url, "^https?://") or #models == 0 then
					state.problem = "It needs a one-word name, an http(s) address and at least one model."
					return
				end
				state.endpoint = nil
				return state, { { to = "provider:pi", msg = { save_endpoint = { id = endpoint.id, url = endpoint.url, key = endpoint.key, models = models } } } }
			end
		elseif msg == "open-terminal" then
			state.login = nil
			local sends: { any } = { { to = "shell", msg = { page = "threads" } }, { to = "workbench", msg = { tab = "terminal" } }, { to = "shell", msg = "show-workbench" } }
			return state, sends
		elseif msg == "toggle-theme" then
			state.theme_open = not state.theme_open
		elseif msg == "close-theme" then
			state.theme_open = false
		elseif type(msg) ~= "table" then
			return
		elseif msg.go then
			state.page = msg.go
		elseif msg.add_key then
			state.key_for = { id = msg.add_key, name = msg.name, env = msg.env }
			state.key = ""
		elseif msg.login then
			state.login = msg.login
		elseif msg.msg == "key" then
			state.key = msg.value
		elseif msg.msg == "endpoint" and state.endpoint then
			(state.endpoint :: any)[msg.field] = msg.value
		elseif msg.appearance then
			return state, { { to = "app:appearance", msg = msg.appearance } }
		elseif msg.dark_theme or msg.light_theme then
			state.theme_open = false
			return state, { { to = "app:theme", msg = if msg.dark_theme then { dark = msg.dark_theme } else { light = msg.light_theme } } }
		end
	end,

	view = function(state: State, app: any, props: any)
		if props == nil then
			return ui.column { width = 0, height = 0 }
		end
		local pi = app.pi
		local nav: { any } = {}
		for _, page in PAGES do
			table.insert(nav, ui.row {
				class = "settings-link",
				selected = state.page == page.id,
				align = "center",
				msg = { go = page.id },
				ui.icon { page.icon :: any },
				ui.text { page.label },
			})
		end
		local body: { any } = {}
		if pi and pi.available ~= false then
			body = if state.page == "providers" then providers_page(pi)
				elseif state.page == "general" then general_page(pi)
				elseif state.page == "appearance" then appearance_page(state, app)
				elseif state.page == "shortcuts" then shortcuts_page()
				else about_page(pi)
		end
		local endpoint = state.endpoint
		local function endpoint_field(field: string, label: string, focus: boolean?): any
			return ui.row {
				class = "page-field",
				ui.input { id = `endpoint-{field}`, grow = true, focus = focus, value = (endpoint :: any)[field], placeholder = label, change = { msg = "endpoint", field = field } },
			}
		end
		return ui.column {
			grow = true,
			gap = "none",
			ui.row { drag = true, height = 52, shrink = false },
			ui.row {
				grow = true,
				gap = "none",
				align = "stretch",
				ui.column { class = "settings-side", width = 200, shrink = false, table.unpack(nav) },
				ui.column { grow = true, scroll = true, gap = "none", ui.column { class = "page", shrink = false, table.unpack(body) } },
			},
			dialog {
				open = state.key_for ~= nil,
				dismiss = "cancel",
				title = if state.key_for then `Add a key for {state.key_for.name}` else "",
				subtitle = "Saved in pi's auth.json, readable only by you.",
				size = "medium",
				submit = "save-key",
				buttons = { { label = "Cancel", msg = "cancel", variant = "ghost" }, { label = "Save", msg = "save-key", variant = "primary" } },
				ui.row { class = "page-field", ui.input { id = "api-key", grow = true, focus = true, mono = true, value = state.key, placeholder = "API key", change = "key", submit = "save-key" } },
			},
			dialog {
				open = state.login ~= nil,
				dismiss = "cancel",
				title = "Sign in with pi",
				size = "medium",
				buttons = { { label = "Cancel", msg = "cancel", variant = "ghost" }, { label = "Open a terminal", msg = "open-terminal", variant = "primary" } },
				ui.column {
					gap = "small",
					ui.text { "pi runs this sign-in itself. In a terminal, start pi and type /login, then pick the provider. Your browser opens; come back when it's done.", wrap = true },
					ui.text { "Pilot picks up the new sign-in by itself.", tone = "muted" },
				},
			},
			dialog {
				open = endpoint ~= nil,
				dismiss = "cancel",
				title = "Add a custom endpoint",
				subtitle = "Any server that speaks OpenAI's chat completions.",
				size = "medium",
				note = state.problem,
				submit = "save-endpoint",
				buttons = { { label = "Cancel", msg = "cancel", variant = "ghost" }, { label = "Save", msg = "save-endpoint", variant = "primary" } },
				endpoint and ui.column {
					gap = "normal",
					endpoint_field("id", "Name, such as ollama", true),
					endpoint_field("url", "Address, such as http://localhost:11434/v1"),
					endpoint_field("key", "API key, if it needs one"),
					endpoint_field("models", "Model ids, separated by commas"),
				},
			},
		}
	end,
}
