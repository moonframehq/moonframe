-- A ready-made sign-in for GitHub or GitLab, drawn from the provider's
-- `auth`: a device code to enter on the forge's site, the CLI's own
-- sign-in, or a hint at gh, glab and the environment; and who is signed
-- in, with Sign out. Its
-- buttons talk to the provider directly, so the mod drawing it must be in
-- that provider's `[settings] accounts`. Put it in any container: a
-- `dialog`, a popover, a settings page.

local loading = use "loading"

type Props = {
	forge: any,
	provider: string?,
}

local SOURCES = {
	saved = "Signed in from this app",
	env = "From the environment (GH_TOKEN, GITHUB_TOKEN or GITLAB_TOKEN)",
	cli = "With the CLI's own sign-in",
}

return component {
	title = "Forge Sign-in",
	category = "Git",
	icon = "key-round",
	description = "Sign in to GitHub or GitLab from the provider's state: a device code with Open and Copy, the gh/glab sign-in; who is signed in and Sign out.",
	when = "The body of the app's own sign-in, opened from `forge-account` or a settings page. Draw your own instead from `app.github.auth` when it should look different; send the provider the same messages.",
	props = {
		["forge"] = "`app.github` or `app.gitlab`",
		["provider"] = "github (default) | gitlab: the provider its buttons talk to",
	},
	styles = {
		[".forge-sign-in"] = { gap = 12 },
		[".forge-sign-in-code"] = { size = "headline", weight = "semibold", font = "mono" },
		[".forge-sign-in-muted"] = { color = "muted-foreground", size = "small" },
		[".forge-sign-in-error"] = { color = "danger", size = "small" },
	},
	example = {
		forge = {
			available = false,
			reason = "not signed in to GitHub",
			host = "github.com",
			auth = {
				status = "pending",
				host = "github.com",
				device = true,
				cli = true,
				code = "WDJB-MJHT",
				url = "https://github.com/login/device",
			},
		},
		provider = "github",
	},

	build = function(props: Props)
		local provider = props.provider or "github"
		local to = `provider:{provider}`
		local title = if provider == "gitlab" then "GitLab" else "GitHub"
		local cli = if provider == "gitlab" then "glab" else "gh"
		local env = if provider == "gitlab" then "GITLAB_TOKEN" else "GH_TOKEN"
		local forge = props.forge
		local auth = forge and forge.auth
		if not auth or auth.status == "checking" or auth.status == "verifying" then
			return ui.column { class = "forge-sign-in", loading { text = "Checking the sign-in…" } }
		end
		local error_text = auth.error and ui.text { auth.error, class = "forge-sign-in-error", wrap = true }

		if auth.status == "signed_in" then
			local viewer = forge.viewer
			return ui.column {
				class = "forge-sign-in",
				ui.row {
					gap = "normal",
					align = "center",
					viewer and viewer.avatar_path and ui.image {
						viewer.avatar_path,
						width = 32,
						height = 32,
						fit = "cover",
						style = { radius = 999 },
					},
					ui.column {
						gap = "none",
						ui.text { `Signed in to {auth.host} as {auth.login or "…"}`, weight = "medium" },
						ui.text { SOURCES[auth.source or "saved"] or "", class = "forge-sign-in-muted" },
					},
				},
				error_text,
				ui.row {
					ui.spacer(),
					ui.button {
						"Sign out",
						variant = "outline",
						disabled = auth.source == "env",
						to = to,
						msg = { sign_out = true },
					},
				},
			}
		end

		if auth.status == "pending" and auth.code then
			local page = auth.complete_url or auth.url
			return ui.column {
				class = "forge-sign-in",
				ui.text { `Enter this code on {auth.host}:`, wrap = true },
				ui.row {
					gap = "normal",
					align = "center",
					ui.text { auth.code, class = "forge-sign-in-code" },
					ui.button { "Copy", icon = "copy", variant = "ghost", small = true, copy = auth.code },
				},
				loading { text = "Waiting for you to approve it…" },
				ui.row {
					ui.button { "Cancel", variant = "ghost", to = to, msg = { cancel = true } },
					ui.spacer(),
					page and ui.button {
						`Open {auth.host}`,
						icon = "external-link",
						variant = "primary",
						to = "app:open",
						msg = page,
					},
				},
			}
		end

		return ui.column {
			class = "forge-sign-in",
			ui.text {
				`Sign in to {title} to see pull requests, checks and issues for this repository.`,
				wrap = true,
			},
			auth.device and ui.button {
				`Sign in with {title}`,
				icon = (if provider == "gitlab" then "brand-gitlab" else "brand-github") :: any,
				variant = "primary",
				to = to,
				msg = { sign_in = true },
			},
			auth.cli and ui.button {
				`Use {cli}'s sign-in`,
				icon = "terminal",
				variant = "outline",
				to = to,
				msg = { use_cli = true },
			},
			-- No pasted-token field: text fields show what is typed, and a
			-- token must not sit on screen. An app with a masked field of its
			-- own sends the provider `{ token = "..." }`.
			ui.text {
				`Or sign in with {cli} in a terminal ({cli} auth login), or set {env}, and it is used.`,
				class = "forge-sign-in-muted",
				wrap = true,
			},
			error_text,
		}
	end,
}
