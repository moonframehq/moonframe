# Patterns

Patterns sit between individual components and complete demo apps: reusable
compositions with deliberate hierarchy, spacing and interaction boundaries.
They are normal Luau components, not a second UI framework.

- **[Browse the docs library](https://docs.moonframe.dev/patterns/):** category
  filters, search by name, description or constituent component, native
  screenshots, and side-by-side example/source code. Each pattern has a
  shareable URL, for example `#issue-card`.
- **`just patterns`:** an independent native app alongside whatever you are
  building. Live previews have their own state; they use sample data and
  perform no network or provider actions.
- **[The main site](https://moonframe.dev/#patterns):** an editorial introduction
  to the collection and its design choices.

## First collection

| Pattern | Category | Composition |
| --- | --- | --- |
| Workspace shell | Shells | Title bar, activity bar, local navigation and a generous content area |
| Workspace switcher | Navigation | Current workspace, avatars and an anchored selection menu |
| Issue card | Workflows | Wrapping title, blockers, labels and quiet metadata |
| Getting started | Onboarding | Progress, completed steps and one expanded next action |

## Use a pattern

1. Copy the desired file from `patterns/app/components/` to your app's
   `components/` folder. Do not copy `pattern-browser.luau`: that is tooling.
2. Call it with `use`, exactly like any other component:

   ```lua
   local issue_card = use "issue-card"

   issue_card {
       title = "Dark mode for the settings page",
       identifier = "APP-150",
       labels = { "feature" },
       priority = "high",
       assignee = "Jamie",
       msg = { open_issue = "APP-150" },
   }
   ```

3. Keep business data and message handlers in the calling mod. The **Example**
   pane shows a complete runnable mod (`patterns/app/mods/preview-<id>/main.luau`).
   To try it in your app, copy it to `mods/example/main.luau` and put
   `ui.mount "example"` in the shell.
4. Restyle the pattern's named classes in `style.luau`. The pattern's own
   defaults use theme colors and work in light and dark mode.

The workspace shell is also usable inside a panel: omit `window` and supply
`height`. For the real window, pass `window = app.window`, set your shell's
`titlebar = "hidden"`, and leave `height` unset. Each shell in one mod needs
a unique `id` for its split.

## Independent, opt-in developer app

`just patterns` generates the catalog, then runs:

```sh
cargo run --release -p moonframe-app --bin moonframe-patterns \
  --no-default-features --features patterns -- --dev
```

The optional `patterns` feature enables a **separate binary target**, not
additional code in the starter or the library. Its files live outside `app/`,
so the normal toolbox embedding and bundle staging never include them.
The browser embeds its own app for running away from the checkout; in a
checkout its Luau reloads live. Its identity, config, cache and saved state
are `moonframe-patterns`, separate from your app and the starter.

Consumers copy only the patterns they use. They need no Cargo feature to use
those copied Luau components. Normal `just run` still launches the starter.

## Add a pattern

1. Write `patterns/app/components/<id>.luau` with `title`, `category`, `icon`,
   `description`, `when` (the design reasoning), `props`, `styles`, `example`
   and `build`. Give it a narrow API, useful content slots and named style
   classes. Depend on existing built-in components.
2. Add `patterns/app/mods/preview-<id>/main.luau`. Use realistic, explicitly
   sample data and local-only interactions. This is the exact code the
   native and web browsers show in their Example panes.
3. Run `python3 scripts/patterns-data.py`. It discovers preview mods and
   reads their actual components, deriving categories and dependencies.
   There is no separate hand-maintained web catalog.
4. Run `just patterns-check`. Capture light and dark native previews with
   `just pattern-shots` on macOS. This enables the optional `pattern-capture`
   feature to export the app's own GPU frame, without desktop recording.
   The website uses these real screenshots, not an HTML approximation of
   the native controls.

`python3 scripts/patterns-data.py --check` verifies both generated catalogs.
`just ci` includes the pattern checks. A component change must refresh its
source export and screenshots; the website's source is never a handwritten
copy of the implementation.
