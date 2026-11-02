#!/bin/bash
set -euo pipefail
cd "$(dirname "$0")/.."
./scripts/build.sh
install_dir="$HOME/Library/Input Methods"
mkdir -p "$install_dir"
if [[ -e "$install_dir/KeyIn.app" ]]; then
    echo "KeyIn already exists. Switch to ABC and move the previous KeyIn.app aside before reinstalling."
    exit 1
fi
ditto build/KeyIn.app "$install_dir/KeyIn.app"
/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister -f "$install_dir/KeyIn.app"
./build/register-keyin "$install_dir/KeyIn.app"
echo "Installed and enabled. Select KeyIn in the input menu; reopen System Settings if its list is stale."
echo "For ⌘Space, disable/reassign Spotlight's shortcut in Keyboard > Keyboard Shortcuts > Spotlight."
