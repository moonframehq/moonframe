# Rune screenshots

`site/img/full/rune-emberline.png` and `rune-audience.png` are the output of
Rune's headless GPU renderer, not browser mockups or desktop captures.
They use the unchanged example apps with their bundled sample data.
Captured on 2026-11-10, on macOS with an Apple M1 Ultra, from Moonframe
`f6f468029bdca1924b6b300480a62abbf22bdb9b` and its locked Rune dependency
`a8ca769b944598e179f33cc6843629b90e6ca831`.

This session had no macOS Screen Recording permission. These captures show
the app content, without the operating system's window chrome. They do not
establish feature parity with GPUI.

## Reproduce

From the repository root, with `mise install` complete:

```sh
cargo build --locked --release -p moonframe-app --no-default-features --features rune
user=$(mktemp -d "${TMPDIR:-/tmp}/moonframe-rune-shots.XXXXXX")
export XDG_STATE_HOME="$user/state"
bin=target/release/moonframe

"$bin" --user "$user" app link examples/emberline
"$bin" --user "$user" app link examples/audience
```

The link commands ask to allow each example's bundled Send feedback
provider. Approve it to use the unmodified apps; in a non-interactive
session, add `--yes` only with the user's permission. No agent is started
by capturing a frame.

```sh
"$bin" --user "$user" --profile emberline --check
"$bin" --user "$user" --profile audience --check

MOONFRAME_CAPTURE_DELAY=1500 "$bin" --user "$user" --profile emberline \
  --snap website/site/img/full/rune-emberline.png \
  --theme dark --size 1480x1040 --scale 2 --at 1
MOONFRAME_CAPTURE_DELAY=1500 "$bin" --user "$user" --profile audience \
  --snap website/site/img/full/rune-audience.png \
  --theme light --size 1600x1180 --scale 2 --at 1
```

The delay gives the static providers time to publish. The PNGs keep the
whole viewport at 2×. The WebP thumbnails used by the site and README are
resized proportionally, without cropping or retouching:

```sh
uv run --with pillow python - <<'PY'
from PIL import Image

for name in ("emberline", "audience"):
    image = Image.open(f"website/site/img/full/rune-{name}.png")
    image.thumbnail((1600, 1600))
    image.save(f"website/site/img/rune-{name}.webp", quality=88, method=6)
PY
```

To use the apps in a real window instead: `just rune-app emberline` and
`just rune-app audience`.
