# Vietnamese spelling data

`vietnamese-syllables.txt` contains 7,067 distinct whitespace-delimited components
extracted from the 31,327 headwords in VNTK's `data/dictionary.json`, revision
`2408c9d2c7e6908d166a773b60fd216492326d97` (retrieved November 1, 2026).

Source: https://github.com/vntk/dictionary
Pinned data: https://raw.githubusercontent.com/vntk/dictionary/2408c9d2c7e6908d166a773b60fd216492326d97/data/dictionary.json
Copyright (c) 2017 Vietnamese Natural Language Processing (VNTK).
Upstream MIT license is retained in `LICENSE.vntk`.

The extraction normalizes to NFC, lowercases, splits headwords on whitespace,
filters components to Vietnamese alphabet letters, deduplicates, and sorts.
Reproduce with `python3 scripts/import-vietnamese.py /path/to/dictionary.json`.
Definitions and examples are not included. This is broad spelling coverage, not
an exhaustive dictionary or a trained language model; names, loans, and rare
forms can be missing. Components extracted from loanwords can also be present.
