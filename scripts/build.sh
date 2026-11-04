#!/bin/bash
set -euo pipefail
cd "$(dirname "$0")/.."
rime_prefix="$(brew --prefix librime)"
export MACOSX_DEPLOYMENT_TARGET="$(otool -l "$rime_prefix/lib/librime.dylib" | awk '/minos / { print $2; exit }')"
: "${MACOSX_DEPLOYMENT_TARGET:=12.0}"
cargo build --release
mkdir -p build/rime-user build/rime-data/build
cp data/rime/*.yaml data/pinyin_simp.dict.yaml build/rime-data/
cat data/rime/keyin-phrases.tsv data/rime/keyin-words.tsv >> build/rime-data/pinyin_simp.dict.yaml
"$rime_prefix/bin/rime_deployer" --build "$PWD/build/rime-user" "$PWD/build/rime-data" "$PWD/build/rime-data/build"
app="$PWD/build/KeyIn.app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
cp data/LICENSE.chinese-meanings data/README.chinese-meanings.md data/chinese-meanings-manifest.json "$app/Contents/Resources/"
cp data/chinese-meanings.json "$app/Contents/Resources/"
cp native/Info.plist "$app/Contents/Info.plist"
cp native/Icons/*.pdf "$app/Contents/Resources/"
cp data/LICENSE.rime data/COPYING.GPL3 data/LICENSE.vntk data/LICENSE.pinyin-simp data/LICENSE.chinese-words data/README.chinese-words.md data/README.vietnamese.md README.md "$app/Contents/Resources/"
mkdir -p "$app/Contents/Resources/rime"
ditto build/rime-data "$app/Contents/Resources/rime"
xcrun clang -fobjc-arc -O2 -Wall -Wextra -Wno-unused-parameter -mmacosx-version-min="$MACOSX_DEPLOYMENT_TARGET" native/main.m native/KeyInRime.m -I"$rime_prefix/include" -L"$rime_prefix/lib" -lrime target/release/libkeyin.a -framework Cocoa -framework QuartzCore -framework InputMethodKit -framework Carbon -framework Security -framework SystemConfiguration -o "$app/Contents/MacOS/KeyIn"
cp -R native/en.lproj native/vi.lproj "$app/Contents/Resources/"
xcrun clang -fobjc-arc -Wall -Wextra native/register.m -framework Foundation -framework Carbon -o build/register-keyin
python3 scripts/bundle-rime.py "$app"
codesign --force --sign - "$app"
plutil -lint "$app/Contents/Info.plist"
echo "Built $app"
