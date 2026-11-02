#!/usr/bin/env python3
"""Merge local CC-CEDICT and CVDICT files into the offline gloss dictionary.

Usage: python3 scripts/import-chinese-meanings.py cedict.txt.gz CVDICT.u8
See data/README.chinese-meanings.md for sources and licensing.
"""
import gzip
import hashlib
import json
from pathlib import Path
import re
import sys

ROOT = Path(__file__).resolve().parent.parent
ENTRY = re.compile(r'^(\S+)\s+(\S+)\s+\[[^\]]*\]\s+/(.*)/$')


def read_entries(path):
    raw = path.read_bytes()
    content = gzip.decompress(raw) if path.suffix == '.gz' else raw
    for number, line in enumerate(content.decode('utf-8-sig').splitlines(), 1):
        line = line.strip()
        if not line or line.startswith('#'):
            continue
        # CVDICT has one published entry with doubled Pinyin brackets.
        line = line.replace('[[Ao4 zhou1 Guang3 bo1 Dian4 tai2] ]', '[Ao4 zhou1 Guang3 bo1 Dian4 tai2]')
        match = ENTRY.fullmatch(line)
        if not match:
            raise ValueError(f'{path}:{number}: invalid dictionary entry')
        traditional, simplified, definition = match.groups()
        senses = list(dict.fromkeys(s.strip() for s in definition.split('/') if s.strip()))
        if not senses:
            raise ValueError(f'{path}:{number}: empty definition')
        yield simplified, '; '.join(senses)


def main():
    if len(sys.argv) != 3:
        raise SystemExit(__doc__)
    merged = {}
    sources = {}
    for language, filename in zip(('en', 'vi'), sys.argv[1:]):
        path = Path(filename)
        buckets = {}
        rows = 0
        for word, gloss in read_entries(path):
            rows += 1
            glosses = buckets.setdefault(word, [])
            if gloss not in glosses:
                glosses.append(gloss)
        for word, glosses in buckets.items():
            merged.setdefault(word, {})[language] = '; '.join(glosses)
        sources[language] = {'sha256': hashlib.sha256(path.read_bytes()).hexdigest(),
                             'source_entries': rows, 'unique_simplified_words': len(buckets)}
    overrides = json.loads((ROOT / 'data/chinese-meanings-overrides.json').read_text())
    for word, glosses in overrides.items():
        merged.setdefault(word, {}).update(glosses)
    (ROOT / 'data/chinese-meanings.json').write_text(
        json.dumps(merged, ensure_ascii=False, sort_keys=True, indent=2) + '\n')
    sources['output'] = {'words': len(merged), **{
        language: sum(bool(entry.get(language)) for entry in merged.values())
        for language in ('vi', 'en')}}
    (ROOT / 'data/chinese-meanings-manifest.json').write_text(json.dumps(sources, indent=2) + '\n')
    print(json.dumps(sources, indent=2))


if __name__ == '__main__':
    main()
