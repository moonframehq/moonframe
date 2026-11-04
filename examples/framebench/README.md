# Framebench

The smallest honest test of live video on Moonframe: a stand-in capture
engine with no interface, and an app that shows its frames and measures what
reaches the screen. Use it to decide whether Moonframe suits a machine and a
frame size before building anything.

```sh
python3 examples/framebench/engine/engine.py --size 2048x1536
moonframe app link examples/framebench
moonframe --profile framebench
```

Pick 18, 25 or 50 i/s and press Start. **Shown** is what reached the screen,
**Skipped** what the viewer dropped to stay current, **Age on screen** how old
a frame is when it is handed to the screen (the engine stamps capture time).
**Engine late** counts frames the engine itself couldn't write on time, which
is the engine's limit and not the viewer's.

It shows the shape to copy:

- The engine runs and tests without the app, and keeps running across a quit,
  a reload or a crash: start the app again and it finds the real state. Kill
  the engine and restart it: the link comes back by itself.
- Frames go through shared memory (`engine/ring.py`), commands and state over
  a socket as JSON lines (`providers/engine/provider.toml`).
- `Stop` is `urgent`, the gain is `latest`, readings are `publish = "latest"`.
- The buttons never assume: what shows comes from what the engine says.

Requires macOS and the `video` feature, which the starter has.
