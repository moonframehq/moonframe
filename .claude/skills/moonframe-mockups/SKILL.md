---
name: moonframe-mockups
description: Show the user ideas for a change in Moonframe as live Luau mockups, side by side in a tab of their own window, before changing anything, so they can pick one (or ask for more) first. Use when the user asks for ideas, options, mockups, variants, alternatives, "a few ways to", "what could X look like" or "show me before you change it" for any UI in Moonframe (a panel, sidebar, status bar, dialog, card, a whole mod's look), in the starter app or in an app running as a profile. Not for a change the user has already described exactly: make that with the moonframe skill.
---

# Mockups: ideas before a change

The user wants to see options before saying yes to one. Draw them as live
UI in the user's own window, in a throwaway mod that touches nothing real,
and wait for their pick. Only then change the real mod, with the
**moonframe** skill.

The pieces, all built in:
- `use "mockups"`: the variants side by side, each in a lettered card
  (A, B, C) with a **Pick** button that copies `Apply mockup B ("Name") for:
  <ask>. The mockups are in <file>.` for the user to paste back to you.
- A mod with a `tab` entry is a kind of main tab (`mod:<name>`).
- The shell takes `{ open_tab = "<kind>" }` from any mod and brings that
  tab up: the open one, else a new one. So the mockup shows itself.
  `{ close_tab = "<kind>" }` closes every tab of that kind, so it goes away
  the same way.

## 1. Read before drawing

- The thing the user means: the mod and the lines that draw it. If they
  say "this" or "here", read `~/.cache/moonframe/selection.md` (see the
  moonframe skill, "Here: the user's selection").
- The **moonframe-components** skill, to build variants from components the
  app already has, so the picked one is cheap to make real.
- `style.luau` (the app's and the user's), to use its classes and colors
  rather than inventing new ones.

## 2. Write the mockup mod

Always `~/.config/moonframe/mods/mockup/main.luau` (`$XDG_CONFIG_HOME` if
set; for an app running as a profile,
`~/.config/moonframe/profiles/<name>/mods/mockup/main.luau`). Never `app/`,
never the real mod: nothing about the app changes until the user picks.
There is one mockup mod; a new ask replaces the file and bumps `ROUND`.

```lua
-- Mockups for: <the ask>. Throwaway: deleted once one is picked.
local mockups = use "mockups"

-- Bump it for each new ask, so the tab comes up again.
local ROUND = "status-bar-1"
-- Set to true once the user has picked: the tab closes, then the folder goes.
local DONE = false

return mod {
	title = "Mockups",
	tab = { title = "Mockups", icon = "sparkles" },

	init = function(previous, props)
		local state = previous or {}
		-- Only the mod in its own place opens and closes the tab; the tab
		-- itself is an instance, which gets props.
		if props ~= nil then
			return state
		elseif DONE then
			return state, { { to = "shell", msg = { close_tab = "mod:mockup" } } }
		elseif state.round ~= ROUND then
			return { round = ROUND }, { { to = "shell", msg = { open_tab = "mod:mockup" } } }
		end
		return state
	end,

	view = function(state, app)
		return mockups {
			ask = "a busier status bar",
			file = "~/.config/moonframe/mods/mockup/main.luau",
			width = 520, -- about the size the real thing gets
			variants = {
				{ name = "Pills", note = "Each value in a pill.", body = ui.row { ... } },
				{ name = "Plain", note = "Muted text, no chrome.", body = ui.row { ... } },
				{ name = "Meters", note = "CPU and memory as bars.", body = ui.row { ... } },
			},
		}
	end,
}
```

What makes the variants worth picking from:
- **Two to four, really different**: a different layout, density or idea,
  not three paddings. Name each in two or three words; the `note` says in
  one line what it tries and what it gives up.
- **The current one first** when changing something that exists ("Now"),
  so the others read against it.
- **Real pieces**: the components and classes the real mod would use, and
  `app.<provider>` data where it is there (fall back to labelled demo data
  while it is nil). A body may call the real mod's own helpers by copying
  them into the file.
- **The real size**: set `width` to what the target gets (about 280 for a
  sidebar, 500 or more for a status bar or a dialog), and give a body a
  `height` when its height matters.
- **Inert**: no sends to providers or other mods (no commits, pushes or
  installs), and ids prefixed `mockup-` on terminals, browsers, inputs and
  instances so none clashes with the real ones. Clicks may change the
  mockup's own state to show how a variant behaves (open a menu, switch a
  tab); keep that in `update` of this mod.

Then check it: `just check` in this repository, `moonframe --check`
with the installed app (add `--profile <name>` for a profile). Fix every
problem it lists for `mockup` before telling the user.

If the window has a shell of its own without `open_tab` (a whole app as a
profile), give the mod a `region` the shell draws instead, or tell the user
where to open it.

## 3. Hand it over

Tell the user, briefly: a **Mockups** tab opened with A, B, C; one line per
variant; **Pick** copies the choice to paste back, or they can just say
"B", "B with A's header", or "more like C". Then stop and wait. Don't
change the real mod before they pick.

## 4. Iterate

- "More like C", "mix A and B": add variants to the same file (D, E) and
  keep the earlier ones, so letters stay stable. Drop ones they rule out.
- A new ask: replace the variants and bump `ROUND` so the tab comes up.

## 5. Apply the pick

1. Make the picked variant real in the target mod, with the moonframe
   skill (`app/` in this repository, `~/.config/moonframe` for the
   installed app), reusing the mockup's body code.
2. `just check` (or `moonframe --check`).
3. Close the Mockups tab **before** deleting anything: set `DONE = true`
   in the mockup file. The window reloads it and the mod sends the shell
   `close_tab`. Wait two seconds for the window to pick that up.
4. Then delete the `mods/mockup/` folder. Deleted first, the mod could no
   longer close its tab, and the tab would be left saying its mod is gone.
5. Say what changed and where.

The same when the user drops the idea ("none of these", "never mind"):
`DONE = true`, wait, delete the folder, change nothing else.
