-- A web page with an address bar: back, forward, reload (stop while the page
-- loads), the address, the title, with `share` a button that offers the page
-- to the system's share menu (`app:share`), and one that opens it in the
-- system browser. The page is a `ui.browser`; the bar drives it with the
-- host's browser commands, and its address field is the page's own
-- (`ui.input { browser = id }`): Enter goes where it says, a host over
-- https or words as a search, and Escape puts back the page's address.

-- What the page last reported: the mod's `navigate` message, kept as is.
type Page = {
	url: string?,
	title: string?,
	loading: boolean?,
	back: boolean?,
	forward: boolean?,
}

type Props = {
	-- Names the page within the mod; one page per id.
	id: string?,
	-- Where to go: http, https or file. A new value navigates.
	url: string?,
	-- The last `navigate` message the mod got, to show where the page is.
	page: Page?,
	-- Sent as the page navigates, with `url`, `title`, `loading`, `back`
	-- and `forward` added; keep it in state and pass it back as `page`.
	navigate: any,
	loading: any,
	zoom: number?,
	-- A Share button for a web page: its address and title to the
	-- system's share menu (macOS; the app needs the `share` feature).
	share: boolean?,
}

return component {
	title = "Browser",
	category = "Content",
	icon = "globe",
	-- Offered by tab bars' + menu. A tab passes `navigate` and keeps what
	-- it reports as `page`, so the tab shows the page it is on.
	tab = { title = "Browser", icon = "globe", props = { url = "https://luau.org" }, report = "navigate" },
	description = "A web page with an address bar: back, forward, reload or stop, an editable address, the title, optionally share, and open in the system browser.",
	when = "A web page in a pane, with browser chrome. Keep the page's `navigate` messages in state and pass the latest as `page`; the address bar navigates on its own. For a bare page without a bar use `ui.browser`.",
	props = {
		["id"] = "keeps the page across renders; one per id",
		["url"] = "address to load (http, https, file); a new value navigates",
		["page"] = "the last `navigate` message: `url`, `title`, `loading`, `back`, `forward`",
		["navigate"] = "message sent as the page navigates, loads or its title changes; keep it and pass it as `page`",
		["loading"] = "message sent as loads start and finish, with `loading` and `url` added",
		["zoom"] = "page zoom, 0.25 to 5",
		["share"] = "true: a Share button that offers a web page's address and title to the system's share menu (`app:share`; macOS, and the app built with `share`)",
	},
	styles = {
		[".browser"] = { gap = 0, radius = 8, border = true },
		[".browser-bar"] = { padding = { 4, 6 }, gap = 4, border = "bottom", background = "secondary" },
		[".browser-address"] = { padding = { 2, 8 }, size = "small" },
		[".browser-security"] = { color = "muted-foreground", size = "small" },
		[".browser-title"] = { color = "muted-foreground", size = "small" },
	},
	example = {
		id = "example",
		url = "https://example.com",
		page = { url = "https://example.com/", title = "Example Domain", back = true, forward = false },
		share = true,
	},
	usage = [[
local browser = use "browser"

-- in view: the address bar, history and loading follow `page` by themselves
browser { id = "docs", url = state.url, page = state.page, navigate = { navigated = true } }
-- in update: msg.navigated sets state.page = msg (keep the whole message)
-- to send the page somewhere from code, set state.url to a new address]],

	build = function(props: Props)
		local id = props.id or "browser"
		local page: Page = props.page or {}
		local address = page.url or props.url or ""
		local busy = page.loading == true
		-- A web page can go to the share menu, when the caller offers it.
		local sharing = props.share == true
			and (string.sub(address, 1, 8) == "https://" or string.sub(address, 1, 7) == "http://")
		local function chrome(label: string, icon: string, command: string, disabled: boolean?)
			return ui.button {
				icon = icon :: any,
				small = true,
				variant = "ghost",
				command = command :: any,
				browser = id,
				tooltip = label,
				disabled = disabled,
			}
		end
		-- A spinner while it loads, a lock over https, a file, or the web.
		local security = if busy
			then ui.icon { "loader-circle", spin = true, class = "browser-security" }
			elseif string.sub(address, 1, 8) == "https://" then ui.icon { "lock", class = "browser-security" }
			elseif string.sub(address, 1, 7) == "file://" then ui.icon { "file", class = "browser-security" }
			else ui.icon { "globe", class = "browser-security" }
		return ui.column {
			class = "browser",
			grow = true,
			ui.row {
				class = "browser-bar",
				align = "center",
				-- Disabled only when the page says so: some platforms can't.
				chrome("Back", "arrow-left", "browser-back", page.back == false),
				chrome("Forward", "arrow-right", "browser-forward", page.forward == false),
				if busy
					then chrome("Stop", "x", "browser-stop")
					else chrome("Reload", "rotate-cw", "browser-reload"),
				security,
				ui.input {
					id = id .. "-address",
					class = "browser-address",
					value = address,
					placeholder = "Search or enter an address",
					browser = id,
					mono = true,
					grow = true,
				},
				page.title and page.title ~= "" and ui.text { page.title, class = "browser-title", truncate = true },
				sharing and ui.button {
					icon = "share",
					small = true,
					variant = "ghost",
					tooltip = "Share",
					to = "app:share",
					msg = { url = address, text = if page.title and page.title ~= "" then page.title else nil },
				},
				chrome("Open in the system browser", "external-link", "browser-open"),
			},
			ui.browser {
				id = id,
				url = props.url,
				navigate = props.navigate,
				loading = props.loading,
				zoom = props.zoom,
			},
		}
	end,
}
