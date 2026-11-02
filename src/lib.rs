//! Offline composition engine. No network, telemetry, or per-keystroke disk access.
use std::{
    collections::{HashMap, HashSet},
    ffi::{c_char, CStr, CString},
    sync::OnceLock,
};

fn spelling_key(text: &str) -> String {
    // Treat hòa/hoà and thủy/thuỷ alike without discarding the tone itself.
    let mut tone = 0;
    let mut key = String::new();
    for c in text.to_lowercase().chars() {
        if let Some(row) = VOWELS.iter().find(|row| row.contains(c)) {
            let index = row.chars().position(|v| v == c).unwrap();
            if index != 0 {
                tone = index;
            }
        }
        key.push(base(c));
    }
    key.push(char::from_digit(tone as u32, 10).unwrap());
    key
}

fn vietnamese_syllables() -> &'static HashSet<String> {
    static WORDS: OnceLock<HashSet<String>> = OnceLock::new();
    WORDS.get_or_init(|| {
        include_str!("../data/vietnamese-syllables.txt")
            .lines()
            .map(spelling_key)
            .collect()
    })
}

fn valid_vietnamese(text: &str) -> bool {
    vietnamese_syllables().contains(&spelling_key(text))
}

fn dictionary() -> &'static HashMap<String, Vec<String>> {
    static DICT: OnceLock<HashMap<String, Vec<String>>> = OnceLock::new();
    DICT.get_or_init(|| {
        let mut map: HashMap<String, Vec<String>> = HashMap::new();
        // Frequent candidates precede the source dictionary's lexical ordering.
        for (key, values) in [
            ("ni", "你 妳 尼"),
            ("hao", "好 號"),
            ("nihao", "你好"),
            ("zhongguo", "中國"),
            ("wo", "我"),
            ("shi", "是 時 事"),
            ("de", "的 得 地"),
            ("le", "了 乐"),
            ("ma", "吗 嘛 马"),
            ("xiexie", "謝謝"),
            ("zaijian", "再見"),
            ("shijie", "世界"),
            ("zhongwen", "中文"),
        ] {
            map.insert(
                key.into(),
                values.split_whitespace().map(str::to_owned).collect(),
            );
        }
        // This source contains phrase frequencies. Sort before merging: source
        // file order is not candidate preference (rare glyphs often come first).
        let mut weighted: HashMap<String, Vec<(&str, u64)>> = HashMap::new();
        for line in include_str!("../data/pinyin_simp.dict.yaml").lines() {
            let mut fields = line.split('\t');
            if let (Some(word), Some(reading), Some(weight)) =
                (fields.next(), fields.next(), fields.next())
            {
                let key = reading.replace(' ', "");
                if key.is_empty() || !key.bytes().all(|b| b.is_ascii_lowercase()) {
                    continue;
                }
                if let Ok(weight) = weight.parse::<u64>() {
                    weighted.entry(key).or_default().push((word, weight));
                }
            }
        }
        for (key, mut entries) in weighted {
            entries.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(b.0)));
            let values = map.entry(key).or_default();
            for (word, _) in entries {
                if !values.iter().any(|v| v == word) {
                    values.push(word.to_owned());
                }
            }
        }
        for line in include_str!("../data/luna_pinyin.dict.yaml").lines() {
            let mut fields = line.split('\t');
            if let (Some(word), Some(reading)) = (fields.next(), fields.next()) {
                let key = reading.replace(' ', "");
                if !key.chars().all(|c| c.is_ascii_lowercase()) {
                    continue;
                }
                let values = map.entry(key).or_default();
                if !values.iter().any(|v| v == word) {
                    values.push(word.to_owned());
                }
            }
        }
        map
    })
}

const VOWELS: [&str; 12] = [
    "aáàảãạ",
    "ăắằẳẵặ",
    "âấầẩẫậ",
    "eéèẻẽẹ",
    "êếềểễệ",
    "iíìỉĩị",
    "oóòỏõọ",
    "ôốồổỗộ",
    "ơớờởỡợ",
    "uúùủũụ",
    "ưứừửữự",
    "yýỳỷỹỵ",
];
fn base(c: char) -> char {
    let c = c.to_lowercase().next().unwrap();
    VOWELS
        .iter()
        .find(|s| s.contains(c))
        .map_or(c, |s| s.chars().next().unwrap())
}
fn case(c: char, original: char) -> char {
    if original.is_uppercase() {
        c.to_uppercase().next().unwrap()
    } else {
        c
    }
}

pub fn telex(raw: &str) -> String {
    let mut out: Vec<char> = Vec::new();
    let mut tone = 0;
    let mut last_tone = None;
    let mut shortcut_w = false;
    for ch in raw.chars() {
        let previous_shortcut_w = shortcut_w;
        shortcut_w = false;
        let c = ch.to_ascii_lowercase();
        if let Some(t) = "sfrxj".find(c) {
            if out
                .iter()
                .any(|c| VOWELS.iter().any(|s| s.contains(base(*c))))
            {
                if last_tone == Some(c) {
                    tone = 0;
                    last_tone = None;
                    out.push(ch);
                } else {
                    tone = t + 1;
                    last_tone = Some(c);
                }
                continue;
            }
        }
        if c == 'z' && tone != 0 {
            tone = 0;
            last_tone = None;
            continue;
        }
        let replacement = match c {
            'a' => Some(('a', 'â')),
            'e' => Some(('e', 'ê')),
            'o' => Some(('o', 'ô')),
            'd' => Some(('d', 'đ')),
            _ => None,
        };
        if let Some((from, to)) = replacement {
            if let Some(last) = out.last_mut() {
                if base(*last) == from {
                    *last = case(to, *last);
                    continue;
                }
                if base(*last) == to {
                    *last = case(from, *last);
                    out.push(ch);
                    continue;
                }
            }
        }
        if c == 'w' {
            // Escape an inserted shortcut anywhere: nhww -> nhw.
            if previous_shortcut_w {
                let last = out.last_mut().unwrap();
                *last = case('w', *last);
                continue;
            }
            let mut changed = false;
            for value in out.iter_mut().rev().take(3) {
                let to = match base(*value) {
                    'a' => 'ă',
                    'o' => 'ơ',
                    'u' => 'ư',
                    _ => continue,
                };
                *value = case(to, *value);
                changed = true;
                if to == 'ă' {
                    break;
                }
            }
            if changed {
                continue;
            }
            out.push(case('ư', ch));
            shortcut_w = true;
            continue;
        }
        out.push(ch);
    }
    let mut indices: Vec<usize> = out
        .iter()
        .enumerate()
        .filter(|(_, c)| VOWELS.iter().any(|s| s.starts_with(base(**c))))
        .map(|(i, _)| i)
        .collect();
    // qu / gi carry an initial glide, except when it is the only vowel.
    if indices.len() > 1
        && out.len() > 1
        && ((base(out[0]) == 'q' && base(out[1]) == 'u')
            || (base(out[0]) == 'g' && base(out[1]) == 'i'))
    {
        indices.retain(|i| *i != 1);
    }
    let target = indices
        .iter()
        .rev()
        .find(|i| "êơ".contains(base(out[**i])))
        .copied()
        .or_else(|| match indices.len() {
            0 => None,
            1 => Some(indices[0]),
            n => {
                let last = *indices.last().unwrap();
                Some(if last + 1 < out.len() {
                    last
                } else if n >= 3 {
                    indices[1]
                } else {
                    indices[0]
                })
            }
        });
    if let Some(i) = target {
        let row = VOWELS.iter().find(|s| s.starts_with(base(out[i]))).unwrap();
        out[i] = case(row.chars().nth(tone).unwrap(), out[i]);
    }
    out.into_iter().collect()
}

/// Exact phrases win; otherwise retain the best nine complete dictionary paths.
/// Apostrophes are hard boundaries, and incomplete suffixes never get dropped.
fn pinyin_candidates(key: &str) -> Vec<String> {
    if key.is_empty()
        || key.len() > 128
        || !key.bytes().all(|b| b.is_ascii_lowercase() || b == b'\'')
        || key.starts_with('\'')
        || key.ends_with('\'')
        || key.contains("''")
    {
        return Vec::new();
    }
    if let Some(exact) = dictionary().get(key) {
        return exact.clone();
    }
    let mut paths: Vec<Vec<(usize, String)>> = vec![Vec::new(); key.len() + 1];
    paths[0].push((0, String::new()));
    for end in 1..=key.len() {
        if key.as_bytes()[end - 1] == b'\'' {
            paths[end] = paths[end - 1].clone();
            continue;
        }
        let mut next = Vec::new();
        for start in 0..end {
            if paths[start].is_empty() {
                continue;
            }
            let Some(words) = dictionary().get(&key[start..end]) else {
                continue;
            };
            for (cost, prefix) in &paths[start] {
                for (rank, word) in words.iter().take(9).enumerate() {
                    next.push((cost + 100 + rank, format!("{prefix}{word}")));
                }
            }
        }
        next.sort();
        let mut seen = HashSet::new();
        next.retain(|(_, word)| seen.insert(word.clone()));
        next.truncate(9);
        paths[end] = next;
    }
    paths
        .pop()
        .unwrap()
        .into_iter()
        .map(|(_, word)| word)
        .collect()
}

/// Display-only syllable separators; the host retains original keystrokes.
fn pinyin_preedit(raw: &str) -> String {
    static SYLLABLES: OnceLock<HashSet<&'static str>> = OnceLock::new();
    let syllables = SYLLABLES.get_or_init(|| {
        [
            include_str!("../data/pinyin_simp.dict.yaml"),
            include_str!("../data/luna_pinyin.dict.yaml"),
        ]
        .iter()
        .flat_map(|source| source.lines())
        .filter_map(|line| line.split('\t').nth(1))
        .flat_map(str::split_whitespace)
        .filter(|s| s.bytes().all(|b| b.is_ascii_lowercase()))
        .collect()
    });
    if !raw.is_ascii() || raw.len() > 128 {
        return raw.to_owned();
    }
    raw.split('\'')
        .map(|part| {
            let key = part.to_ascii_lowercase();
            let mut paths: Vec<Option<Vec<usize>>> = vec![None; key.len() + 1];
            paths[key.len()] = Some(Vec::new());
            for start in (0..key.len()).rev() {
                for end in (start + 1..=key.len().min(start + 6)).rev() {
                    if !syllables.contains(&key[start..end]) {
                        continue;
                    }
                    if let Some(suffix) = &paths[end] {
                        let mut path = vec![end];
                        path.extend(suffix);
                        if paths[start]
                            .as_ref()
                            .is_none_or(|old| path.len() < old.len())
                        {
                            paths[start] = Some(path);
                        }
                    }
                }
                if paths[start].is_none()
                    && syllables
                        .iter()
                        .any(|syllable| syllable.starts_with(&key[start..]))
                {
                    paths[start] = Some(vec![key.len()]);
                }
            }
            let Some(ends) = &paths[0] else {
                return part.to_owned();
            };
            let mut start = 0;
            ends.iter()
                .map(|&end| {
                    let syllable = &part[start..end];
                    start = end;
                    syllable
                })
                .collect::<Vec<_>>()
                .join("'")
        })
        .collect::<Vec<_>>()
        .join("'")
}

fn pinyin_initials(raw: &str) -> bool {
    (2..=8).contains(&raw.len()) && raw.bytes().all(|b| b"bpmfdtnlgkhjqxrzcsyw".contains(&b))
}

fn pinyin_prefix(key: &str) -> bool {
    static PREFIXES: OnceLock<HashSet<String>> = OnceLock::new();
    PREFIXES
        .get_or_init(|| {
            dictionary()
                .keys()
                .flat_map(|reading| (1..=reading.len().min(6)).map(|end| reading[..end].to_owned()))
                .collect()
        })
        .contains(key)
}

/// mode: 0 auto, 1 English, 2 Vietnamese, 3 Chinese.
/// Wire format: language, preedit, then candidate lines. Input is bounded by the host.
pub fn compose(raw: &str, mode: u32) -> String {
    compose_with_context(raw, mode, 0)
}

fn compose_with_context(raw: &str, mode: u32, previous: u32) -> String {
    let key = raw.to_ascii_lowercase();
    let vietnamese = telex(raw);
    let english = [
        "as", "is", "of", "or", "if", "us", "was", "has", "his", "her", "for", "our", "your",
        "this", "that", "with", "test", "text", "hello", "world", "office", "coffee", "book",
        "look", "good", "soon", "see", "new", "how", "now", "show", "know", "low", "row", "law",
        "draw", "raw", "saw", "few",
    ];
    let candidates = if mode == 0 || mode == 3 {
        pinyin_candidates(&key)
    } else {
        Vec::new()
    };
    let detected = if mode != 0 {
        mode
    } else if previous == 2 && valid_vietnamese(&vietnamese) {
        // Resolve valid Vietnamese/English collisions using the typing context.
        2
    } else if previous == 3
        && key.len() <= 3
        && !candidates.is_empty()
        && !(vietnamese != raw && valid_vietnamese(&vietnamese))
    {
        // Short Pinyin particles/pronouns can also be English words (a, me, he).
        // Continue the most recently confirmed Chinese context in this ambiguity.
        3
    } else if english.contains(&key.as_str())
        || include_str!("../data/english-common.txt")
            .split_whitespace()
            .any(|word| word == key)
    {
        1
    } else if vietnamese != raw && valid_vietnamese(&vietnamese) {
        2
    } else if previous == 3
        && !raw.is_empty()
        && (!candidates.is_empty() || pinyin_prefix(&key) || pinyin_initials(raw))
    {
        3
    } else if valid_vietnamese(&vietnamese) {
        2
    } else if !candidates.is_empty() && key.len() >= 4 {
        3
    } else {
        1
    };
    match detected {
        2 => format!("Vietnamese\n{vietnamese}"),
        3 => format!(
            "Chinese\n{}{}",
            pinyin_preedit(raw),
            if candidates.is_empty() {
                String::new()
            } else {
                format!("\n{}", candidates.join("\n"))
            }
        ),
        _ => format!("English\n{raw}"),
    }
}

/// Conservative boundary correction: known English typos and a unique adjacent
/// key transposition validated against Vietnamese / Pinyin dictionaries.
fn correction(raw: &str, mode: u32) -> Option<String> {
    if !(3..=32).contains(&raw.len()) || !raw.is_ascii() {
        return None;
    }
    let lower = raw.to_ascii_lowercase();
    if mode <= 1 {
        let replacement = match lower.as_str() {
            "teh" => Some("the"),
            "hte" => Some("the"),
            "adn" => Some("and"),
            "recieve" => Some("receive"),
            "helllo" => Some("hello"),
            "becuase" => Some("because"),
            "thier" => Some("their"),
            "woudl" => Some("would"),
            "dont" => Some("don't"),
            _ => None,
        };
        if let Some(word) = replacement {
            let word = if raw.chars().all(|c| c.is_ascii_uppercase()) {
                word.to_uppercase()
            } else if raw.starts_with(|c: char| c.is_ascii_uppercase()) {
                format!("{}{}", word[..1].to_uppercase(), &word[1..])
            } else {
                word.to_owned()
            };
            return Some(format!("English\n{word}"));
        }
    }
    if mode == 1
        || valid_vietnamese(&telex(raw))
        || !pinyin_candidates(&lower).is_empty()
        || include_str!("../data/english-common.txt")
            .split_whitespace()
            .any(|w| w == lower)
    {
        return None;
    }
    let mut results = HashSet::new();
    for i in 0..raw.len() - 1 {
        let mut bytes = raw.as_bytes().to_vec();
        bytes.swap(i, i + 1);
        let edited = String::from_utf8(bytes).unwrap();
        let vi = telex(&edited);
        if (mode == 0 || mode == 2) && vi != edited && valid_vietnamese(&vi) {
            results.insert(format!("Vietnamese\n{vi}"));
        }
        if mode == 3 || (mode == 0 && raw.len() >= 6) {
            let candidates = pinyin_candidates(&edited.to_ascii_lowercase());
            if let Some(first) = candidates.first() {
                results.insert(format!("Chinese\n{raw}\n{first}"));
            }
        }
    }
    if results.len() == 1 {
        results.into_iter().next()
    } else {
        None
    }
}

/// # Safety
/// `raw` must be a valid NUL-terminated UTF-8 string.
#[no_mangle]
pub unsafe extern "C" fn keyin_correct(raw: *const c_char, mode: u32) -> *mut c_char {
    if raw.is_null() {
        return std::ptr::null_mut();
    }
    let raw = unsafe { CStr::from_ptr(raw) }.to_string_lossy();
    CString::new(correction(&raw, mode).unwrap_or_default())
        .unwrap()
        .into_raw()
}

/// Languages available for explicit selection: bits 1 English, 2 Vietnamese,
/// 3 Chinese. Literal input is always available; manual modes remain unrestricted.
fn language_options(raw: &str) -> u32 {
    if raw.is_empty() {
        return 0;
    }
    let mut options = 1 << 1;
    if valid_vietnamese(&telex(raw)) {
        options |= 1 << 2;
    }
    if !pinyin_candidates(&raw.to_ascii_lowercase()).is_empty() || pinyin_initials(raw) {
        options |= 1 << 3;
    }
    options
}

/// # Safety
/// `raw` must point to a valid NUL-terminated UTF-8 string.
#[no_mangle]
pub unsafe extern "C" fn keyin_language_options(raw: *const c_char) -> u32 {
    if raw.is_null() {
        return 0;
    }
    language_options(&unsafe { CStr::from_ptr(raw) }.to_string_lossy())
}

/// # Safety
/// `raw` must point to a valid NUL-terminated UTF-8 string.
#[no_mangle]
pub unsafe extern "C" fn keyin_compose_context(
    raw: *const c_char,
    mode: u32,
    previous: u32,
) -> *mut c_char {
    if raw.is_null() {
        return std::ptr::null_mut();
    }
    let raw = unsafe { CStr::from_ptr(raw) }.to_string_lossy();
    CString::new(compose_with_context(&raw, mode, previous))
        .unwrap()
        .into_raw()
}

/// # Safety
/// `raw` must point to a valid NUL-terminated UTF-8 string for this call.
#[no_mangle]
pub unsafe extern "C" fn keyin_compose(raw: *const c_char, mode: u32) -> *mut c_char {
    if raw.is_null() {
        return std::ptr::null_mut();
    }
    let raw = unsafe { CStr::from_ptr(raw) }.to_string_lossy();
    CString::new(compose(&raw, mode)).unwrap().into_raw()
}
/// # Safety
/// `ptr` must be an unfreed pointer returned by `keyin_compose`.
#[no_mangle]
pub unsafe extern "C" fn keyin_free(ptr: *mut c_char) {
    if !ptr.is_null() {
        drop(unsafe { CString::from_raw(ptr) });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn vietnamese_words() {
        for (input, expected) in [
            ("tieengs", "tiếng"),
            ("Vieetj", "Việt"),
            ("ddawng", "đăng"),
            ("dduwowngf", "đường"),
            ("nguwowif", "người"),
            ("hoaf", "hòa"),
            ("quas", "quá"),
            ("gif", "gì"),
            ("ass", "as"),
            ("aaa", "aa"),
            ("asz", "a"),
        ] {
            assert_eq!(telex(input), expected, "{input}");
        }
    }
    #[test]
    fn detection_and_overrides() {
        assert_eq!(compose("hello", 0), "English\nhello");
        assert_eq!(compose("tieengs", 0), "Vietnamese\ntiếng");
        assert!(compose("nihao", 0).starts_with("Chinese\nni'hao\n你好"));
        assert_eq!(compose("nihao", 1), "English\nnihao");
        assert!(compose("ni", 3).contains("\n你"));
        assert_eq!(compose("as", 2), "Vietnamese\ná");
    }
    #[test]
    fn editing_recomputes_from_original_keystrokes() {
        assert_eq!(telex("tieengs"), "tiếng");
        assert_eq!(telex("tieeng"), "tiêng");
        assert_eq!(telex("tiee"), "tiê");
        assert_eq!(telex(""), "");
        assert_eq!(telex("DDAS"), "ĐÁ");
        assert_eq!(compose("rust", 0), "English\nrust");
        assert_eq!(compose("ni'hao", 3).lines().nth(2), Some("你好"));
    }
    #[test]
    fn arbitrary_unicode_and_unknown_readings_are_preserved() {
        assert_eq!(compose("🙂", 1), "English\n🙂");
        assert_eq!(compose("zzzzzz", 3), "Chinese\nzzzzzz");
        for text in [
            "", "a", "s", "w", "z", "đ", "中文", "aáà", "uwow", "ssss", "aaaa",
        ] {
            for mode in 0..4 {
                let _ = compose(text, mode);
            }
        }
    }
    #[test]
    fn dictionary_coverage() {
        assert!(dictionary().len() > 10000);
        assert!(vietnamese_syllables().len() > 6000);
    }
    #[test]
    fn automatic_pinyin_separators_are_display_only() {
        for (raw, expected) in [
            ("nihaolema", "ni'hao'le'ma"),
            ("zenmeyang", "zen'me'yang"),
            ("kankan", "kan'kan"),
            ("NIHAO", "NI'HAO"),
            ("ni'hao", "ni'hao"),
            ("xi'an", "xi'an"),
            ("xian", "xian"),
            ("nihaol", "ni'hao'l"),
            ("nihao'", "ni'hao'"),
            ("zzzzzz", "zzzzzz"),
        ] {
            assert_eq!(pinyin_preedit(raw), expected, "{raw}");
            assert_eq!(compose(raw, 3).lines().nth(1), Some(expected), "{raw}");
        }
        assert_eq!(compose("nihao", 1), "English\nnihao");
        assert_eq!(compose("nhwng", 0), "Vietnamese\nnhưng");
    }
    #[test]
    fn frequent_simplified_phrases_rank_first() {
        for (raw, expected) in [
            ("kankan", "看看"),
            ("zenmeyang", "怎么样"),
            ("kan", "看"),
            ("zenme", "怎么"),
            ("kan'kan", "看看"),
            ("zen'me'yang", "怎么样"),
        ] {
            assert_eq!(
                pinyin_candidates(raw).first().map(String::as_str),
                Some(expected),
                "{raw}"
            );
            assert_eq!(compose(raw, 3).lines().nth(2), Some(expected), "{raw}");
            if raw.len() >= 4 {
                assert_eq!(compose(raw, 0).lines().nth(2), Some(expected), "{raw}");
            }
        }
        assert_eq!(compose("kankan", 1), "English\nkankan");
    }
    #[test]
    fn standalone_w_and_escape() {
        for (raw, expected) in [
            ("w", "ư"),
            ("W", "Ư"),
            ("ws", "ứ"),
            ("ww", "w"),
            ("WW", "W"),
            ("uw", "ư"),
            ("nhwng", "nhưng"),
            ("NHWNG", "NHƯNG"),
            ("nhwngz", "nhưngz"),
            ("nhww", "nhw"),
        ] {
            assert_eq!(telex(raw), expected);
        }
        assert_eq!(compose("w", 0), "Vietnamese\nư");
        assert_eq!(compose("nhwng", 0), "Vietnamese\nnhưng");
        assert_eq!(compose("nhwng", 2), "Vietnamese\nnhưng");
        assert_eq!(compose("nhwng", 1), "English\nnhwng");
        assert_eq!(compose("world", 0), "English\nworld");
        assert!(compose("wo", 3).starts_with("Chinese\nwo\n我"));
    }
    #[test]
    fn pinyin_phrases_are_segmented_without_losing_suffixes() {
        for raw in ["nihaolema", "ni'hao'le'ma", "NIHAOLEMA"] {
            for mode in [0, 3] {
                assert_eq!(compose(raw, mode).lines().nth(2), Some("你好了吗"), "{raw}");
            }
        }
        assert_eq!(compose("nihaolema", 1), "English\nnihaolema");
        assert!(pinyin_candidates("nihaolemaq").is_empty());
        assert!(pinyin_candidates("ni''hao").is_empty());
        assert!(pinyin_candidates("🙂").is_empty());
        assert!(pinyin_candidates(&"a".repeat(129)).is_empty());
        assert_eq!(pinyin_candidates("nihaole")[0], "你好了");
        assert_eq!(pinyin_candidates("nihao")[0], "你好");
        assert!(!pinyin_candidates("xi'an").contains(&"先".to_string()));
    }
    #[test]
    fn vietnamese_dictionary_resolves_pinyin_collisions() {
        for (raw, expected) in [
            ("chao", "chao"),
            ("chong", "chong"),
            ("hoaf", "hòa"),
            ("thuyr", "thủy"),
            ("Vieetj", "Việt"),
        ] {
            assert_eq!(compose(raw, 0), format!("Vietnamese\n{expected}"));
        }
        for raw in ["nihao", "zhongguo", "xiexie", "shijie", "zhongwen"] {
            assert!(compose(raw, 0).starts_with("Chinese\n"), "{raw}");
        }
        assert_eq!(compose("qzuw", 0), "English\nqzuw");
        assert_eq!(compose("qzuw", 2), "Vietnamese\nqzư");
        assert!(compose("chong", 3).starts_with("Chinese\nchong\n"));
    }
    #[test]
    fn contextual_detection_can_switch_without_getting_stuck() {
        for raw in ["ne", "ma", "ba", "me", "he", "a", "n"] {
            assert!(
                compose_with_context(raw, 0, 3).starts_with("Chinese\n"),
                "{raw}"
            );
        }
        assert_eq!(compose_with_context("me", 0, 1), "English\nme");
        assert_eq!(compose_with_context("ne", 1, 3), "English\nne");
        assert!(compose_with_context("ni", 0, 3).starts_with("Chinese\n"));
        assert!(compose_with_context("hao", 0, 3).starts_with("Chinese\n"));
        assert!(compose_with_context("zh", 0, 3).starts_with("Chinese\n"));
        assert_eq!(compose_with_context("hello", 0, 3), "English\nhello");
        assert_eq!(compose_with_context("nhwng", 0, 3), "Vietnamese\nnhưng");
        assert_eq!(compose_with_context("tieengs", 0, 3), "Vietnamese\ntiếng");
        assert!(compose_with_context("zenmeyang", 0, 2).starts_with("Chinese\n"));
        assert_eq!(compose_with_context("chao", 0, 2), "Vietnamese\nchao");
        assert!(compose_with_context("chao", 0, 3).starts_with("Chinese\n"));
        assert_eq!(compose_with_context("ni", 1, 3), "English\nni");
        assert_eq!(compose_with_context("nhwng", 2, 3), "Vietnamese\nnhưng");
    }
    #[test]
    fn vietnamese_context_wins_ambiguous_english_words() {
        assert_eq!(compose_with_context("its", 0, 2), "Vietnamese\nít");
        assert_eq!(compose_with_context("its", 0, 1), "English\nits");
        assert_eq!(compose_with_context("its", 1, 2), "English\nits");
        assert_eq!(compose_with_context("hello", 0, 2), "English\nhello");
        assert_eq!(compose_with_context("as", 0, 2), "Vietnamese\ná");
    }

    #[test]
    fn initials_need_chinese_context_and_preserve_telex() {
        assert!(compose_with_context("nhlm", 0, 3).starts_with("Chinese\n"));
        assert!(compose_with_context("zmy", 0, 3).starts_with("Chinese\n"));
        for previous in [0, 1, 2] {
            assert_eq!(compose_with_context("nhlm", 0, previous), "English\nnhlm");
        }
        assert_eq!(compose_with_context("nhwng", 0, 3), "Vietnamese\nnhưng");
        assert_eq!(compose_with_context("hello", 0, 3), "English\nhello");
        assert_eq!(
            compose_with_context("HTTP", 0, 3).lines().nth(1),
            Some("HTTP")
        );
        assert!(!compose_with_context("HTTP", 0, 3).starts_with("Chinese\n"));
        assert_eq!(compose_with_context("nhlm", 1, 3), "English\nnhlm");
    }
    #[test]
    fn explicit_language_alternatives() {
        assert_eq!(language_options(""), 0);
        assert_eq!(language_options("chao"), (1 << 1) | (1 << 2) | (1 << 3));
        assert_eq!(language_options("nhlm"), (1 << 1) | (1 << 3));
        assert_eq!(language_options("🙂"), 1 << 1);
        assert!(language_options("nhwng") & (1 << 2) != 0);
    }
    #[test]
    fn ffi_ownership() {
        unsafe {
            let input = CString::new("tieengs").unwrap();
            let result = keyin_compose(input.as_ptr(), 2);
            assert_eq!(
                CStr::from_ptr(result).to_str().unwrap(),
                "Vietnamese\ntiếng"
            );
            keyin_free(result);
            keyin_free(std::ptr::null_mut());
        }
    }
    #[test]
    fn boundary_typo_correction() {
        assert_eq!(correction("teh", 0), Some("English\nthe".into()));
        assert_eq!(correction("Recieve", 1), Some("English\nReceive".into()));
        assert_eq!(correction("DONT", 1), Some("English\nDON'T".into()));
        for raw in ["nhwng", "hello", "chong", "nihao", "🙂", ""] {
            assert_eq!(correction(raw, 0), None, "{raw}");
        }
        assert_eq!(correction("teh", 3), None);
        assert_eq!(correction("tienegs", 2), Some("Vietnamese\ntiếng".into()));
        assert_eq!(correction("nhwgn", 2), Some("Vietnamese\nnhưng".into()));
        assert_eq!(correction("tienegs", 1), None);
        assert_eq!(correction("nihaolmea", 3), None); // Ambiguous: do not guess.
    }
}
