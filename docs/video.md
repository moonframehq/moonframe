# Live video

`ui.video` shows frames another process makes, at the rate it makes them,
without passing through Luau or the window's drawing. It is for a camera, a
scanner or a player in an engine of its own. macOS only, behind the `video`
feature (the starter has it; a library app adds `features = ["video"]`).

`examples/framebench` is a whole working app, with a stand-in engine in
Python and the figures to judge it by. Run it before building anything.

## How the parts fit

```text
engine (any language)                 Moonframe
  frames ──► ring in shared memory ──► ui.video's thread ──► Metal layer
  state  ◄── Unix socket, JSON lines ◄► socket provider ──► app.<name>, Luau
```

- **Frames** go through a ring in POSIX shared memory, never JSON or files.
  The engine never waits for the viewer, and a viewer that falls behind skips
  to the newest frame. This is a preview: frames you record go on their own
  path in the engine, whatever happens here. The layout is documented in
  [`crates/moonframe-frames`](../crates/moonframe-frames/src/lib.rs),
  `examples/framebench/engine/ring.py` writes it from Python, and
  `moonframe_frames::Writer` from Rust.
- **Commands and state** go through a [socket provider](#the-engine-link).
- Each `ui.video` has a thread that uploads and presents on its own, so
  video and the window can't hold each other up.

Formats are `rgba8`, `rgba16`, `rgba16f`, `mono16` and `bayer16` (12-bit
sensors in 16-bit words: set `white = 4095`). The engine picks size and format
per frame, so a reduced preview during capture and a full frame at rest are
just different frames; a second `source` works too. Demosaicing is bilinear,
for preview; an engine that wants better sends finished colour.

## Colour

Nothing between the engine and the screen is 8-bit. The layer is 16-bit float,
and `color` tags it with what the pixels are (`srgb`, `display-p3`, `rec709`,
`rec2020`, `linear-srgb`, `linear-p3`). macOS matches that to each screen's
profile, on whichever screen the window is, including across a move. What a
screen finally shows depends on it: most are 8 or 10 bits per channel.

The transform is yours and explicit: `black`, `white`, `gain`, `gamma`, in
that order, nothing else. `gamma = 1` and `gain = 1` leave pixels alone, so
there is no hidden second correction. `color = "none"` turns matching off too,
for a source that made pixels for that very screen. `extended = true` lets
values above 1 reach a screen that shows them.

`probe` reads a pixel as the source has it and as the transform shows it, by
the same sum the shader does (`moonframe_frames::Tone`). What ColorSync does
after that is the system's, and the probe doesn't report it.

There is no 3D LUT or ICC file input: apply one in the engine. `ui.image` is
another matter: it decodes to 8 bits and doesn't read embedded profiles, so
judge pictures in `ui.video`.

## Over the picture

The picture is a native view above the window's drawing, like `ui.browser`'s
page, so nothing the window draws can lie over it, and it hides while a
popover or dialog reaches it. Crop frames and guides are `guides`, which the
host draws in the picture. Events pass through it, so `pointer` and `wheel` on
an element around it work. `video-viewer` puts this together, with zoom, pan,
100 % and pixel reading, its controls under the picture.

`zoom` counts device pixels: at 1, one picture pixel is one on the screen.
From 200 % pixels show square (`filter` changes that).

## Measuring

`stats` arrives each second with `fps`, `shown`, `skipped`, `latency` (the age
of a frame when handed to the screen, against the engine's capture stamp, in
ms; it leaves out the display's own delay), `live`, `width`, `height`, `format`,
`frame`, `fit` and `density`. `RUST_LOG=moonframe::perf=debug` shows what the
window itself costs. Measure release builds, at the real frame size, while
dragging a slider.

## The engine link

A `kind = "socket"` provider speaks the lines of a process provider to a
program that runs on its own:

```toml
kind    = "socket"
socket  = "~/Library/Application Support/Nova/engine.sock"
publish = "latest"          # readings repeat; only the newest matters
urgent  = ["stop"]          # goes ahead of everything waiting
latest  = ["exposure"]      # a dragged slider leaves one value in the queue
```

| Moonframe guarantees | Yours to build |
| --- | --- |
| Connects, reconnects with backoff, tells `app.links.<name>.connected` | Serving the socket; its permissions |
| Says hello on each connection: `{"from":"","message":{"connected":true}}`, so you can publish real state | Answering it with what is true |
| Order of ordinary messages; an `urgent` one goes first and is never dropped while linked | Acting on commands, and saying what happened |
| Counts every dropped message in `app.links.<name>.dropped` | Deciding what a drop means for the operator |
| Leaves the program running when the app quits, reloads or crashes | Keeping capture safe with no app attached |

Nothing is delivered for certain: a sent message says nothing about its
effect, so show what `app.<name>` says, not what you asked. Messages sent with
no link are dropped, never kept, since a command held back would run at some
unknown later moment. An `urgent` message still waits behind a line being
written, so an engine must keep reading its socket.

## Closing

`app:guard` holds back closing the main window and quitting:

```lua
-- As capture starts:
return state, { { to = "app:guard", msg = { hold = "closing", timeout = 30 } } }

-- In `update`: msg.msg == "closing", msg.closing is "window" or "quit".
-- Ask the engine to stop; once `app.engine` confirms it:
return state, { { to = "app:guard", msg = { release = true } } }
```

`{ hold = false }` stops holding. Past `timeout` from the first attempt, the
next attempt goes through, so a stuck mod can't trap the user. Quitting from
the Dock or logging out goes around it: GPUI gives no way to refuse those,
which is one more reason to keep capture safe in the engine.

A `kind = "process"` provider sees stdin close, has `stop-grace` seconds (0.25
by default, up to 30) to leave, then gets SIGTERM, then 0.25 s, then a kill.
This is also what a reload does when its folder changed, and quitting waits
for it. A socket provider's program is left alone.

A bundle watches no files out of Developer Mode, so nothing reloads under it.

## A second screen

```lua
{ to = "app:window", msg = { open = "preview", display = "external", fullscreen = true } }
```

`display` is `"primary"`, `"external"` or an `id` from `app.displays`, and a
mod's `window` key takes the same. Each window's `ui.video` has its own thread
on the same ring. `app.window.display` is the screen the main window is on.

## Instruments

`histogram`, `waveform`, `vectorscope` and `audio-meter` draw what the engine
measured, at the rate it publishes, independently of the video. Keep them to
a few hundred columns and a few thousand points at 5 to 15 updates a second:
each publish runs Luau, and a provider line is capped at 1 MiB and 10,000
values. For a denser scope, render it in the engine and show it as a small
`ui.video`.

## Shipping

`PAYLOAD=DIR` puts a folder into the bundle as it is, and process providers
find it as `$MOONFRAME_PAYLOAD`: a Python, native libraries, the engine. It
stays out of the app's own folder, because a package ships source only. See
[stability](stability.md) for what may change.

```sh
PAYLOAD=build/nova-engine just bundle-app nova
NOTARY_PROFILE=nova scripts/sign-app.sh target/bundle/Nova.app \
    "Developer ID Application: Name (TEAMID)"
```

`sign-app.sh` signs every binary inside first, then the app, with the hardened
runtime. A Python that loads unsigned native modules may need
`com.apple.security.cs.disable-library-validation` in your entitlements.
