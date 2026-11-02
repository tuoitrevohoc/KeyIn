#!/usr/bin/env python3
"""Copy the complete non-system Mach-O dependency graph into the app.
Retain Homebrew formula licenses/receipts and raise the bundle's minimum macOS
version to the actual maximum required by its packaged binaries.
"""
from pathlib import Path
import json
import plistlib
import re
import shutil
import subprocess
import sys

app = Path(sys.argv[1]).resolve()
frameworks = app / 'Contents/Frameworks'
licenses = app / 'Contents/Resources/ThirdParty'
frameworks.mkdir(parents=True, exist_ok=True)
licenses.mkdir(parents=True, exist_ok=True)
binary = app / 'Contents/MacOS/KeyIn'
visited = {}
minimum = (12, 0)

def run(*args):
    return subprocess.check_output(args, text=True)

def dependencies(path):
    return [line.strip().split(' (')[0] for line in run('otool', '-L', str(path)).splitlines()[1:]]

def visit(source):
    global minimum
    source = Path(source)
    canonical = source.resolve()
    if canonical in visited:
        return visited[canonical]
    name = source.name
    target = frameworks / name
    if target.exists():
        target.unlink()
    shutil.copy2(canonical, target)
    target.chmod(0o755)
    visited[canonical] = target
    # Keep license files next to an exact installed dependency receipt.
    parts = canonical.parts
    if 'Cellar' in parts:
        root = Path(*parts[:parts.index('Cellar') + 3])
        dest = licenses / (root.parent.name + '-' + root.name)
        dest.mkdir(exist_ok=True)
        for file in root.rglob('*'):
            if file.is_file() and (file.name.lower().startswith(('license', 'copying', 'copyright', 'notice')) or file.name == 'INSTALL_RECEIPT.json'):
                relative = file.relative_to(root)
                (dest / relative).parent.mkdir(parents=True, exist_ok=True)
                shutil.copy2(file, dest / relative)
    commands = run('otool', '-l', str(target))
    for version in re.findall(r'\bminos (\d+\.\d+(?:\.\d+)?)', commands):
        minimum = max(minimum, tuple(map(int, version.split('.'))))
    for dep in dependencies(canonical):
        if Path(dep).exists() and Path(dep).resolve() == canonical:
            continue
        if dep.startswith(('/opt/homebrew/', '/usr/local/')):
            child = visit(dep)
            subprocess.run(['install_name_tool', '-change', dep, '@loader_path/' + child.name, str(target)], check=True, capture_output=True)
        elif not dep.startswith(('/usr/lib/', '/System/Library/')):
            raise RuntimeError(f'Unresolved library {dep} from {source}')
    subprocess.run(['install_name_tool', '-id', '@rpath/' + target.name, str(target)], check=True, capture_output=True)
    subprocess.run(['codesign', '--force', '--sign', '-', str(target)], check=True, capture_output=True)
    return target

for dep in dependencies(binary):
    if dep.startswith(('/opt/homebrew/', '/usr/local/')):
        target = visit(dep)
        subprocess.run(['install_name_tool', '-change', dep, '@executable_path/../Frameworks/' + target.name, str(binary)], check=True, capture_output=True)

plist_path = app / 'Contents/Info.plist'
with plist_path.open('rb') as file:
    plist = plistlib.load(file)
plist['LSMinimumSystemVersion'] = '.'.join(map(str, minimum))
with plist_path.open('wb') as file:
    plistlib.dump(plist, file)
manifest = {str(source): target.name for source, target in visited.items()}
(licenses / 'rime-libraries.json').write_text(json.dumps(manifest, indent=2) + '\n')
print(f'Bundled {len(visited)} libraries; minimum macOS {plist["LSMinimumSystemVersion"]}')
