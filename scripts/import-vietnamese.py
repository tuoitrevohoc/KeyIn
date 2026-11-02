#!/usr/bin/env python3
"""Extract normalized Vietnamese syllables from a local VNTK dictionary.json.

Source: vntk/dictionary, revision 2408c9d2c7e6908d166a773b60fd216492326d97
Usage: python3 scripts/import-vietnamese.py /path/to/dictionary.json
See data/README.vietnamese.md for attribution and the source URL.
"""
import json
from pathlib import Path
import sys
import unicodedata

words = json.loads(Path(sys.argv[1]).read_text())
letters = set(
    'aăâbcdđeêghiklmnoôơpqrstuưvxy'
    'áàảãạắằẳẵặấầẩẫậéèẻẽẹếềểễệíìỉĩị'
    'óòỏõọốồổỗộớờởỡợúùủũụứừửữựýỳỷỹỵ'
)
syllables = sorted({
    syllable
    for word in words
    for syllable in unicodedata.normalize('NFC', word).lower().split()
    if set(syllable) <= letters
})
output = Path(__file__).resolve().parent.parent / 'data/vietnamese-syllables.txt'
output.write_text('\n'.join(syllables) + '\n')
print(f'{len(syllables)} entries written to {output}')
