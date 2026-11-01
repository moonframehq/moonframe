-- A web page with an address bar: back, forward and reload buttons, the
-- address it shows, and a button that opens it in the system browser. The
-- page is a `ui.browser`; the bar drives it with the host's browser commands.

type Props = {
	-- Names the page within the mod; one page per id.
	id: string?,
	-- Where to go: http, https or file. A new value navigates.
	url: string?,
	-- The address and title to show, from the mod's `navigate` messages.
	at: string?,
	title: string?,
	-- Sent with `url` and `title` added as the page navigates.
	navigate: any,
	loading: any,
	zoom: number?,
}

return component {
	title = "Browser",
	category = "Content",
	icon = "globe",
	-- Offered by tab bars' + menu. A tab passes `navigate` and keeps the
	-- `url` and `title` it reports, so the tab shows the page it is on.
	tab = { title = "Browser", icon = "globe", props = { url = "https://luau.org" }, report = "navigate" },
	description = "A web page with an address bar: back, forward, reload, its address, and open in the system browser.",
	when = "A web page in a pane, with browser chrome. Keep the address the page reports in state (from `navigate`) and pass it as `at`. For a bare page without a bar use `ui.browser`.",
	props = {
		["id"] = "keeps the page across renders; one per id",
		["url"] = "address to load (http, https, file); a new value navigates",
		["at"] = "the address to show in the bar, from `navigate` messages",
		["title"] = "the page title to show, from `navigate` messages",
		["navigate"] = "message sent as the page navigates, with `url` and `title` added",
		["loading"] = "message sent as loads start and finish, with `loading` and `url` added",
		["zoom"] = "page zoom, 0.25 to 5",
	},
	styles = {
		[".browser"] = { gap = 0, radius = 8, border = true },
		[".browser-bar"] = { padding = { 4, 6 }, gap = 4, border = "bottom", background = "secondary" },
		[".browser-address"] = { padding = { 2, 8 }, radius = 6, background = "background", font = "mono", size = "small" },
		[".browser-title"] = { color = "muted-foreground", size = "small" },
	},
	example = { id = "example", url = "https://example.com", at = "https://example.com/", title = "Example Domain" },

	build = function(props: Props)
		local id = props.id or "browser"
		local function chrome(label: string, icon: string, command: string)
			return ui.button {
				icon = icon :: any,
				small = true,
				variant = "ghost",
				command = command :: any,
				browser = id,
				tooltip = label,
			}
		end
		return ui.column {
			class = "browser",
			grow = true,
			ui.row {
				class = "browser-bar",
				align = "center",
				chrome("Back", "arrow-left", "browser-back"),
				chrome("Forward", "arrow-right", "browser-forward"),
				chrome("Reload", "rotate-cw", "browser-reload"),
				ui.row {
					class = "browser-address",
					grow = true,
					align = "center",
					ui.text { props.at or props.url or "about:blank", truncate = true },
				},
				props.title and props.title ~= "" and ui.text { props.title, class = "browser-title", truncate = true },
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
