#!/bin/bash
set -euo pipefail
cd "$(dirname "$0")/.."
# Requires a completed scripts/build.sh and ffmpeg (brew install ffmpeg).
frameworks="$PWD/build/KeyIn.app/Contents/Frameworks"
work=$(mktemp -d /tmp/keyin-demo.XXXXXX)
mkdir -p "$work/frames" "$work/user" docs/media
xcrun clang -fobjc-arc -Wno-unused-parameter -I"$(brew --prefix librime)/include" -Inative scripts/demo.m native/KeyInRime.m target/release/libkeyin.a "$frameworks/librime.1.dylib" -Wl,-rpath,"$frameworks" -framework Cocoa -framework QuartzCore -framework InputMethodKit -framework Carbon -framework Security -framework SystemConfiguration -o "$work/demo"
"$work/demo" "$PWD/build/KeyIn.app/Contents/Resources/rime" "$work/user" "$work/frames"
ffmpeg -hide_banner -loglevel error -y -framerate 6 -i "$work/frames/%04d.png" -filter_complex '[0:v]split[a][b];[a]palettegen[p];[b][p]paletteuse' -loop 0 docs/media/keyin-demo.gif
cp "$work/frames/settings.png" docs/media/keyin-settings.png
printf 'Demo written to docs/media/keyin-demo.gif and keyin-settings.png\nFrames: %s/frames\n' "$work"
