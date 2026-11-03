# The tour film

The one-minute film in the tour section of moonframe.dev, and the muted loop
that previews it. It is a stylised motion graphic, not a screen recording:
`gen.mjs` draws every frame as SVG, in the site's palette, around the
repository's real screenshots, agent icons and component names, and
`render.sh` turns the frames into the files the page plays. Re-make it when a
feature lands that the film should show, or when the screenshots it uses
change.

## Make it

From the repository root. It needs node, librsvg (`rsvg-convert`), ffmpeg
with libx264, and cwebp; on macOS, `brew install librsvg ffmpeg webp`.

```sh
website/tools/film/render.sh
```

About two minutes. It scales the screenshots into `target/film/frames/img`,
writes the 1,800 frames as SVG, renders them in parallel, and writes into
`website/site/film/`:

| File | What |
| --- | --- |
| `film.mp4` | the film: 60s, 1920×1080, 30fps H.264, about 4 MB |
| `film-poster.webp` | its poster, shown until it plays |
| `loop.mp4`, `loop-poster.webp` | the first two scenes, muted, 1280 wide, played as the preview |
| `chapters.vtt` | the chapters (written by hand, see below) |

H.264 only: VP9 came out larger on these flat frames.

## Check a scene before rendering it all

Draw a few frames and look at them. Frame numbers are 30 a second; `--schedule`
says where each scene is. Run `render.sh` once first, so `target/film/frames/img`
holds the images the frames point at.

```sh
node website/tools/film/gen.mjs --schedule
node website/tools/film/gen.mjs 200 450 690
for f in 0200 0450 0690; do rsvg-convert target/film/frames/f$f.svg -o target/film/f$f.png; done
```

A contact sheet of one frame per scene shows the whole film at once:

```sh
cd target/film && magick montage f*.png -geometry 960x540+4+4 -tile 3x contact.png
```

## How `gen.mjs` is built

- **Palette and type.** `C` holds the site's colors (paper, ink, the sunset
  stripes, teal); headlines are Iowan Old Style, body Helvetica Neue, code
  JetBrains Mono, as on the site.
- **Primitives.** `text`, `rect`, `g` (opacity, move, scale, rotate), `img`
  (a screenshot with rounded corners and a shadow), `pill`, `cursor` (with a
  click ring: `pressAt`), `sun` (the icon's striped sun), `codeLine` (light
  Luau highlighting), `typed` (text typed out between two times), `wrap`.
- **Easing.** `prog(t, a, b)` gives 0 to 1 between two times; `eOut`, `eIO`
  and `eBack` shape it; `fadeIO(t, a, b)` fades a scene in and out.
- **Scenes.** One function per scene, `ask(t)`, `inspect(t)`, `agents(t)`,
  `live(t)` and so on, each opening with `caption(...)`: the numbered
  eyebrow, a serif headline and one sans line.
- **Two clocks.** `S` gives each scene a range on its own clock, and every
  cue inside it (a click at 14.05, typing from 14.6 to 16.0) is written in
  those times. `ORDER` is the order the film plays them in: `PLACE` lays them
  end to end, each starting 0.2s before the previous ends so they cross-fade,
  and `NUM` numbers their captions. Reordering the film is a change to
  `ORDER` alone.

## Add a scene for a new feature

1. **Write the scene.** A function `feature(t)` that starts with
   `const [a, b] = S.feature`, fades with `fadeIO(t, a, b)`, and opens with
   `caption(t, a, b, NUM.feature, "Eyebrow", "Headline.", "One line.")`. Give
   it a range in `S` on a clock of its own (any free span, such as
   `[60, 66]`), and add it to `draw` in `frame()`.
2. **Place it** in `ORDER`. The film grows by the scene's length less 0.2s;
   to keep it near a minute, shorten or drop another scene, or raise `DUR`
   (`--schedule` warns when they disagree).
3. **Ground it in the repo.** Use real screenshots (`docs/screenshots/`, added
   to the list `render.sh` scales), real file, mod and component names, and
   real line numbers. What is drawn rather than captured, such as an agent's
   steps, stays plausible and is covered by the site's caption, which says the
   film is stylised and the agent's steps illustrative.
4. **Keep the claims exact.** Anything the film says about safety, platforms
   or limits must match the README's [What runs with what
   access](../../../README.md#what-runs-with-what-access). A review caught the
   sandbox scene once promising that a Wasm provider can't touch your machine,
   which is untrue while building one from source runs Cargo as you.
5. **Look at frames** from the scene (above), then render everything.
6. **Move what depends on times.** `node gen.mjs --schedule` prints each
   scene's seek time and label:
   - the chapter buttons in `website/site/index.html` (`data-t` is the seek,
     `<time>` the label) and `website/site/film/chapters.vtt`;
   - the section's lead text in `index.html`, which lists the scenes in order;
   - in `render.sh`, the poster frame (now 329, the best month turning orange),
     and the loop's first frame, frame count and fade-out, which cover the
     first two scenes.
7. **Open a pull request** with screenshots of the section (below).

## The story it tells

Agents first: the film opens on asking the running app for a change, then
pointing at something with the inspector, then coding agents in a mod. Live
Luau comes after, as what happens underneath, and there too the edit is the
agent's while the person only clicks. Nobody edits a file by hand on screen.
A new feature goes where it fits that story, not at the front by default.

## Screenshots for the pull request

Capture the tour section in a browser at 1440 and 390 pixels wide, before
playing, while playing (seek into the scene you changed), and on a phone;
`website/screenshots/home-tour*.webp` are the current ones. Serve the site
with range requests, or seeking fails: `npx http-server -p 8765 website/site`
works, Python's `http.server` does not. Headless Chrome's `--screenshot`
clamps narrow windows and ignores `#tour`, so drive a browser instead
(Playwright: set the viewport, scroll `#tour` into view, click a chapter, wait
until the film's `currentTime` reaches it).

## Things that went wrong once

- `rsvg-convert` reads images relative to the SVG, so frames point at `img/`
  beside them; render from `target/film/frames` or give absolute paths.
- The tag after each sandbox title sits at a measured width (`titleW`):
  character counts misjudge Iowan Old Style by a third either way.
- A `hidden` button with its own `display` rule stays visible; the page's CSS
  has `[hidden] { display:none }` for the film's buttons.
- The loop's poster is cropped like the loop, or it shows the film's progress
  line.
