# moonframe.dev

The project's website: static HTML, no build step.

```text
site/                 what Netlify publishes (netlify.toml at the repo root)
  index.html          moonframe.dev
  docs/index.html     docs.moonframe.dev (served by a host rewrite in site/_redirects)
  components-browser.js, data/components.js   the Components browser on the home page
  img/                thumbnails (WebP, transparent corners) and img/full/ PNGs for the viewer
mockups/              the ten design directions the site was picked from, for reference
tools/components-data.py   regenerates data/components.js from app/components/*.luau
tools/og-images.py   renders the social cards from og/card.html and writes the Open Graph and X tags
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

## Hosting

One Netlify site serves both domains. `netlify.toml` publishes `website/site`
and skips deploys when nothing under `website/` changed. In Netlify, add
`moonframe.dev` as the primary domain and `docs.moonframe.dev` as a domain
alias; `_redirects` routes the docs host to `/docs/` and sends
`moonframe.dev/docs/*` and `www` to their canonical addresses.
