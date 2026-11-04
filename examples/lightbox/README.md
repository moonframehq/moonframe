# Lightbox

A pretend film scanner, to see Moonframe's live pieces working together. Its
engine is a Python process the app starts, needing only Python 3. It makes the
film up, and measures the same picture it shows, so the instruments are real.

```sh
moonframe app link examples/lightbox
moonframe --profile lightbox
```

Requires the `video` and `awake` features, which the starter has, and macOS
for the picture. Press **Start**, then try:

| Do this | To see |
| --- | --- |
| Drag **Exposure** | `waveform`, `vectorscope` and `histogram` follow the picture, as one `latest` message |
| Wheel, pinch or double-click the picture | `video-viewer`'s zoom and pan |
| **Crash engine** during a scan | The engine dies, `restart = "on-failure"` brings it back, and the scan carries on from its saved place |
| **Restart engine** | `app:restart`: the old process leaves before the new one starts |
| Quit during a scan | `app:guard` holds it back and asks; **Stop and close** stops the engine, then quits |
| Hide the window | `app:dock` shows the scan's progress on the Dock icon |
| `pmset -g assertions` during a scan | `app:awake` holding the machine, as "Lightbox is scanning" |
| The cup, between scans | The `caffeine` component, a hold of its own; it waits out a scan greyed out |
| Let it finish | `app:notify` and `app:sound` |

The scan's place lives in the system's temporary folder, so that writing it
doesn't count as a change to the provider, which a reload would restart.
