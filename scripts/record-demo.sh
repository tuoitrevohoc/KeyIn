#!/bin/bash
set -euo pipefail
cd "$(dirname "$0")/.."
# Requires a completed scripts/build.sh and ffmpeg (brew install ffmpeg).
frameworks="$PWD/build/KeyIn.app/Contents/Frameworks"
work=$(mktemp -d /tmp/keyin-demo.XXXXXX)
mkdir -p "$work/frames" "$work/user" docs/media
xcrun clang -fobjc-arc -Wno-unused-parameter -I"$(brew --prefix librime)/include" -Inative scripts/demo.m native/KeyInRime.m target/release/libkeyin.a "$frameworks/librime.1.dylib" -Wl,-rpath,"$frameworks" -framework Cocoa -framework QuartzCore -framework InputMethodKit -framework Carbon -framework Security -framework SystemConfiguration -o "$work/demo"
"$work/demo" "$PWD/build/KeyIn.app/Contents/Resources/rime" "$work/user" "$work/frames"
# Full-size frames only: offset sub-frames rendered as broken fragments in some viewers.
ffmpeg -hide_banner -loglevel error -y -framerate 10 -i "$work/frames/%04d.png" -filter_complex '[0:v]format=rgb24,split[a][b];[a]palettegen=stats_mode=full[p];[b][p]paletteuse=dither=bayer:bayer_scale=4:diff_mode=rectangle' -gifflags -offsetting -loop 0 docs/media/keyin-demo.gif
ffmpeg -hide_banner -loglevel error -y -framerate 10 -i "$work/frames/%04d.png" -c:v libx264 -pix_fmt yuv420p -crf 20 -movflags +faststart "$work/keyin-demo.mp4"
cp "$work/frames/settings.png" docs/media/keyin-settings.png
printf 'Demo written to docs/media/keyin-demo.gif and keyin-settings.png\nVideo: %s/keyin-demo.mp4\nFrames: %s/frames\n' "$work" "$work"
