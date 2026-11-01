-- Offers to copy the app's agent skills (moonframe, its components, apps and
-- mockups) to ~/.config/moonframe/.claude/skills, so an agent working on the
-- user's mods there knows how to build them: a toast in the window's corner,
-- from what the `skills` provider sees, and only once the user agrees.
-- After an update ships newer skills it offers again. × hides the offer
-- until the skills change; that is saved. It waits while herdr-core's offer
-- is up in the same corner (the `offers/herdr` topic).
local toast = use "toast"

type State = {
	-- The shipped skills' version the user said no to.
	dismissed: string?,
	asked: boolean,
	-- herdr-core's offer is showing.
	herdr: boolean,
}

return mod {
	region = "status-right",
	order = 101,

	subscribe = { "offers/herdr" },

	init = function(previous: State?): State
		return { dismissed = previous and previous.dismissed, asked = false, herdr = false }
	end,

	update = function(state: State, msg)
		if msg == "install" then
			state.asked = true
			return state, { { to = "provider:skills", msg = "install" } }
		elseif type(msg) == "table" and msg.topic == "offers/herdr" then
			state.herdr = msg.value == true
		elseif type(msg) == "table" and msg.dismiss then
			-- From the offer, or the result once read.
			if not state.asked and msg.version then
				state.dismissed = msg.version
			end
			state.asked = false
		end
		return nil
	end,

	view = function(state: State, app)
		local skills = app.skills
		local card: any = nil
		if skills == nil or not skills.available then
			card = nil
		elseif skills.installing then
			card = toast {
				id = "skills",
				icon = "loader-circle",
				source = "Agent skills",
				title = "Copying the skills…",
				body = `Into {skills.target}`,
				dismiss = false,
			}
		elseif skills.error and state.asked then
			card = toast {
				id = "skills",
				level = "error",
				source = "Agent skills",
				title = "The skills were not copied",
				body = skills.error,
				full = true,
				action = { label = "Try again", msg = "install" },
			}
		elseif skills.fresh and state.asked then
			card = toast {
				id = "skills",
				level = "success",
				source = "Agent skills",
				title = "Agents know Moonframe now",
				body = `The skills are in {skills.target}. An agent started in ~/.config/moonframe uses them.`,
				full = true,
			}
		elseif skills.status ~= "current" and state.dismissed ~= skills.version and not state.herdr then
			local update = skills.status == "stale"
			card = toast {
				id = "skills",
				-- No for this version: a newer one offers again.
				dismiss = { dismiss = "skills", version = skills.version },
				source = "Agent skills",
				title = if update then "Update the agent skills" else "Teach agents Moonframe",
				body = (if update
					then `This version has newer skills than {skills.target}. `
					else `Copy the app's skills to {skills.target}? `)
					.. "An agent working on your mods there then knows how to build them, and can mock up ideas in a tab before changing anything.",
				full = true,
				action = { label = if update then "Update" else "Copy", msg = "install" },
			}
		end
		return ui.overlay {
			open = card ~= nil,
			place = "bottom-end",
			content = card or ui.text "",
		}
	end,
}
