#!/usr/bin/env bash
# Renders the showcase film on moonframe.dev into website/site/film/.
# Needs node, rsvg-convert (librsvg), ffmpeg with libx264,
# and cwebp. Frames go to $FILM_WORK (default target/film), about 2 GB.
set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
repo="$(cd "$here/../../.." && pwd)"
work="${FILM_WORK:-$repo/target/film}"
site="$repo/website/site/film"
jobs="${JOBS:-$(getconf _NPROCESSORS_ONLN)}"
export FILM_WORK="$work"

for tool in node rsvg-convert ffmpeg cwebp; do
	command -v "$tool" >/dev/null || { echo "render.sh: $tool is missing" >&2; exit 1; }
done

rm -rf "$work/frames" "$work/out"
mkdir -p "$work/frames/img" "$work/out" "$site"

# The screenshots the frames embed, scaled down so each frame renders quickly.
shots="$repo/docs/screenshots"
scale() { ffmpeg -loglevel error -y -i "$1" -vf "scale='min($3,iw)':-2" "$2"; }
for name in window dark components; do
	scale "$shots/$name.png" "$work/frames/img/$name.png" 1800
done
for shot in "$shots"/apps/*.png; do
	scale "$shot" "$work/frames/img/app-$(basename "$shot")" 1400
done
scale "$repo/assets/icon.png" "$work/frames/img/icon.png" 512

node "$here/gen.mjs"
# The inner sh expands $1, one frame per call, so the single quotes are meant.
# shellcheck disable=SC2016
(cd "$work/frames" && find . -maxdepth 1 -name 'f*.svg' -print0 |
	xargs -0 -P "$jobs" -I{} sh -c 'rsvg-convert "$1" -o "../out/$(basename "$1" .svg).png"' _ {})

frames="$work/out/f%04d.png"
# The film, played with controls. H.264 plays everywhere, and on these flat
# frames VP9 came out larger, so there is no WebM.
ffmpeg -loglevel error -y -framerate 30 -i "$frames" -c:v libx264 -preset slow -crf 24 \
	-pix_fmt yuv420p -movflags +faststart "$site/film.mp4"
# Its poster: the moment the report's best month turns orange.
cwebp -quiet -q 82 -resize 1600 0 "$work/out/f0605.png" -o "$site/film-poster.webp"

# The muted loop beside "How you work": the first two scenes (4.2s to 21.4s),
# fading in and out so the cut is not seen. The crop drops the progress line.
fades="crop=1920:1072:0:0,scale=1280:-2,fade=in:st=0:d=0.3:color=0xf2e8d4,fade=out:st=16.85:d=0.35:color=0xf2e8d4"
ffmpeg -loglevel error -y -framerate 30 -start_number 126 -i "$frames" -frames:v 516 \
	-vf "$fades" -c:v libx264 -preset slow -crf 26 -pix_fmt yuv420p -movflags +faststart -an "$site/loop.mp4"
cwebp -quiet -q 80 -resize 1280 0 "$work/out/f0300.png" -o "$site/loop-poster.webp"

ls -la "$site"
