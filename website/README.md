# moonframe.dev

The project's website: static HTML, no build step.

```text
site/                 what Netlify publishes (netlify.toml at the repo root)
  index.html          moonframe.dev
  docs/index.html     docs.moonframe.dev (served by a host rewrite in site/_redirects)
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

The film under the hero is a stylised motion graphic, not a screen recording:
`tools/film/gen.mjs` draws each frame as SVG around the real screenshots in
`docs/screenshots/`, the agent icons in `assets/icons/` and the names in
`app/components/`. After changing a scene or a screenshot it uses, re-render
from the repo root (needs node, librsvg, ffmpeg with libx264,
and cwebp; about two minutes, with frames kept in `target/film`):

```sh
website/tools/film/render.sh
```

To check a scene first, draw only some frames (30 a second) and look at them:

```sh
node website/tools/film/gen.mjs 300 605 && rsvg-convert target/film/frames/f0605.svg -o /tmp/f.png
```

It writes `film.mp4`, its poster, and the muted `loop`
that previews the film (the first two scenes). Chapter times live in both
`film/chapters.vtt` and the chapter buttons in `index.html`; move them
together when a scene's timing changes.

## Hosting

One Netlify site serves both domains. `netlify.toml` publishes `website/site`
and skips deploys when nothing under `website/` changed. In Netlify, add
`moonframe.dev` as the primary domain and `docs.moonframe.dev` as a domain
alias; `_redirects` routes the docs host to `/docs/` and sends
`moonframe.dev/docs/*` and `www` to their canonical addresses.
