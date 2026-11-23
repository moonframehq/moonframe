---
name: moonframe-mockups
description: Show the user ideas for a UI as mockups drawn with Moonframe's own components, side by side, before building anything, so they can pick one (or ask for more) first. Use when the user asks for ideas, options, mockups, variants, alternatives, "a few ways to", "what could X look like" or "show me before you change it" for any UI in Moonframe (a panel, sidebar, status bar, dialog, card, a whole mod's look), for a screen or app not built yet on Moonframe, in the starter app or in an app running as a profile. Shows them in a Sheprd browser tab when running in Herdr, else in a Mockups tab of the user's window. Not for a change the user has already described exactly: make that with the moonframe skill.
---

# Mockups: ideas before a change

The user wants to see options before saying yes to one. Write the ideas
as **one Luau file** built from the components Moonframe already has,
let `moonframe mockup` check and draw it, and wait for their pick. Only
then build the real thing, with the **moonframe** skill (or
**moonframe-apps** for a new app). Because the mockup is real Luau on
real components, the picked variant's `body` is already most of the code.

## 1. Read before drawing

- The thing the user means: the mod and the lines that draw it. If they
  say "this" or "here", read `~/.cache/moonframe/selection.md` (see the
  moonframe skill, "Here: the user's selection").
- The **moonframe-components** skill. Build variants only from components
  and `ui.*` elements listed there, so the picked one is cheap to make
  real. `mockup check` rejects a component or class that does not exist.
- `style.luau` (the app's and the user's), to use its classes and colors
  rather than inventing new ones.

## 2. Write the mockup file

Somewhere that is not the app: a `design/` folder in the project, or the
scratchpad. Never `app/` or the real mod. One file per ask:

```lua
-- Mockups for: a busier status bar.
local meter = use "meter"

return {
	ask = "a busier status bar", -- shown on top and quoted in the pick
	width = 520,                 -- about the size the real thing gets
	-- height = 40,              -- unset: each PNG is as tall as it draws
	variants = {
		{ name = "Now", note = "Today's bar, for comparison.",
		  body = ui.row { gap = "small", ui.text "main" } },
		{ name = "Pills", note = "Each value in a pill; busier but scannable.",
		  body = ui.row { gap = "small", ui.badge "main", ui.badge "+3" } },
		{ name = "Meters", note = "CPU and memory as bars; costs width.",
		  -- A function gets `app`, for live provider data.
		  body = function(app)
			  local cpu = app.system and app.system.cpu or 0.42 -- demo data while nil
			  return ui.row { gap = "small", meter { value = cpu, label = "CPU" } }
		  end },
	},
}
```

- `body` is an element, a list of elements, or `function(app)` returning
  either. A variant may set its own `width` and `height`.
- Make **two to four that really differ**: a different layout, density or
  idea, not three paddings. Name each in two or three words; the `note`
  says in one line what it tries and what it gives up.
- Put **the current one first** ("Now") when changing something that
  exists, so the others read against it.
- Use **real pieces and real sizes**: the components and classes the real
  mod would use, `app.<provider>` data where it is there (labelled demo
  data while it is nil), and about 280 wide for a sidebar, 500 or more for
  a status bar or dialog.
- Keep it **inert**: no sends to providers or other mods (no commits,
  pushes or installs), and ids prefixed `mockup-` on terminals, browsers,
  inputs and instances.

## 3. Check it

```sh
moonframe mockup check FILE     # the installed app
moonframe --profile NAME mockup check FILE   # an app running as a profile
just mockup check FILE          # in the moonframe repository
```

Every problem is printed against the file's own line numbers. Fix them
all before showing anything.

## 4. Show it

**In Herdr** (`HERDR_ENV=1`), draw PNGs and open them in a Sheprd tab:

```sh
just mockup page FILE --open    # in the moonframe repository
```

This renders each variant headless, in light and dark, to
`<file's folder>/<name>.mockup/` (`--out DIR` to change it), writes an
`index.html` there on Sheprd's mockup kit, and opens it beside your
terminal. **Pick** sends the choice to you, and **Annotate** lets the user
mark what to change. Their reply arrives as your next prompt (see the
herdr-gpui-browser skill). From an SSH host Sheprd cannot open local
files: serve the folder (`python3 -m http.server 8000` in it) and run
`herdr-gpui browser open localhost:8000`.

Drawing PNGs needs a build with the `capture` feature, which `just mockup`
uses. An installed `moonframe` without it says so; use the live tab then.
`just mockup render FILE` writes the PNGs alone, which you can read to look
at your own work before showing it.

**Otherwise**, show the variants live in a tab of the user's own window:

```sh
moonframe mockup open FILE      # writes mods/mockup in the user's folder
```

The window loads it and brings up a **Mockups** tab with A, B, C. **Pick**
copies the choice for the user to paste back. Run `open` again after each
edit to the file.

Then tell the user briefly: one line per variant, and that they can pick,
or just say "B", "B with A's header" or "more like C". Then stop and wait.
Don't change anything real before they pick.

## 5. Iterate

- "More like C", "mix A and B": add variants to the same file (D, E) and
  keep the earlier ones, so the letters stay stable. Drop the ones they
  rule out.
- A new ask: a new file.

## 6. Apply the pick

1. Make the picked variant real in the target mod, with the moonframe
   skill (`app/` in this repository, `~/.config/moonframe` for the
   installed app), reusing its `body` code. For a new app, scaffold it
   with the moonframe-apps skill and move the body into its first mod.
2. `just check` (or `moonframe --check`).
3. If a live tab is open, run `moonframe mockup close`. It closes the tab
   first, then removes `mods/mockup`. Never delete that folder by hand:
   the tab would be left saying its mod is gone.
4. Say what changed and where. Delete the mockup file and its `.mockup`
   folder unless the user wants to keep them.

Do the same when the user drops the idea ("none of these", "never
mind"): `moonframe mockup close` if a tab is open, and change nothing
else.
