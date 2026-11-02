-- The forge corner of the title bar, as herdr-gpui has it: the current
-- branch's pull (or merge) request with its lines, a click away from the
-- browser, falling back to the checkout's uncommitted lines; then the
-- account, whose click opens the sign-in in a dialog. GitHub or GitLab,
-- whichever the repository's remote is on.
--
-- It is in providers/github's and providers/gitlab's `accounts`, so the
-- sign-in's buttons may sign in and out.

local forge_pr = use "forge-pr"
local forge_account = use "forge-account"
local forge_sign_in = use "forge-sign-in"
local dialog = use "dialog"

type State = { open: boolean }

return mod {
	title = "Forge",

	init = function(): State
		-- A dialog left open is not reopened at the next launch.
		return { open = false }
	end,

	update = function(state: State, msg: any)
		if msg == "account" then
			state.open = not state.open
		elseif msg == "close" then
			state.open = false
		elseif type(msg) == "table" and msg.reply and msg.ok and msg.login then
			-- Signed in: the dialog has done its job.
			state.open = false
		end
		return state
	end,

	view = function(state: State, app)
		local git = app.git
		local remote = git and git.remote
		local provider = if remote and remote.kind == "gitlab" then "gitlab" else "github"
		local forge = if provider == "gitlab" then app.gitlab else app.github
		return ui.row {
			gap = "small",
			align = "center",
			forge_pr {
				forge = forge,
				changes = git and { added = git.added, deleted = git.deleted },
			},
			forge_account { forge = forge, brand = provider, msg = "account" },
			dialog {
				open = state.open,
				dismiss = "close",
				title = if provider == "gitlab" then "GitLab" else "GitHub",
				subtitle = forge and forge.host,
				icon = if provider == "gitlab" then "brand-gitlab" else "brand-github",
				size = "small",
				forge_sign_in { forge = forge, provider = provider },
			},
		}
	end,
}
