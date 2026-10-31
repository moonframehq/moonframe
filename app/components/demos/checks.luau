-- CI for a pull request: click the header to fold it, click a check to
-- re-run it (it goes pending; click again to finish it as passed).
local checks = use "checks"

return mod {
	init = function(previous)
		return previous
			or {
				collapsed = false,
				checks = {
					{ id = "linux", name = "ci / test (ubuntu)", state = "passed", detail = "4m 12s" },
					{ id = "macos", name = "ci / test (macos)", state = "passed", detail = "6m 40s" },
					{ id = "clippy", name = "ci / clippy", state = "failed", detail = "1 warning denied" },
					{ id = "release", name = "ci / build release", state = "pending", detail = "running 2m" },
					{ id = "review", name = "Review", state = "passed", detail = "Approved by alice" },
				},
			}
	end,

	update = function(state, msg)
		if msg == "fold" then
			state.collapsed = not state.collapsed
		elseif type(msg) == "table" and msg.rerun then
			for _, check in state.checks do
				if check.id == msg.rerun then
					if check.state == "pending" then
						check.state = "passed"
						check.detail = "1m 58s"
					else
						check.state = "pending"
						check.detail = "queued"
					end
				end
			end
		end
	end,

	view = function(state)
		return checks {
			checks = state.checks,
			collapsed = state.collapsed,
			toggle = "fold",
			event = "rerun",
		}
	end,
}
