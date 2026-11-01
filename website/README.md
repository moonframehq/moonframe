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

## Hosting

One Netlify site serves both domains. `netlify.toml` publishes `website/site`
and skips deploys when nothing under `website/` changed. In Netlify, add
`moonframe.dev` as the primary domain and `docs.moonframe.dev` as a domain
alias; `_redirects` routes the docs host to `/docs/` and sends
`moonframe.dev/docs/*` and `www` to their canonical addresses.
