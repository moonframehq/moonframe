# moonframe.dev

The project's website: static HTML, no build step.

```text
site/                 what Netlify publishes (netlify.toml at the repo root)
  index.html          moonframe.dev: markup only
  home.css, home.js   its styles and script (the plates, viewer, install pickers, hero demo)
  film.js             the tour film's player
  docs/index.html     docs.moonframe.dev (served by a host rewrite in site/_redirects)
  docs/docs.css, docs/docs.js   the docs' styles and script, shared with the guides
  docs/guide.css      extra styles for a guide generated from Markdown
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

## Renderer screenshots

The home page's **GPUI by default. Rune, experimentally.** section uses
`img/rune-*.webp` and full-size `img/full/rune-*.png`. These are captures
from Rune's own headless renderer, explicitly labelled as such. See
[tools/rune-shots.md](tools/rune-shots.md) for the source revisions,
sample-data apps and reproduction commands.

## Patterns

Patterns are hidden from the site for now: nothing links to them, and
`site/_redirects` sends `/patterns/` to the docs home. To bring them back,
remove that redirect and restore the links in `site/index.html`,
`site/docs/index.html` and `site/llms.txt`.

`site/docs/patterns/` reads
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
and skips Netlify's automatic builds. In Netlify, add `moonframe.dev` as the
primary domain and `docs.moonframe.dev` as a domain
alias; `_redirects` routes the docs host to `/docs/` and sends
`moonframe.dev/docs/*` and `www` to their canonical addresses.

The [`Deploy website` workflow](../.github/workflows/site.yml) runs on pushes
to `main` that change `website/site/`, `netlify.toml`, or the workflow itself.
It uploads the committed static files without a build step:

```sh
netlify deploy --prod --dir=website/site --no-build
```

In the GitHub repository's **Settings → Environments → website**, restrict
deployment branches to `main` and add environment secrets
`NETLIFY_AUTH_TOKEN` (a Netlify personal access token) and
`NETLIFY_SITE_ID` (the Moonframe project's ID from Netlify project settings).
The deploy job uses this environment. Production uploads are serialized.
Regenerate and commit any generated pages
or assets before pushing; changes to source tools or mockups alone do not
trigger deployment.
