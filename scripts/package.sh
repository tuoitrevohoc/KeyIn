#!/bin/bash
set -euo pipefail
cd "$(dirname "$0")/.."
app="$PWD/build/KeyIn.app"
[[ -d "$app" ]] || { echo 'Run ./scripts/build.sh first.' >&2; exit 1; }
version=$(/usr/libexec/PlistBuddy -c 'Print :CFBundleShortVersionString' "$app/Contents/Info.plist")
minimum=$(/usr/libexec/PlistBuddy -c 'Print :LSMinimumSystemVersion' "$app/Contents/Info.plist")
out="$PWD/build/installer/$version"
mkdir -p "$out"
staging=$(mktemp -d /tmp/keyin-package.XXXXXX)
trap 'rm -rf "$staging"' EXIT
mkdir -p "$staging/root/Library/Input Methods" "$staging/resources"
ditto "$app" "$staging/root/Library/Input Methods/KeyIn.app"
staged_app="$staging/root/Library/Input Methods/KeyIn.app"
if [[ -n "${KEYIN_APP_IDENTITY:-}" ]]; then
    : "${KEYIN_INSTALLER_IDENTITY:?Set the Developer ID Installer identity too}"
    for library in "$staged_app"/Contents/Frameworks/*.dylib; do
        codesign --force --timestamp --options runtime --sign "$KEYIN_APP_IDENTITY" "$library"
    done
    codesign --force --timestamp --options runtime --sign "$KEYIN_APP_IDENTITY" "$staged_app"
fi
codesign --verify --deep --strict "$staged_app"
# Installer text in English and Vietnamese; Installer shows the one matching the system language.
mkdir -p "$staging/resources/en.lproj" "$staging/resources/vi.lproj"
cat > "$staging/resources/en.lproj/welcome.txt" <<TEXT
KeyIn $version — experimental preview
Requires Apple Silicon and macOS $minimum or later.

Before installing: switch to ABC and quit KeyIn in Activity Monitor.
This installer installs KeyIn for all users. If you previously installed KeyIn in ~/Library/Input Methods, move that app
to a backup folder first to avoid duplicate input methods.
No typing history or preferences are removed.
TEXT
cat > "$staging/resources/en.lproj/conclusion.txt" <<'TEXT'
Installation complete.
1. Log out of macOS and log back in.
2. Open System Settings > Keyboard > Text Input > Edit > +.
3. Find KeyIn under Vietnamese, add it, and select KeyIn in the input menu.

Languages and options: KeyIn menu > Settings…
To uninstall, switch to ABC, quit KeyIn, remove it from Keyboard settings,
and remove /Library/Input Methods/KeyIn.app.
TEXT
cat > "$staging/resources/vi.lproj/welcome.txt" <<TEXT
KeyIn $version — bản thử nghiệm
Cần máy Apple Silicon và macOS $minimum trở lên.

Trước khi cài: chuyển sang bàn phím ABC và thoát KeyIn trong Activity Monitor.
Bộ cài này cài KeyIn cho mọi người dùng. Nếu trước đây bạn đã cài KeyIn trong ~/Library/Input Methods, hãy chuyển app đó
sang thư mục khác trước để không bị trùng bộ gõ.
Lịch sử gõ và tùy chỉnh được giữ nguyên.
TEXT
cat > "$staging/resources/vi.lproj/conclusion.txt" <<'TEXT'
Cài đặt xong.
1. Đăng xuất khỏi macOS rồi đăng nhập lại.
2. Mở Cài đặt hệ thống > Bàn phím > Nguồn đầu vào > Sửa > +.
3. Tìm KeyIn trong mục Tiếng Việt, thêm vào và chọn KeyIn trên menu bàn phím.

Ngôn ngữ và tùy chọn: menu KeyIn > Tùy chỉnh…
Để gỡ cài đặt, chuyển sang ABC, thoát KeyIn, xóa KeyIn khỏi cài đặt Bàn phím,
rồi xóa /Library/Input Methods/KeyIn.app.
TEXT
pkgbuild --analyze --root "$staging/root" "$staging/components.plist"
python3 - "$staging/components.plist" <<'PYCOMP'
import plistlib, sys
from pathlib import Path
p = Path(sys.argv[1])
items = plistlib.loads(p.read_bytes())
for item in items:
    item['BundleIsRelocatable'] = False
p.write_bytes(plistlib.dumps(items))
PYCOMP
pkgbuild --component-plist "$staging/components.plist" --root "$staging/root" --scripts scripts/installer --identifier org.keyin.inputmethod.installer --version "$version" --install-location / --ownership recommended "$staging/component.pkg"
python3 - "$staging" "$minimum" <<'PY'
import sys
from pathlib import Path
from xml.sax.saxutils import escape
folder=Path(sys.argv[1])
(folder/'distribution.xml').write_text(f'''<?xml version="1.0" encoding="utf-8"?>
<installer-gui-script minSpecVersion="2">
<title>KeyIn</title>
<welcome file="welcome.txt" mime-type="text/plain"/>
<conclusion file="conclusion.txt" mime-type="text/plain"/>
<options customize="never" require-scripts="false" hostArchitectures="arm64"/>
<domains enable_localSystem="true" enable_currentUserHome="false" enable_anywhere="false"/>
<allowed-os-versions><os-version min="{escape(sys.argv[2])}"/></allowed-os-versions>
<choices-outline><line choice="default"/></choices-outline>
<choice id="default" visible="false"><pkg-ref id="org.keyin.inputmethod.installer"/></choice>
<pkg-ref id="org.keyin.inputmethod.installer">component.pkg</pkg-ref>
</installer-gui-script>
''')
PY
sign_args=()
if [[ -n "${KEYIN_INSTALLER_IDENTITY:-}" ]]; then
    : "${KEYIN_APP_IDENTITY:?Set the Developer ID Application identity too}"
    sign_args=(--sign "$KEYIN_INSTALLER_IDENTITY" --timestamp)
fi
package="$out/KeyIn-$version-macos-arm64.pkg"
productbuild --distribution "$staging/distribution.xml" --resources "$staging/resources" --package-path "$staging" ${sign_args[@]+"${sign_args[@]}"} "$package"
if [[ -n "${KEYIN_NOTARY_PROFILE:-}" ]]; then
    : "${KEYIN_APP_IDENTITY:?Notarization requires Developer ID signing}"
    xcrun notarytool submit "$package" --keychain-profile "$KEYIN_NOTARY_PROFILE" --wait
    xcrun stapler staple "$package"
    xcrun stapler validate "$package"
    spctl --assess --type install --verbose "$package"
fi
(cd "$out" && shasum -a 256 "$(basename "$package")" > SHA256SUMS.txt)
echo "Built $package"
