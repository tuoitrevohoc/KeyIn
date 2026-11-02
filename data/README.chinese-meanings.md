# Chinese candidate glosses

`chinese-meanings.json` is an adapted collection released under **Creative
Commons Attribution-ShareAlike 4.0 International (CC BY-SA 4.0)**.
Full license: `LICENSE.chinese-meanings`.
https://creativecommons.org/licenses/by-sa/4.0/

Sources retrieved 2026-11-02:

- **CC-CEDICT**, published by MDBG and community contributors:
  https://www.mdbg.net/chinese/dictionary?page=cc-cedict
  https://www.mdbg.net/chinese/export/cedict/cedict_1_0_ts_utf-8_mdbg.txt.gz
  Release timestamp: 2026-11-02T01:18:15Z; 125,356 source entries.
  Referenced work: CEDICT, Copyright (C) 1997, 1998 Paul Andrew Denisowski.
- **CVDICT**, Phong Phan:
  https://github.com/ph0ngp/CVDICT
  https://raw.githubusercontent.com/ph0ngp/CVDICT/main/CVDICT.u8
  Git blob SHA: 47bc5ae9b8acc821f6e4ddb5b318758e42d383c5.
  122,597 parsed entries (the upstream header reports 122,591).
  Derived from CC-CEDICT. Mostly translated by a fine-tuned ChatGPT-4o model
  with partial manual correction by its author. Translations, proper names,
  scientific names and phrasing may contain errors; glosses are a reference,
  not a guarantee of accuracy.

KeyIn adaptations: index by simplified headword, merge distinct definitions for
multiple pronunciations, replace slash separators with semicolons, normalize
one malformed CVDICT Pinyin bracket, and prefer the short KeyIn glosses in
`chinese-meanings-overrides.json`. The KeyIn-authored overrides are also offered
under CC BY-SA 4.0 for this dictionary collection. Traditional-only spellings
are not added as extra headwords. The UI can annotate unmatched phrases with separate word/character glosses;
these are not synthesized sentence translations. Some headwords have only one language available.

Result: 125,353 distinct headwords, 119,046 with Vietnamese glosses and 121,593
with English glosses. See `chinese-meanings-manifest.json` for source SHA-256
checksums and counts. These data licenses do not change the application code's
LGPL-3.0-or-later license.

To regenerate after downloading the two source files locally:

```sh
python3 scripts/import-chinese-meanings.py /path/to/cedict.txt.gz /path/to/CVDICT.u8
```

The importer requires no network or additional packages. Review changes and
update this attribution record when importing a different source revision.
