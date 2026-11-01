# Gantry

A dark, monospace console for an infra platform: the services running on
it, and for each one its resources, usage over the day and alerts. Square
corners, thin rules, colour only where it means something.

```sh
moonframe app link examples/gantry --yes
moonframe --profile gantry
```

- **Services**: the fleet's figures, filters by state, a search field, and
  a row per service with tick meters for CPU, GPU and memory. Click one to
  open it.
- **A service**: tags, Live logs (a sheet of the latest lines) and New
  deploy (it rolls out over a few samples). Four tabs:
  - *Resource*: CPU, GPU and memory allocated and used, as tick meters;
    Configure turns the limits into − and + that change the allocation.
    Usage history: hover the plot for a sample's values, click to pin the
    crosshair; 6h, 12h or 24h. Alerts toggle, fire when their threshold is
    crossed, and Add alert adds presets (right-click one to remove it).
  - *Overview*: requests, latency, errors, replicas, deploys, details and
    recent logs. *Analytics*: requests and p95 latency over the range.
    *Settings*: autoscaling, replicas, deploy policy.
- **Home, Tasks, Docs, Status, Changelog** in the top bar, in the same
  style; Go in the menu bar has them on ⌘1 to ⌘6.

The data is **simulated sample telemetry** from
`providers/telemetry/telemetry.pl` (perl 5, as macOS ships): a new sample
every 3 seconds stands for 15 minutes, so the meters and the chart move.
Replace that script to read a real platform; its header documents the shape
of `app.telemetry`.

It uses the shared `tick-meter` and `line-chart` components, and three of
its own in `components/` (`frame-box`, `square-button`, `tag`). The face is
IBM Plex Mono as the Nerd Font build `BlexMono Nerd Font Mono`
(`brew install --cask font-blex-mono-nerd-font`); without it the text falls
back to another font.
