-- The forge account button of a title bar: the signed-in user's picture,
-- or the forge's logo with a dot while no one is signed in (or the last
-- read failed). A click sends `msg`, so the app opens whatever it draws
-- the sign-in in: a dialog, a popover, a settings page.

local avatar = use "avatar"

type Props = {
	forge: any,
	brand: string?,
	msg: any,
	to: string?,
}

return component {
	title = "Forge Account",
	category = "Git",
	icon = "brand-github",
	description = "The GitHub or GitLab account in a title bar: the user's picture, or the logo with a dot when signed out; a click opens the app's own sign-in.",
	when = "The account control beside `forge-pr` in a title bar or sidebar footer. What the click opens is the app's: show `forge-sign-in` in a `dialog` or a popover, or a settings page of your own.",
	props = {
		["forge"] = "`app.github` or `app.gitlab`",
		["brand"] = "github (default) | gitlab: the logo when no one is signed in",
		["msg"] = "message sent on a click (open the app's sign-in)",
		["to"] = "the mod to send `msg` to",
	},
	styles = {
		[".forge-account"] = { padding = 4, radius = 6, color = "muted-foreground" },
		[".forge-account:hover"] = { background = "foreground/8", color = "foreground" },
		[".forge-account-dot"] = { radius = 999, background = "warning" },
	},
	example = {
		forge = {
			available = false,
			reason = "not signed in to GitHub",
			host = "github.com",
			auth = { status = "signed_out", host = "github.com", device = true, cli = true },
		},
		brand = "github",
		msg = "account",
	},

	build = function(props: Props)
		local forge = props.forge
		local auth = forge and forge.auth
		local signed_in = auth ~= nil and auth.status == "signed_in"
		local login = auth and auth.login
		local viewer = forge and forge.viewer
		local logo = if props.brand == "gitlab" then "brand-gitlab" else "brand-github"
		local title = if props.brand == "gitlab" then "GitLab" else "GitHub"
		local attention = auth ~= nil and (not signed_in or auth.error ~= nil) and auth.status ~= "checking"
		local tooltip = if signed_in and login
			then `{title}: {login}`
			elseif auth and auth.status == "pending" then `{title}: waiting for the code`
			else `Sign in to {title}`
		return ui.row {
			class = "forge-account",
			hug = true,
			shrink = false,
			align = "center",
			justify = "center",
			tooltip = tooltip,
			msg = props.msg,
			to = props.to,
			if signed_in and login
				then avatar {
					name = login,
					src = viewer and viewer.avatar_path,
					size = "small",
					shape = "circle",
					tooltip = tooltip,
				}
				else ui.icon { name = logo :: any },
			attention and ui.row {
				class = "forge-account-dot",
				width = 7,
				height = 7,
				float = { top = 2, right = 2 },
			},
		}
	end,
}
