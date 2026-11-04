# Chinese word list

`rime/keyin-words.tsv` holds 146,615 simplified Chinese words that are missing
from `pinyin_simp.dict.yaml`, with toneless Pinyin readings and weights. It is
appended to the build copy of the Rime dictionary (macOS, Windows) and compiled
into the Rust table (iOS), so multi-syllable words such as 越南语 are offered as a
whole instead of being assembled from unrelated single-word picks.

Sources, retrieved 2026-11-04, both under the MIT license (`LICENSE.chinese-words`):

- **jieba** `jieba/dict.txt` (words and corpus counts), Copyright (c) 2013 Sun Junyi:
  https://github.com/fxsjy/jieba, revision `67fa2e36e72f69d9134b8a1037b83fbb070b9775`,
  SHA-256 `7197c3211ddd98962b036cdf40324d1ea2bfaa12bd028e68faa70111a88e12a8`.
- **phrase-pinyin-data** `large_pinyin.txt` (phrase readings), Copyright (c) 2017 mozillazg:
  https://github.com/mozillazg/phrase-pinyin-data, revision `4afa3226ce53c9138b939e83c3bb294dbe0da511`,
  SHA-256 `7e34c0e09ad398a7a07c0964bebb9677ace2114ed9d1c2ccdfce939102db2134`.

Selection: jieba words of two or more CJK characters that `pinyin_simp` does not
already contain. The reading comes from phrase-pinyin-data; without one, a word
with a jieba count of at least 5 is kept only when every character has a single
dominant (≥ 90% of its weight) reading in `pinyin_simp`. Every syllable must be a
`pinyin_simp` syllable. Weight is twice the jieba count, which puts it on roughly
the scale of `pinyin_simp`'s phrase weights. Existing `pinyin_simp` entries are
not changed.

Regenerate with
`python3 scripts/import-chinese-words.py /path/to/dict.txt /path/to/large_pinyin.txt`.
