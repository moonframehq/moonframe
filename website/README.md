# moonframe.dev

The project's website: static HTML, no build step.

```text
site/                 what Netlify publishes (netlify.toml at the repo root)
  index.html          moonframe.dev
  docs/index.html     docs.moonframe.dev (served by a host rewrite in site/_redirects)
  llms.txt            for coding agents: how to start a Moonframe app in the user's project
  components-browser.js, data/components.js   the Components browser on the home page
  img/                thumbnails (WebP, transparent corners) and img/full/ PNGs for the viewer
  img/components/     each component's preview, cut from the app's Components window
  film/               the tour film, its muted preview loop, posters and chapters
mockups/              the ten design directions the site was picked from, for reference
tools/components-data.py   regenerates data/components.js from app/components/*.luau
tools/component-shots.py   screenshots each component's preview into img/components/
tools/og-images.py   renders the social cards from og/card.html and writes the Open Graph and X tags
tools/film/          draws and encodes the tour film (gen.mjs, render.sh)
```

Preview locally with any static server, from `site/`:

```sh
cd website/site && python3 -m http.server 8000
```

After adding or changing components, refresh the browser's data from the
repo root:

```sh
python3 website/tools/components-data.py
```

then re-capture the previews of the ones you changed (macOS, needs Screen
Recording permission; with no names it captures every component, which
takes a while):

```sh
uv run --with pillow website/tools/component-shots.py NAME ...
```

A component without a picture shows a pointer to the app instead.

## Patterns

The home page's editorial Patterns section and `site/docs/patterns/` share
`site/docs/patterns/data.js`, generated from the real Luau pattern definitions
and preview mods. The docs offer a searchable category grid and a side-by-side
preview/code detail page, with deep links to each pattern.

```sh
python3 scripts/patterns-data.py
just patterns-check
just pattern-shots  # macOS; exports the app's own GPU frame
```

Previews are real native screenshots in `site/docs/patterns/previews/`, in
light and dark variants. The website identifies them as screenshots; the
interactive previews run in the independent app opened by `just patterns`.
See [the pattern guide](../docs/patterns.md) for authoring and distribution.

The social title, description and image alt text live in `PAGES` in
`tools/og-images.py`; the tool writes them into each page's
`<!-- social -->` block, so edit them there, not in the page (the page's
`<title>` is separate). After changing that copy or `og/card.html`,
re-render the cards (needs Chrome or Chromium; set `CHROME` if it isn't
found):

```sh
python3 website/tools/og-images.py
```

Each card is named by a hash of its bytes (`social/home-1a2b3c4d5e.png`), so
a changed card gets a new URL and social sites fetch it instead of a cached
copy. Old cards are kept, so links shared before a change keep their image;
delete one only once nothing should point at it.

## The tour film

The film in the tour section is a stylised motion graphic, drawn by
`tools/film/gen.mjs` around the real screenshots and rendered by
`tools/film/render.sh` into `site/film/`. When a feature lands that it should
show, or a screenshot it uses changes, follow
[tools/film/README.md](tools/film/README.md): how the film is built, how to
add a scene, what to check, and the chapter times to move with it.

## Hosting

One Netlify site serves both domains. `netlify.toml` publishes `website/site`
and skips deploys when nothing under `website/` changed. In Netlify, add
`moonframe.dev` as the primary domain and `docs.moonframe.dev` as a domain
alias; `_redirects` routes the docs host to `/docs/` and sends
`moonframe.dev/docs/*` and `www` to their canonical addresses.
