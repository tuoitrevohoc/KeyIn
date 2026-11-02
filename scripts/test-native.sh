#!/bin/bash
set -euo pipefail
cd "$(dirname "$0")/.."
rime_prefix="$(brew --prefix librime)"
frameworks="$PWD/build/KeyIn.app/Contents/Frameworks"
shared="$PWD/build/KeyIn.app/Contents/Resources/rime"
# Link tests against the shipped libraries, not the Homebrew dylib graph.
xcrun clang -fobjc-arc -Wall -Wextra -I"$rime_prefix/include" -Inative native/KeyInRime.m tests/rime_smoke.m "$frameworks/librime.1.dylib" -Wl,-rpath,"$frameworks" -framework Foundation -o build/rime-smoke
xcrun clang -fobjc-arc -Wall -Wextra -Wno-unused-parameter -I"$rime_prefix/include" -Inative tests/controller_smoke.m native/KeyInRime.m target/release/libkeyin.a "$frameworks/librime.1.dylib" -Wl,-rpath,"$frameworks" -framework Cocoa -framework QuartzCore -framework InputMethodKit -framework Carbon -framework Security -framework SystemConfiguration -o build/controller-smoke
rime_test_dir=$(mktemp -d /tmp/keyin-rime-test.XXXXXX)
./build/rime-smoke "$shared" "$rime_test_dir"
# Reopen the same user dictionary in a new process.
./build/rime-smoke "$shared" "$rime_test_dir"
./build/controller-smoke "$shared" "$(mktemp -d /tmp/keyin-controller-test.XXXXXX)"
codesign --verify --deep --strict build/KeyIn.app
python3 - <<'PY'
from pathlib import Path
import subprocess
app = Path('build/KeyIn.app/Contents')
for binary in [app/'MacOS/KeyIn', *(app/'Frameworks').glob('*.dylib')]:
    deps = subprocess.check_output(['otool', '-L', str(binary)], text=True)
    assert '/opt/homebrew/' not in deps and '/usr/local/' not in deps, deps
print('Bundle dependency isolation and signature checks passed')
PY
